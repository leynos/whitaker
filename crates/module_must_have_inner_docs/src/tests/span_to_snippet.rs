//! Span-to-snippet tests for module doc detection fallbacks.

use super::{ModuleDocDisposition, detect_module_docs_in_span, primary_span_for_disposition};
use rustc_span::source_map::{FilePathMapping, SourceMap};
use rustc_span::{FileName, Span};

#[test]
fn span_to_snippet_failure_returns_unknown() {
    let (source_map, span) = unresolvable_span();
    let disposition = detect_module_docs_in_span(&source_map, span);

    assert_eq!(disposition, ModuleDocDisposition::Unknown);
}

#[test]
fn span_to_snippet_failure_skips_diagnostic() {
    let (source_map, span) = unresolvable_span();
    let disposition = detect_module_docs_in_span(&source_map, span);

    assert_eq!(disposition, ModuleDocDisposition::Unknown);
    assert!(
        primary_span_for_disposition(disposition, span).is_none(),
        "unknown disposition should skip lint emission"
    );
}

fn unresolvable_span() -> (SourceMap, Span) {
    let source_map = SourceMap::new(FilePathMapping::empty());
    let first =
        source_map.new_source_file(FileName::Custom("first.rs".into()), "mod first {}".into());
    let second =
        source_map.new_source_file(FileName::Custom("second.rs".into()), "mod second {}".into());
    let span = Span::with_root_ctxt(first.start_pos, second.start_pos);

    (source_map, span)
}
