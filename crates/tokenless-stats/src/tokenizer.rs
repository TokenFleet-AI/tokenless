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

/// Estimate token count with CJK-aware character weighting.
///
/// Unlike [`estimate_tokens`] (which uses a flat 4-char-per-token
/// ratio), this function assigns 1 token per CJK character and
/// groups ASCII runs by the standard 4-char-per-token heuristic.
/// CJK ranges include: CJK Unified Ideographs, CJK Extension A,
/// CJK Compatibility Ideographs, Hiragana, Katakana, and Hangul.
#[must_use]
pub fn estimate_tokens_cjk_aware(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let mut tokens = 0usize;
    let mut ascii_run = 0usize;
    for ch in text.chars() {
        if is_cjk_char(ch) {
            tokens += ascii_run.div_ceil(4);
            ascii_run = 0;
            tokens += 1;
        } else {
            ascii_run += 1;
        }
    }
    tokens += ascii_run.div_ceil(4);
    tokens
}

/// Returns `true` if the character falls within common CJK ranges.
fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{AC00}'..='\u{D7AF}'
    )
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

    #[test]
    fn test_estimate_tokens_cjk_aware_empty() {
        assert_eq!(estimate_tokens_cjk_aware(""), 0);
    }

    #[test]
    fn test_estimate_tokens_cjk_aware_english() {
        // "hello world" = 11 chars, div_ceil(4) = 3
        assert_eq!(estimate_tokens_cjk_aware("hello world"), 3);
    }

    #[test]
    fn test_estimate_tokens_cjk_aware_cjk() {
        // each CJK char = 1 token
        assert_eq!(estimate_tokens_cjk_aware("你好世界"), 4);
    }

    #[test]
    fn test_estimate_tokens_cjk_aware_mixed() {
        // "hello" (5 ascii -> 2 tokens) + "你好" (2 CJK -> 2 tokens) + "world" (5 ascii -> 2 tokens) = 6
        assert_eq!(estimate_tokens_cjk_aware("hello你好world"), 6);
    }

    #[test]
    fn test_is_cjk_char() {
        assert!(is_cjk_char('中'));
        assert!(is_cjk_char('あ'));
        assert!(is_cjk_char('カ'));
        assert!(is_cjk_char('한'));
        assert!(!is_cjk_char('a'));
        assert!(!is_cjk_char('1'));
        assert!(!is_cjk_char(' '));
    }
}
