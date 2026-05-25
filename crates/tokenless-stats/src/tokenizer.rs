//! Tokenizer for estimating token counts from text or byte length.

/// Estimate token count from text using a character-based heuristic.
///
/// Uses approximately 4 characters per token for English text.
/// For CJK text (which averages fewer chars per token), this
/// may overestimate — but provides a consistent approximation.
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.chars().count().div_ceil(4)
}

/// Estimate token count from a byte length when text is unavailable.
///
/// Uses approximately 4 bytes per token.  For ASCII text this is
/// equivalent to `estimate_tokens(&str)`; for multi-byte UTF-8 it
/// produces a coarser estimate. Prefer `estimate_tokens` when the
/// text is available.
#[must_use]
pub fn estimate_tokens_from_bytes(bytes: usize) -> usize {
    if bytes == 0 {
        return 0;
    }
    bytes.div_ceil(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_short() {
        // "hello" -> 5 chars, div_ceil(4) = 2
        assert_eq!(estimate_tokens("hello"), 2);
    }

    #[test]
    fn test_estimate_tokens_exact() {
        // "abcd" -> 4 chars, div_ceil(4) = 1
        assert_eq!(estimate_tokens("abcd"), 1);
    }

    #[test]
    fn test_estimate_tokens_from_bytes_zero() {
        assert_eq!(estimate_tokens_from_bytes(0), 0);
    }

    #[test]
    fn test_estimate_tokens_from_bytes_small() {
        assert_eq!(estimate_tokens_from_bytes(5), 2);
    }
}
