use std::path::{Path, PathBuf};

/// A validated relative path that is safe against path traversal attacks.
///
/// Rejects absolute paths, `..` components, null bytes, and other dangerous
/// constructs at construction time.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SafePath(PathBuf);

impl SafePath {
    /// Create a new `SafePath` from a string.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::Path`] if the path:
    /// - is absolute
    /// - contains `..` components
    /// - contains null bytes
    /// - has components longer than 255 bytes
    /// - contains Unicode bidirectional control characters
    pub fn new(path: impl AsRef<Path>) -> crate::Result<Self> {
        let path = path.as_ref();

        if path.is_absolute() {
            return Err(crate::CoreError::Path(
                "absolute paths are not allowed".into(),
            ));
        }

        for component in path.components() {
            use std::path::Component;
            match component {
                Component::ParentDir => {
                    return Err(crate::CoreError::Path(
                        "'..' components are not allowed".into(),
                    ));
                }
                Component::Normal(os_str) => {
                    let s = os_str.to_string_lossy();
                    if s.contains('\0') {
                        return Err(crate::CoreError::Path("null bytes are not allowed".into()));
                    }
                    if os_str.len() > 255 {
                        return Err(crate::CoreError::Path(format!(
                            "path component exceeds 255 bytes: '{s}'"
                        )));
                    }
                    // Reject Unicode bidirectional control characters
                    if s.contains('\u{202A}')
                        || s.contains('\u{202B}')
                        || s.contains('\u{202C}')
                        || s.contains('\u{202D}')
                        || s.contains('\u{202E}')
                        || s.contains('\u{2066}')
                        || s.contains('\u{2067}')
                        || s.contains('\u{2068}')
                        || s.contains('\u{2069}')
                    {
                        return Err(crate::CoreError::Path(
                            "Unicode bidirectional control characters are not allowed".into(),
                        ));
                    }
                }
                _ => {}
            }
        }

        Ok(Self(path.to_path_buf()))
    }

    /// Return the inner path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for SafePath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl std::fmt::Display for SafePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}
