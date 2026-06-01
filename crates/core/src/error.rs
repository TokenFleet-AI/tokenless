use thiserror::Error;

/// Unified error type for the tokenless core crate.
#[non_exhaustive]
#[derive(Error, Debug)]
pub enum CoreError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Application-level error with a human-readable message.
    #[error("{0}")]
    App(String),

    /// Path validation error.
    #[error("Invalid path: {0}")]
    Path(String),
}

/// Convenience type alias for results with [`CoreError`].
pub type Result<T> = std::result::Result<T, CoreError>;
