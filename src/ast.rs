#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Scalar(String),
    Array(Vec<Value>),
    Object(ObjectNode),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectNode {
    entries: Vec<Entry>,
}

impl ObjectNode {
    pub fn new(entries: Vec<Entry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    key: String,
    value: Value,
    metadata: EntryMetadata,
}

impl Entry {
    pub fn new(key: impl Into<String>, value: Value) -> Self {
        Self {
            key: key.into(),
            value,
            metadata: EntryMetadata::default(),
        }
    }

    pub fn with_metadata(
        key: impl Into<String>,
        value: Value,
        metadata: EntryMetadata,
    ) -> Self {
        Self {
            key: key.into(),
            value,
            metadata,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn metadata(&self) -> &EntryMetadata {
        &self.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut EntryMetadata {
        &mut self.metadata
    }

    pub fn set_metadata(&mut self, metadata: EntryMetadata) {
        self.metadata = metadata;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntryMetadata {
    pub duplicate_index: Option<usize>,
    pub duplicate_suffix: Option<String>,
    pub nested_quoted: bool,
}
