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

pub fn parse_ftl(path: &Path) -> BTreeMap<String, FtlEntry> {
    let mut entries: BTreeMap<String, FtlEntry> = BTreeMap::new();
    let content = fs::read_to_string(path).expect("ftl file should be readable");
    let message_re = Regex::new(r"^([A-Za-z0-9_-]+)\\s*=\\s*(.*)$").expect("valid message regex");
    let attribute_re =
        Regex::new(r"^\\s+\\.([A-Za-z0-9_-]+)\\s*=\\s*(.*)$").expect("valid attribute regex");

    let mut current_id: Option<String> = None;
    let mut current_attribute: Option<String> = None;

    for line in content.lines() {
        if process_message_line(
            line,
            &message_re,
            &mut entries,
            &mut current_id,
            &mut current_attribute,
        ) {
            continue;
        }

        if process_attribute_line(
            line,
            &attribute_re,
            &mut entries,
            &current_id,
            &mut current_attribute,
        ) {
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
    current_id: &mut Option<String>,
    current_attribute: &mut Option<String>,
) -> bool {
    if let Some(captures) = message_re.captures(line) {
        let id = captures[1].to_string();
        let value = captures[2].to_string();
        let entry = entries.entry(id.clone()).or_default();
        entry.value = value;
        *current_id = Some(id);
        *current_attribute = None;
        return true;
    }

    false
}

fn process_attribute_line(
    line: &str,
    attribute_re: &Regex,
    entries: &mut BTreeMap<String, FtlEntry>,
    current_id: &Option<String>,
    current_attribute: &mut Option<String>,
) -> bool {
    if let Some(captures) = attribute_re.captures(line) {
        if let Some(current) = current_id.as_ref() {
            let name = captures[1].to_string();
            let value = captures[2].to_string();
            entries
                .entry(current.clone())
                .or_default()
                .attributes
                .insert(name.clone(), value);
            *current_attribute = Some(name);
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
    if let Some(entry) = entries.get_mut(message_id.as_str()) {
        if let Some(text) = entry.attributes.get_mut(attribute.as_str()) {
            text.push('\n');
            text.push_str(line.trim());
        }
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
        .expect("locales directory should exist")
        .filter_map(|entry| {
            let entry = entry.expect("valid directory entry");
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

    for entry in fs::read_dir(&en).expect("en-GB locale should exist") {
        let entry = entry.expect("valid directory entry");
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
