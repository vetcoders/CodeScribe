//! Whisper timestamp token helpers.
//!
//! Resolves timestamp token ranges directly from the tokenizer and parses
//! decoder token streams into segment-level timestamps.

use tokenizers::Tokenizer;

use crate::pipeline::contracts::TranscriptSegment;

/// Timestamp token range resolved from tokenizer special tokens.
#[derive(Debug, Clone, Copy)]
pub struct TimestampRange {
    /// Token ID for `<|0.00|>`.
    pub begin: u32,
    /// Token ID for `<|30.00|>`.
    pub end_inclusive: u32,
}

impl TimestampRange {
    /// Resolve the timestamp token range from tokenizer special tokens.
    pub fn from_tokenizer(tokenizer: &Tokenizer) -> Option<Self> {
        let begin = tokenizer.token_to_id("<|0.00|>")?;
        let end_inclusive = tokenizer.token_to_id("<|30.00|>")?;
        Some(Self {
            begin,
            end_inclusive,
        })
    }

    /// Returns true when `tok` is a timestamp token.
    pub fn is_timestamp(&self, tok: u32) -> bool {
        tok >= self.begin && tok <= self.end_inclusive
    }

    /// Converts timestamp token ID to seconds (Whisper: 20ms step).
    pub fn to_seconds(&self, tok: u32) -> f32 {
        (tok.saturating_sub(self.begin)) as f32 * 0.02
    }
}

/// Parse decoder token output into final text + segment-level timestamps.
pub fn extract_segments(
    all_tokens: &[u32],
    tokenizer: &Tokenizer,
    ts_range: &TimestampRange,
) -> (String, Vec<TranscriptSegment>) {
    let mut segments = Vec::new();
    let mut current_start: Option<f32> = None;
    let mut current_tokens: Vec<u32> = Vec::new();

    for &tok in all_tokens {
        if ts_range.is_timestamp(tok) {
            let time = ts_range.to_seconds(tok);
            match current_start {
                None => {
                    current_start = Some(time);
                }
                Some(start) => {
                    if !current_tokens.is_empty()
                        && let Ok(text) = tokenizer.decode(&current_tokens, true)
                    {
                        let text = text.trim().to_string();
                        if !text.is_empty() {
                            segments.push(TranscriptSegment {
                                text,
                                start_ts: start,
                                end_ts: time,
                            });
                        }
                    }
                    current_tokens.clear();
                    current_start = Some(time);
                }
            }
        } else {
            current_tokens.push(tok);
        }
    }

    if !current_tokens.is_empty()
        && let Ok(text) = tokenizer.decode(&current_tokens, true)
    {
        let text = text.trim().to_string();
        if !text.is_empty() {
            let start = current_start.unwrap_or(0.0);
            segments.push(TranscriptSegment {
                text,
                start_ts: start,
                end_ts: start,
            });
        }
    }

    let text_tokens: Vec<u32> = all_tokens
        .iter()
        .filter(|&&tok| !ts_range.is_timestamp(tok))
        .copied()
        .collect();
    let full_text = tokenizer.decode(&text_tokens, true).unwrap_or_default();

    (full_text, segments)
}
