//! Span-to-snippet tests for module doc detection fallbacks.

use super::{ModuleDocDisposition, detect_module_docs_in_span, primary_span_for_disposition};
use rstest::{fixture, rstest};
use rustc_span::source_map::{FilePathMapping, SourceMap};
use rustc_span::{FileName, Span};

#[rstest]
fn span_to_snippet_failure_returns_unknown(unresolvable_span_fixture: (SourceMap, Span)) {
    let (source_map, span) = unresolvable_span_fixture;
    let disposition = detect_module_docs_in_span(&source_map, span);

    assert_eq!(disposition, ModuleDocDisposition::Unknown);
}

#[rstest]
fn span_to_snippet_failure_skips_diagnostic(unresolvable_span_fixture: (SourceMap, Span)) {
    let (source_map, span) = unresolvable_span_fixture;
    let disposition = detect_module_docs_in_span(&source_map, span);

    assert_eq!(disposition, ModuleDocDisposition::Unknown);
    assert!(
        primary_span_for_disposition(disposition, span).is_none(),
        "unknown disposition should skip lint emission"
    );
}

#[fixture]
fn unresolvable_span_fixture() -> (SourceMap, Span) {
    unresolvable_span()
}

/// Builds a cross-file span (start in "first.rs", end in "second.rs") with the
/// root context so the `SourceMap` cannot resolve it to a single file. This
/// deliberate edge case exercises the snippet resolution failure path.
fn unresolvable_span() -> (SourceMap, Span) {
    let source_map = SourceMap::new(FilePathMapping::empty());
    let first =
        source_map.new_source_file(FileName::Custom("first.rs".into()), "mod first {}".into());
    let second =
        source_map.new_source_file(FileName::Custom("second.rs".into()), "mod second {}".into());
    let span = Span::with_root_ctxt(first.start_pos, second.start_pos);

    (source_map, span)
}
