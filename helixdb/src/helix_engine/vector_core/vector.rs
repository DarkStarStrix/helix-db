use std::cmp::Ordering;
use serde::{Deserialize, Serialize}; // vector struct to store raw data, dimension and de
use crate::helix_engine::types::VectorError;

#[repr(C, align(16))]
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct HVector {
    id: String,
    pub is_deleted: bool,
    pub level: usize,
    pub distance: f64,
    data: Vec<f64>,
}

impl Eq for HVector {}

impl PartialOrd for HVector {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.distance.partial_cmp(&self.distance)
    }
}

impl Ord for HVector {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

pub trait EuclidianDistance {
    fn distance(from: &HVector, to: &HVector) -> f64;
}

impl EuclidianDistance for HVector {
    #[inline(always)]
    fn distance(from: &HVector, to: &HVector) -> f64 {
        if from.len() == to.len() {
            #[cfg(target_arch = "aarch64")]
            unsafe { return from.simd_distance_unchecked(to); }
            #[cfg(not(target_arch = "aarch64"))]
            return from.scalar_distance(to);
        }
        from.scalar_distance(to)
    }
}

impl HVector {
    #[inline(always)]
    pub fn new(id: String, data: Vec<f64>) -> Self {
        HVector {
            id,
            is_deleted: false,
            level: 0,
            data,
            distance: 0.0,
        }
    }

    #[inline(always)]
    pub fn from_slice(id: String, level: usize, data: Vec<f64>) -> Self {
        HVector {
            id,
            is_deleted: false,
            level,
            data,
            distance: 0.0,
        }
    }

    #[inline(always)]
    pub fn get_data(&self) -> &[f64] {
        &self.data
    }

    #[inline(always)]
    pub fn get_id(&self) -> &str {
        &self.id
    }

    #[inline(always)]
    pub fn get_level(&self) -> usize {
        self.level
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let size = self.data.len() * std::mem::size_of::<f64>();
        let mut bytes = Vec::with_capacity(size);
        for &value in &self.data {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    pub fn from_bytes(id: String, level: usize, bytes: &[u8]) -> Result<Self, VectorError> {
        if bytes.len() % std::mem::size_of::<f64>() != 0 {
            return Err(VectorError::InvalidVectorData);
        }

        let mut data = Vec::with_capacity(bytes.len() / std::mem::size_of::<f64>());
        let chunks = bytes.chunks_exact(std::mem::size_of::<f64>());

        for chunk in chunks {
            let value = f64::from_le_bytes(chunk.try_into().unwrap());
            data.push(value);
        }

        Ok(HVector {
            id,
            is_deleted: false,
            level,
            data,
            distance: 0.0,
        })
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[inline(always)]
    pub fn distance_to(&self, other: &HVector) -> f64 {
        HVector::distance(self, other)
    }

    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    unsafe fn simd_distance_unchecked(&self, other: &HVector) -> f64 {
        use std::arch::aarch64::{vaddvq_f64, vld1q_f64, vmulq_f64, vsubq_f64};

        let mut sum = 0.0;
        let n = self.len();
        let mut i = 0;

        while i + 2 <= n {
            let a = vld1q_f64(self.data[i..].as_ptr());
            let b = vld1q_f64(other.data[i..].as_ptr());
            let diff = vsubq_f64(a, b);
            let squared = vmulq_f64(diff, diff);
            sum += vaddvq_f64(squared);
            i += 2;
        }

        while i < n {
            let diff = self.data[i] - other.data[i];
            sum += diff * diff;
            i += 1;
        }

        sum.sqrt()
    }

    #[inline(always)]
    fn scalar_distance(&self, other: &HVector) -> f64 {
        self.data
            .iter()
            .zip(other.data.iter())
            .map(|(&a, &b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// (pooling operation reduce)
    #[inline(always)]
    pub fn reduce_dims(&mut self, target_dim: usize) {
        let chunk_size = (self.data.len() as f64 / target_dim as f64).ceil() as usize;
        let mut reduced = Vec::with_capacity(target_dim);

        for chunk_idx in 0..target_dim {
            let start = chunk_idx * chunk_size;
            let end = (start + chunk_size).min(self.data.len());

            if start >= self.data.len() {
                break;
            }

            let avg = self.data[start..end].iter().sum::<f64>() / (end - start) as f64;
            reduced.push(avg);
        }

        self.data = reduced;
    }

    // TODO: scale up again
}

#[cfg(test)]
mod vector_tests {
    use super::*;

    #[test]
    fn test_hvector_new() {
        let data = vec![1.0, 2.0, 3.0];
        let vector = HVector::new("test".to_string(), data);
        assert_eq!(vector.get_data(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_hvector_from_slice() {
        let data = [1.0, 2.0, 3.0];
        let vector = HVector::from_slice("test".to_string(), 0, data.to_vec());
        assert_eq!(vector.get_data(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_hvector_distance() {
        let v1 = HVector::new("test".to_string(), vec![1.0, 0.0]);
        let v2 = HVector::new("test".to_string(), vec![0.0, 1.0]);
        let distance = HVector::distance(&v1, &v2);
        assert!((distance - 2.0_f64.sqrt()).abs() < 1e-10);
    }

    #[test]
    fn test_hvector_distance_zero() {
        let v1 = HVector::new("test".to_string(), vec![1.0, 2.0, 3.0]);
        let v2 = HVector::new("test".to_string(), vec![1.0, 2.0, 3.0]);
        let distance = HVector::distance(&v1, &v2);
        assert!(distance.abs() < 1e-10);
    }

    #[test]
    fn test_hvector_distance_to() {
        let v1 = HVector::new("test".to_string(), vec![0.0, 0.0]);
        let v2 = HVector::new("test".to_string(), vec![3.0, 4.0]);
        let distance = v1.distance_to(&v2);
        assert!((distance - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_bytes_roundtrip() {
        let original = HVector::new("test".to_string(), vec![1.0, 2.0, 3.0]);
        let bytes = original.to_bytes();
        let reconstructed = HVector::from_bytes("test".to_string(), 0, &bytes).unwrap();
        assert_eq!(original.get_data(), reconstructed.get_data());
    }

    #[test]
    fn test_hvector_len() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let vector = HVector::new("test".to_string(), data);
        assert_eq!(vector.len(), 4);
    }

    #[test]
    fn test_hvector_is_empty() {
        let empty_vector = HVector::new("test".to_string(), vec![]);
        let non_empty_vector = HVector::new("test".to_string(), vec![1.0, 2.0]);

        assert!(empty_vector.is_empty());
        assert!(!non_empty_vector.is_empty());
    }

    #[test]
    fn test_hvector_distance_different_dimensions() {
        let v1 = HVector::new("test".to_string(), vec![1.0, 2.0, 3.0]);
        let v2 = HVector::new("test".to_string(), vec![1.0, 2.0, 3.0, 4.0]);
        let distance = HVector::distance(&v1, &v2);
        assert!(distance.is_finite());
    }

    #[test]
    fn test_hvector_large_values() {
        let v1 = HVector::new("test".to_string(), vec![1e6, 2e6]);
        let v2 = HVector::new("test".to_string(), vec![1e6, 2e6]);
        let distance = HVector::distance(&v1, &v2);
        assert!(distance.abs() < 1e-10);
    }

    #[test]
    fn test_hvector_negative_values() {
        let v1 = HVector::new("test".to_string(), vec![-1.0, -2.0]);
        let v2 = HVector::new("test".to_string(), vec![1.0, 2.0]);
        let distance = HVector::distance(&v1, &v2);
        assert!((distance - (20.0_f64).sqrt()).abs() < 1e-10);
    }
}


