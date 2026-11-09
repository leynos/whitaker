# Architectural decision record (ADR) 0011: Bumpy Road reference semantics

## Status

Accepted (2026-11-08): use CodeScene's Bumpy Road diagnostic as the reference
for Whitaker's intended observable classification, as requested in issue
[#357](https://github.com/leynos/whitaker/issues/357). The implementation and
migration proposed in
[RFC 0013](../rfcs/rfc-0013-bumpy-road-semantic-alignment.md) remain subject to
review.

## Date

2026-11-08.

## Context and problem statement

Whitaker's weighted line-signal heuristic misses a function for which CodeScene
reports two nested regions. The lint runs; the algorithms disagree. Sharing the
Bumpy Road name without stating this difference encourages users to treat a
clean Whitaker result as a clean CodeScene result.

## Decision outcome

Semantic alignment is a project goal. Validate intended behaviour against a
versioned CodeScene corpus, including the equivalent `relative_to_manifest`
fixture, rather than treating the existing numeric heuristic as the semantic
authority. Document known differences until alignment is demonstrated.

Retain the compiler-adapter/pure-analysis boundary when designing a structural
detector. CodeScene is a development-time reference oracle, not a runtime
service dependency. Do not claim knowledge of unpublished implementation
details from public descriptions alone.

## Consequences

The existing threshold, smoothing and minimum-length settings continue to
control the shipped lint. This decision does not change defaults or approve the
RFC's proposed configuration schema. Future implementation must preserve an
explicit migration path, prove the two-region case and test broader reference
agreement before claiming compatibility.
