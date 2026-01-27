//! Embedded E5 embedder model - direct include via generated code
//!
//! Release builds: Model files included directly in binary (~1GB)
//! Debug builds: Empty slices, use CODESCRIBE_EMBEDDER_PATH
//!
//! Created by M&K (c)2026 VetCoders

#[cfg(embed_e5)]
mod data {
    include!(concat!(env!("OUT_DIR"), "/embedded_e5_data.rs"));
}

#[cfg(not(embed_e5))]
mod data {
    pub static CONFIG: &[u8] = &[];
    pub static TOKENIZER: &[u8] = &[];
    pub static WEIGHTS: &[u8] = &[];
}

/// Check if embedded E5 model is available
///
/// Note: We only check weights_size, not cfg!(embed_e5).
/// The cfg!() macro can return false in workspace builds even when
/// the data was correctly embedded via #[cfg(embed_e5)].
/// If weights exist (len > 0), the model is available.
pub fn is_embedded_available() -> bool {
    let weights_size = data::WEIGHTS.len();
    tracing::debug!("[E5] Embedded check: weights_size={}", weights_size);
    weights_size > 0
}

/// Get embedded model data if available
pub fn get_embedded_data() -> Option<EmbeddedE5> {
    if !is_embedded_available() {
        return None;
    }
    Some(EmbeddedE5 {
        config: data::CONFIG,
        tokenizer: data::TOKENIZER,
        weights: data::WEIGHTS,
    })
}

/// Embedded E5 model data - static byte slices from binary
pub struct EmbeddedE5 {
    /// E5 model configuration (config.json)
    pub config: &'static [u8],
    /// Tokenizer (tokenizer.json)
    pub tokenizer: &'static [u8],
    /// Model weights (model.safetensors)
    pub weights: &'static [u8],
}

impl EmbeddedE5 {
    /// Total size in bytes
    pub fn total_size(&self) -> usize {
        self.config.len() + self.tokenizer.len() + self.weights.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_availability() {
        let available = is_embedded_available();
        println!("Embedded E5 available: {}", available);

        if available {
            let model = get_embedded_data().unwrap();
            println!(
                "E5 model size: {:.1} MB",
                model.total_size() as f64 / 1_000_000.0
            );
        }
    }
}
