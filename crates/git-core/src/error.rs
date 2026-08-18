use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{0}")]
    Git(#[from] git2::Error),
    #[error("{0}")]
    NotARepository(String),
    #[error("{0}")]
    Invalid(String),
}

/// Serialized as a plain message so the UI can show it without unwrapping a tag.
impl Serialize for CoreError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
