# Whitaker clone detector: two-pass design (rustc_lexer + ra_ap_syntax) with SARIF IR

## Purpose and non-goals

**Purpose.** Provide a practical, scalable clone detector for Rust that
integrates cleanly with Whitaker's Dylint suite. The detector runs as a
companion command-line interface (CLI) that produces a Static Analysis Results
Interchange Format (SARIF) report. Whitaker lints ingest this SARIF output and
emit precise diagnostics inside the compiler or editor. The detector focuses on
Type-1 (whitespace or comment differences), Type-2 (identifier or literal
renaming), and Type-3 (near-miss) clones.

**Non-goals.** Type-4 (semantic) clones; mid-level intermediate representation
(MIR), program dependence graph (PDG), or static single assignment (SSA)
analysis; cross-language clones.

## Overview: two-pass pipeline and SARIF IR

The repository layout below highlights new crates and their roles.

```plaintext
crates/
  common/                     # existing Whitaker helpers
  whitaker_sarif/             # SARIF 2.1.0 model + helpers
  whitaker_clones_core/       # token + AST engines, LSH, grouping, scoring
  whitaker_clones_cli/        # `cargo whitaker clones` binary
  clone_detected/             # (dylint) thin consumer of SARIF -> diagnostics
  suite/                      # existing aggregate dylint library (unchanged)
```

**Pass A - token pipeline (rustc_lexer).** Goal: fast, workspace-wide discovery
of Type-1 and Type-2 candidates.

- Tokenise.
- Normalise.
- k-shingle to Rabin-Karp rolling hash.
- Winnowing.
- MinHash + locality-sensitive hashing (LSH).
- Candidate pairing.
- Jaccard similarity.
- Emit SARIF (pre-refined).

**Pass B - AST pipeline (ra_ap_syntax).** Goal: refine Pass A candidates and
detect Type-3 near-miss clones.

- Parse with `ra_ap_syntax`.
- Map candidate spans to `SyntaxNode` regions.
- Derive abstract syntax tree (AST) feature vectors and canonical subtree
  hashes.
- Cosine or SimHash scoring with optional light tree-edit refinement.
- Update or append SARIF results (refined).

**IR handshake.** SARIF 2.1.0 is the intermediate representation (IR) between
passes. Use `runs[0]` for the token pass and `runs[1]` for the AST pass. Each
`result` carries `properties` for Whitaker-specific metadata.

## Crate responsibilities

### `whitaker_sarif`

- `serde` models of SARIF 2.1.0 (subset plus extensions) with `From` and `Into`
  helpers.
- Helpers to build rules, results, locations, artifacts, and invocations.
- Stable file layout: `target/whitaker/clones.{pass}.sarif` and
  `target/whitaker/clones.refined.sarif`.
- Merge logic: combine runs and deduplicate results by
  `(fingerprint, file, region)`.

### `whitaker_clones_core`

- **fs**: workspace discovery via `ignore`, `globset`, and `camino`.
- **token**: `rustc_lexer`-based normalisation, k-shingling, winnowing, and
  similarity.
- **ast**: `ra_ap_syntax` parsing, region-to-node mapping, and feature
  extraction.
- **index**: MinHash and LSH (N bands by R rows), candidate generation, and
  grouping.
- **score**: Jaccard (tokens), cosine (AST features), and optional edit distance
  refinement.
- **group**: connected components, star, or complete-linkage to produce clone
  classes.
- **config**: thresholds, toggles (ignore identifiers or literals), and LSH
  auto-tuning.

### `whitaker_clones_cli`

- `cargo whitaker clones` with subcommands: `scan`, `refine`, `report`, and
  `clean`.
- Produces SARIF plus an optional Hypertext Markup Language (HTML) summary for
  humans.

### `clone_detected` (Dylint)

- Thin consumer: load SARIF and emit lints for files in the current crate only.
- Rule IDs map to Whitaker rules; suggestions link to HTML or include local
  fix-its.

## Data model and configuration

### Normalised fragment key

`FragmentId = Blake3(file_rel_path | start_byte | end_byte | norm_profile)`

### Similarity profiles

- **T1:** strip comments and whitespace.
- **T2:** T1 plus canonicalise identifiers and literals (`<ID_i>`, `<NUM>`,
  `<STR>`); scope-local ID numbering.
- **T3:** AST features plus subtree hashes; identifier and literal
  canonicalisation applies at concrete syntax tree (CST) level.

### Configuration (Tom's Obvious, Minimal Language (TOML); used by CLI and lint)

```toml
[clones]
min_lines = 5
min_nodes = 10
similarity_threshold = 0.80
max_edit_distance = 50.0
ignore_literals = false
ignore_identifiers = false

[type_thresholds]
# must satisfy: type1 > type2 > type3
type1 = 0.95
type2 = 0.90
type3 = 0.85

[lsh]
enabled = "auto"     # "true" | "false" | "auto"
auto_threshold = 500
bands = 32
rows = 4
hashes = 128
```

## SARIF schema and mapping

### Rules

- `WHK001` - Type-1 clone (token exact after trivia removal)
- `WHK002` - Type-2 clone (token equivalent under renaming)
- `WHK003` - Type-3 clone (near-miss; AST similar)

### Result mapping

- `ruleId`: `WHK00{1|2|3}`
- `level`: `warning` (configurable), `note` for marginal pairs
- `message.text`:
  `Type-{N} clone: {fileA}:{spanA} <-> {fileB}:{spanB} (sim = 0.92)`
- `locations[0]`: primary occurrence (current file if emitted by lint)
- `relatedLocations[*]`: peer fragments in the class
- `partialFingerprints`: `{ "whitakerFragment": FragmentId, "tokenHash": u64,`
  `"astHash": u64 }`
- `properties`:

  ```json
  {
    "whitaker": {
      "profile": "T1|T2|T3",
      "k": 25,
      "window": 16,
      "jaccard": 0.92,
      "cosine": 0.88,
      "groupId": 174,
      "classSize": 4
    }
  }
  ```

### Runs

- **Run 0**: token pass (producer = `whitaker_clones_cli@token`).
- **Run 1**: AST pass (producer = `whitaker_clones_cli@ast`,
  `invocations[0].executionSuccessful = true` if completed).

Whitaker's lint reads either the refined run (preferred) or falls back to Run 0.

## Pass A: token engine (rustc_lexer)

### Normalisation

```rust
pub enum Norm { T1, T2 }

pub fn normalise(src: &str, norm: Norm) -> Vec<(u32, std::ops::Range<usize>)> {
    use rustc_lexer::{tokenize, TokenKind};
    let mut out = Vec::new();
    let mut off = 0;
    for tok in tokenize(src) {
        let kind = match tok.kind {
            TokenKind::Whitespace
            | TokenKind::LineComment
            | TokenKind::BlockComment { .. } => {
                off += tok.len;
                continue;
            }
            TokenKind::Ident if matches!(norm, Norm::T2) => 1, // <ID>
            TokenKind::Literal { .. } if matches!(norm, Norm::T2) => 2, // <LIT>
            _ => tok.kind as u32,
        };
        out.push((kind, off..off + tok.len));
        off += tok.len;
    }
    out
}
```

### k-shingles, rolling hash, winnowing

- **Shingle:** sequence of `k` token kinds (default `k=25`).
- **Rabin-Karp:** base `B = 1_000_003` 64-bit rolling hash.
- **Winnowing:** window `w=16`; keep minima to stabilise fingerprints.
- Store `(fingerprint, region)` where `region` spans token bytes.

### MinHash and LSH

- Compute 128-dimensional MinHash for each fragment's fingerprint set.
- LSH parameters default from config; candidate pairs collide in at least one
  band.

### Pair scoring (token level)

- Jaccard over fingerprint sets; accept as Type-1 or Type-2 if above thresholds
  (`type1`, `type2`).
- Build clone classes via connected components; record class similarity as the
  mean pair score.

### SARIF emission (Run 0)

- For each accepted pair or class, write a `result` with the T1 or T2 rule.
- Include `artifactLocation`, `region` (1-based line and column),
  `partialFingerprints`, and `properties`.

## Pass B: AST engine (ra_ap_syntax)

### Parsing and region mapping

```rust
use ra_ap_syntax::{SourceFile, SyntaxNode, TextRange, TextSize};

pub struct AstFragment {
    pub node: SyntaxNode,
    pub range: TextRange,
}

pub fn map_bytes_to_node(file_text: &str, start: usize, end: usize) -> AstFragment {
    let sf = SourceFile::parse(file_text).ok().expect("parse");
    let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));
    // Heuristic: choose the smallest node covering the byte range.
    let mut best = sf.syntax().clone();
    for n in sf.syntax().descendants() {
        let r = n.text_range();
        if r.contains_range(range) && r.len() <= best.text_range().len() {
            best = n;
        }
    }
    AstFragment { node: best, range }
}
```

### Feature extraction

- **Node-kind histogram:** vector `V[kind_id] += w(depth)` with
  `w(depth) = 1 / (1 + depth)`.
- **Production multiset:** bigrams and trigrams of
  `(parent_kind -> child_kind)`.
- **Canonical subtree hash:** Merkle-style hash where leaves are normalised
  (`<ID>`, `<LIT>`), and internal nodes include kind plus arity.

### Scoring and acceptance (Type-3)

- Compute cosine similarity between histograms.
- If cosine is at least `type3_threshold`, mark as a Type-3 candidate.
- Optional refine: bounded tree-edit distance on a 500-node cap; accept if the
  normalised tree-edit distance is at most `max_edit_distance`.

### SARIF update (Run 1)

- For each refined pair, add or merge a `result` under `WHK003`, or upgrade
  `WHK002` to `WHK003` with `baselineState = "updated"`.
- Record `properties.whitaker.cosine` and the `astHash` partial fingerprint.

## Incrementality and caching

- **Per-file cache:** `{ path, mtime, hash(file_text), token_fp, minhash,
  ast_hashes }` in `target/whitaker/clones-cache.bin`.
- **Dirtying:** path or mtime change, or config hash change, invalidates
  entries.
- **Sharding:** index shards by the first 12 bits of the fingerprint to reduce
  memory peaks.

## Grouping and reporting

- **Grouping:** connected components by at-least-threshold similarity; store
  `groupId`.
- **HTML report (optional):** side-by-side diff with token or AST overlays and
  similarity gauges.
- **SARIF merge:** combine Run 0 and Run 1; deduplicate.

## Dylint integration (`clone_detected` lint)

- Load `target/whitaker/clones.refined.sarif` if present, else
  `target/whitaker/clones.token.sarif`.
- Filter results to current crate files using absolute path mapping from
  `SourceMap`.
- Emit diagnostics with spans; attach `help` suggesting decomposition or
  factoring; point to the HTML anchor for the group.
- Respect `#[allow(whitaker::clone_detected)]` and per-file allowlist in
  config.

## CLI surface

```bash
# Pass A: token scan over the workspace.
cargo whitaker clones scan \
  --paths . --include '**/*.rs' --exclude 'target/**' \
  --min-lines 5 --similarity-threshold 0.80 \
  --lsh auto --out target/whitaker/clones.token.sarif

# Pass B: AST refinement of candidates.
cargo whitaker clones refine \
  --in target/whitaker/clones.token.sarif \
  --out target/whitaker/clones.refined.sarif

# Summarise for humans.
cargo whitaker clones report --in target/whitaker/clones.refined.sarif --html
```

## Safety, scale, and performance notes

- Token pass is O(total tokens) plus LSH; memory is dominated by fingerprint
  sets, and winnowing keeps this compact.
- AST pass only touches candidate regions; cost is bounded by candidate count.
- Skip macro-expanded code by default (`span.from_expansion()`); include an
  opt-in flag.

## Testing strategy

- **Golden tests** for SARIF: stable JSON with deterministic ordering.
- **User interface (UI) tests** in the Dylint crate: consume pre-baked SARIF to
  verify diagnostics.
- **Property tests** for normalisation (for example, comment or whitespace
  permutations yield identical T1 tokens).

## Risks and mitigations

- **False positives** (builder or macro-heavy code): provide allowlists and
  raise T3 threshold.
- **Path mapping mismatches**: normalise with `camino::Utf8Path` and record
  `originalUriBaseIds` in SARIF.
- **AST drift** across `ra_ap_*` snapshots: pin versions and narrow the API
  surface in `whitaker_clones_core::ast`.

## Milestones

- **M1**: Pass A end-to-end (token) to SARIF; lint reads and warns on T1 or T2.
- **M2**: Pass B refinement (AST) to merged SARIF; T3 results; HTML diff.
- **M3**: Incremental cache; continuous integration (CI) integration; docs.

## Minimal code skeletons (selected)

### Token to fingerprints to candidates

```rust
pub struct Fingerprint {
    pub value: u64,
    pub range: std::ops::Range<usize>,
}

pub fn shingles(tokens: &[(u32, std::ops::Range<usize>)], k: usize) -> Vec<Fingerprint> {
    /* ... */
}

pub fn winnow(fps: &[Fingerprint], window: usize) -> Vec<Fingerprint> {
    /* ... */
}

pub struct MinHasher {
    /* seeds */
}

impl MinHasher {
    pub fn sketch(&self, fps: &[Fingerprint]) -> [u64; 128] {
        /* ... */
    }
}

pub struct LshIndex {
    /* bands by rows */
}

impl LshIndex {
    pub fn insert(&mut self, frag: &FragmentId, sketch: &[u64; 128]) {
        /* ... */
    }

    pub fn candidates(&self, frag: &FragmentId, sketch: &[u64; 128])
        -> impl Iterator<Item = &FragmentId> {
        /* ... */
    }
}
```

### AST features

```rust
pub fn ast_features(node: &ra_ap_syntax::SyntaxNode) -> FeatureVec {
    /* histogram with depth weights */
}

pub fn ast_hash(node: &ra_ap_syntax::SyntaxNode) -> u64 {
    /* merkle-ish canonical hash */
}
```

### SARIF builder (excerpt)

```rust
use whitaker_sarif as sarif;

pub fn make_result(rule: &str, loc_a: Loc, loc_b: Loc, sim: f32, props: Props) -> sarif::Result {
    sarif::Result::new(rule)
        .with_level("warning")
        .with_message(format!("{} <-> {} (sim = {sim:.2})", loc_a, loc_b))
        .with_locations(vec![loc_a.into()])
        .with_related_locations(vec![loc_b.into()])
        .with_properties(props.into())
}
```

## Acceptance criteria

- Pass A SARIF contains T1 and T2 pairs with correct spans and stable
  fingerprints.
- Pass B SARIF upgrades or adds T3 results; cosine and Jaccard values are
  present.
- Dylint emits diagnostics for files under analysis; hyperlinks (if HTML is
  generated) resolve to group anchors.
- Continuous integration job runs token and AST passes on the workspace; no
  panics; stable output across runs unless sources change.
