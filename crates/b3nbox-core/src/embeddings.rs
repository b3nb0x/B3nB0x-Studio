use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedded {
    pub id: i64,
    pub kind: String,
    pub key: String,
    pub content: String,
    pub created_at: String,
    pub similarity: f32,
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

pub fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn blob_to_vec(blob: &[u8]) -> Vec<f32> {
    let (chunks, _rest) = blob.as_chunks::<4>();
    chunks.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_roundtrip() {
        let a = vec![1.0, 0.0, 1.0];
        let b = vec![1.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.5).abs() < 0.001, "sim = {sim}");

        let blob = vec_to_blob(&a);
        let back = blob_to_vec(&blob);
        assert_eq!(a, back);
        assert_eq!(blob.len(), 12);
    }

    #[test]
    fn identical_vectors_similarity_one() {
        let v = vec![0.2, -0.5, 0.9, 1.1];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
        assert_eq!(cosine_similarity(&v, &[0.0; 4]), 0.0);
    }
}
