//! Common utilities shared across the workspace.
//!
//! Currently provides basic mathematical helpers used by multiple crates.

/// Return trimmed model text unless it is empty or an empty quoted literal.
///
/// Language models sometimes emit `""` or `''` when they mean "nothing"; those
/// should not be treated as meaningful speech or thought.
pub fn non_empty_model_text(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if trimmed.is_empty() || is_empty_quoted_literal(trimmed) {
        None
    } else {
        Some(trimmed)
    }
}

fn is_empty_quoted_literal(text: &str) -> bool {
    let Some((open, close)) = text.chars().next().zip(text.chars().last()) else {
        return false;
    };
    if !matches!((open, close), ('"', '"') | ('\'', '\'') | ('`', '`')) {
        return false;
    }
    let inner_start = open.len_utf8();
    let inner_end = text.len().saturating_sub(close.len_utf8());
    inner_start <= inner_end && text[inner_start..inner_end].trim().is_empty()
}

/// Compute the cosine similarity between two vectors.
///
/// Returns 0.0 if either vector is empty.
///
/// # Examples
/// ```
/// use common::cosine_similarity;
/// let a = [1.0_f32, 0.0, 0.0];
/// let b = [0.5_f32, 0.0, 0.0];
/// assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-4);
/// ```
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b + 1e-5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_similarity_for_empty() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
        assert_eq!(cosine_similarity(&[1.0], &[]), 0.0);
    }

    #[test]
    fn filters_empty_model_text() {
        assert_eq!(non_empty_model_text(""), None);
        assert_eq!(non_empty_model_text("  "), None);
        assert_eq!(non_empty_model_text(r#""""#), None);
        assert_eq!(non_empty_model_text("'  '"), None);
        assert_eq!(non_empty_model_text("` `"), None);
        assert_eq!(non_empty_model_text(" hello "), Some("hello"));
        assert_eq!(non_empty_model_text(r#""hello""#), Some(r#""hello""#));
    }

    #[test]
    fn similarity_basic() {
        let a = [1.0_f32, 2.0, 3.0];
        let b = [1.0_f32, 2.0, 3.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let b = [0.0_f32, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }
}
