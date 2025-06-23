/// Mathematical utilities for embeddings and vector operations.
///
/// # Examples
///
/// ```rust,ignore
/// use lingproc::math::cosine_similarity;
/// let a = [1.0_f32, 0.0, 0.0];
/// let b = [0.5_f32, 0.0, 0.0];
/// assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
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
    fn similarity_basic() {
        let a = [1.0_f32, 2.0, 3.0];
        let b = [1.0_f32, 2.0, 3.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let b = [0.0_f32, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }
}
