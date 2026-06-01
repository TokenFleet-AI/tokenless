/// Validated configuration for tokenless components.
///
/// The `name` field must be non-empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    name: String,
    description: Option<String>,
}

impl Config {
    /// Create a new `Config` with a non-empty name.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::App`] if `name` is empty or whitespace-only.
    pub fn new(name: impl Into<String>) -> crate::Result<Self> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(crate::CoreError::App(
                "config name must not be empty".into(),
            ));
        }
        Ok(Self {
            name,
            description: None,
        })
    }

    /// Set an optional description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Return the configuration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the optional description.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}
