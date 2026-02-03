//! Span-to-snippet tests for module doc detection fallbacks.

use super::{ModuleDocDisposition, detect_module_docs_in_span};
use rustc_span::source_map::{FilePathMapping, SourceMap};
use rustc_span::{FileName, Span};

#[test]
fn span_to_snippet_failure_returns_unknown() {
    let source_map = SourceMap::new(FilePathMapping::empty());
    let first =
        source_map.new_source_file(FileName::Custom("first.rs".into()), "mod first {}".into());
    let second =
        source_map.new_source_file(FileName::Custom("second.rs".into()), "mod second {}".into());
    let span = Span::with_root_ctxt(first.start_pos, second.start_pos);
    let disposition = detect_module_docs_in_span(&source_map, span);

    assert_eq!(disposition, ModuleDocDisposition::Unknown);
}
