use crate::common::error::{err_msg, Result};

/// Dot product of two equal-length slices.
pub fn dot_product(a: &[f32], b: &[f32]) -> Result<f32> {
    if a.len() != b.len() {
        return Err(err_msg(format!(
            "dot_product: dimension mismatch {} vs {}",
            a.len(),
            b.len()
        )));
    }
    Ok(a.iter().zip(b.iter()).map(|(x, y)| x * y).sum())
}

/// Euclidean (L2) norm of a vector.
pub fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Returns a unit-length copy of `v`.
///
/// Returns `Err` if `v` is empty or is the zero vector.
pub fn normalize(v: &[f32]) -> Result<Vec<f32>> {
    if v.is_empty() {
        return Err(err_msg("normalize: vector is empty"));
    }
    let norm = l2_norm(v);
    if norm == 0.0 {
        return Err(err_msg("normalize: cannot normalize a zero vector"));
    }
    Ok(v.iter().map(|x| x / norm).collect())
}

/// Cosine similarity between two vectors, in the range `[-1.0, 1.0]`.
///
/// Returns `Err` on dimension mismatch, empty input, or either vector being zero.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32> {
    if a.len() != b.len() {
        return Err(err_msg(format!(
            "cosine_similarity: dimension mismatch {} vs {}",
            a.len(),
            b.len()
        )));
    }
    if a.is_empty() {
        return Err(err_msg("cosine_similarity: vectors are empty"));
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a = l2_norm(a);
    let norm_b = l2_norm(b);
    if norm_a == 0.0 || norm_b == 0.0 {
        return Err(err_msg("cosine_similarity: zero vector has no direction"));
    }
    Ok(dot / (norm_a * norm_b))
}

/// Squared Euclidean distance between two vectors.
pub fn squared_euclidean(a: &[f32], b: &[f32]) -> Result<f32> {
    if a.len() != b.len() {
        return Err(err_msg(format!(
            "squared_euclidean: dimension mismatch {} vs {}",
            a.len(),
            b.len()
        )));
    }
    Ok(a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum())
}

/// Euclidean distance between two vectors.
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> Result<f32> {
    squared_euclidean(a, b).map(f32::sqrt)
}
