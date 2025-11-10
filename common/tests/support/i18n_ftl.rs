//! Fluent Translation List (FTL) parsing and locale discovery for localisation
//! test suites.
//!
//! This module exposes lightweight newtypes and parsing utilities reused across
//! the localisation quality and behaviour assertions. It understands message
//! declarations, attribute lines, and multi-line continuations so tests can
//! inspect Fluent resources without depending on the runtime loader.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct FtlEntry {
    pub value: String,
    pub attributes: HashMap<String, String>,
}

pub struct LocaleContext<'a> {
    pub locale: &'a str,
    pub path: &'a Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleCode(String);

impl LocaleCode {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for LocaleCode {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl AsRef<str> for LocaleCode {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for LocaleCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageId(String);

impl MessageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for MessageId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for MessageId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for MessageId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttributeName(String);

impl AttributeName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for AttributeName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for AttributeName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for AttributeName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for AttributeName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn locales_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../locales")
}

struct ParseCursor<'state> {
    current_id: &'state mut Option<String>,
    current_attribute: &'state mut Option<String>,
}

impl<'state> ParseCursor<'state> {
    fn new(
        current_id: &'state mut Option<String>,
        current_attribute: &'state mut Option<String>,
    ) -> Self {
        Self {
            current_id,
            current_attribute,
        }
    }

    fn message(&self) -> Option<&str> {
        self.current_id.as_deref()
    }

    fn attribute(&self) -> Option<&str> {
        self.current_attribute.as_deref()
    }

    fn set_message(&mut self, identifier: String) {
        *self.current_id = Some(identifier);
        self.current_attribute.take();
    }

    fn set_attribute(&mut self, attribute: String) {
        *self.current_attribute = Some(attribute);
    }
}

fn compile_regex(pattern: &str, context: &str) -> Regex {
    Regex::new(pattern).unwrap_or_else(|error| panic!("{context}: {error}"))
}

static MESSAGE_RE: Lazy<Regex> = Lazy::new(|| {
    compile_regex(
        r"^([A-Za-z0-9_-]+)\\s*=\\s*(.*)$",
        "message declarations should compile",
    )
});
static ATTRIBUTE_RE: Lazy<Regex> = Lazy::new(|| {
    compile_regex(
        r"^\\s+\\.([A-Za-z0-9_-]+)\\s*=\\s*(.*)$",
        "attribute declarations should compile",
    )
});

pub fn parse_ftl(path: &Path) -> BTreeMap<String, FtlEntry> {
    let mut entries: BTreeMap<String, FtlEntry> = BTreeMap::new();
    let content = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("ftl file should be readable: {error}"));

    let mut current_id: Option<String> = None;
   let mut current_attribute: Option<String> = None;

    for line in content.lines() {
        if {
            let mut cursor = ParseCursor::new(&mut current_id, &mut current_attribute);
            process_message_line(line, &MESSAGE_RE, &mut entries, &mut cursor)
        } {
            continue;
        }

        if {
            let mut cursor = ParseCursor::new(&mut current_id, &mut current_attribute);
            process_attribute_line(line, &ATTRIBUTE_RE, &mut entries, &mut cursor)
        } {
            continue;
        }

        if is_ignorable_line(line) {
            continue;
        }

        process_continuation_line(
            line,
            current_id.as_deref(),
            current_attribute.as_deref(),
            &mut entries,
        );
    }

    entries
}

fn process_message_line(
    line: &str,
    message_re: &Regex,
    entries: &mut BTreeMap<String, FtlEntry>,
    cursor: &mut ParseCursor<'_>,
) -> bool {
    if let Some(captures) = message_re.captures(line) {
        let id = captures[1].to_string();
        let value = captures[2].to_string();
        let entry = entries.entry(id.clone()).or_default();
        entry.value = value;
        cursor.set_message(id);
        return true;
    }

    false
}

fn process_attribute_line(
    line: &str,
    attribute_re: &Regex,
    entries: &mut BTreeMap<String, FtlEntry>,
    cursor: &mut ParseCursor<'_>,
) -> bool {
    if let Some(captures) = attribute_re.captures(line) {
        if let Some(current) = cursor.message() {
            let name = captures[1].to_string();
            let value = captures[2].to_string();
            let entry = entries.entry(current.to_string()).or_default();
            entry.attributes.insert(name.clone(), value);
            cursor.set_attribute(name);
        }
        return true;
    }

    false
}

fn is_ignorable_line(line: &str) -> bool {
    line.trim().is_empty() || line.trim_start().starts_with('#')
}

fn process_continuation_line(
    line: &str,
    current_id: Option<&str>,
    current_attribute: Option<&str>,
    entries: &mut BTreeMap<String, FtlEntry>,
) {
    if let Some(id) = current_id {
        let message_id = MessageId::from(id);
        if let Some(attribute) = current_attribute {
            let attribute = AttributeName::from(attribute);
            append_to_attribute(entries, &message_id, &attribute, line);
        } else {
            append_to_message(entries, &message_id, line);
        }
    }
}

fn append_to_attribute(
    entries: &mut BTreeMap<String, FtlEntry>,
    message_id: &MessageId,
    attribute: &AttributeName,
    line: &str,
) {
    if let Some(text) = entries
        .get_mut(message_id.as_str())
        .and_then(|entry| entry.attributes.get_mut(attribute.as_str()))
    {
        text.push('\n');
        text.push_str(line.trim());
    }
}

fn append_to_message(entries: &mut BTreeMap<String, FtlEntry>, message_id: &MessageId, line: &str) {
    if let Some(entry) = entries.get_mut(message_id.as_str()) {
        entry.value.push('\n');
        entry.value.push_str(line.trim());
    }
}

pub fn secondary_locales() -> Vec<(String, PathBuf)> {
    let root = locales_root();
    let mut locales: Vec<(String, PathBuf)> = fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("locales directory should exist: {error}"))
        .filter_map(|entry| {
            let entry = entry.unwrap_or_else(|error| panic!("valid directory entry: {error}"));
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_dir())
                .and_then(|_| entry.file_name().into_string().ok())
                .and_then(|name| {
                    if name == "en-GB" {
                        None
                    } else {
                        Some((name, entry.path()))
                    }
                })
        })
        .collect();

    locales.sort_by(|left, right| left.0.cmp(&right.0));
    locales
}

pub fn file_pairs() -> Vec<(String, PathBuf, PathBuf)> {
    let en = locales_root().join("en-GB");
    let locales = secondary_locales();
    let mut pairs: Vec<(String, PathBuf, PathBuf)> = Vec::new();

    for entry in
        fs::read_dir(&en).unwrap_or_else(|error| panic!("en-GB locale should exist: {error}"))
    {
        let entry = entry.unwrap_or_else(|error| panic!("valid directory entry: {error}"));
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("ftl") {
            continue;
        }

        let en_path = entry.path();
        let file_name: PathBuf = entry.file_name().into();

        for (locale, directory) in &locales {
            pairs.push((locale.clone(), en_path.clone(), directory.join(&file_name)));
        }
    }

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_id_from_string_reuses_allocation() {
        let original = String::from("message-key");
        let ptr = original.as_ptr();
        let len = original.len();

        let message_id = MessageId::from(original);

        assert!(std::ptr::eq(message_id.as_str().as_ptr(), ptr));
        assert_eq!(message_id.as_str().len(), len);
    }

    #[test]
    fn attribute_name_from_string_reuses_allocation() {
        let original = String::from("attribute-name");
        let ptr = original.as_ptr();
        let len = original.len();

        let attribute_name = AttributeName::from(original);

        assert!(std::ptr::eq(attribute_name.as_str().as_ptr(), ptr));
        assert_eq!(attribute_name.as_str().len(), len);
    }
}
