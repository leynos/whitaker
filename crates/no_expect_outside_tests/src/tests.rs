use crate::context::summarise_context;
use common::attributes::{Attribute, AttributeKind, AttributePath};
use common::{ContextEntry, ContextKind};
use rstest::rstest;

fn function_entry(name: &str, attrs: Vec<Attribute>) -> ContextEntry {
    ContextEntry::new(name, ContextKind::Function, attrs)
}

fn module_entry(name: &str, attrs: Vec<Attribute>) -> ContextEntry {
    ContextEntry::new(name, ContextKind::Module, attrs)
}

fn test_attribute() -> Attribute {
    Attribute::new(AttributePath::from("test"), AttributeKind::Outer)
}

#[rstest]
fn summarises_plain_context() {
    let entries = vec![function_entry("handler", Vec::new())];
    let summary = summarise_context(&entries, false);

    assert!(!summary.is_test);
    assert_eq!(summary.function_name.as_deref(), Some("handler"));
}

#[rstest]
fn recognises_test_attribute() {
    let entries = vec![function_entry("test_case", vec![test_attribute()])];
    let summary = summarise_context(&entries, false);

    assert!(summary.is_test);
    assert_eq!(summary.function_name.as_deref(), Some("test_case"));
}

#[rstest]
fn honours_cfg_test() {
    let entries = vec![module_entry("tests", Vec::new())];
    let summary = summarise_context(&entries, true);

    assert!(summary.is_test);
    assert_eq!(summary.function_name, None);
}
