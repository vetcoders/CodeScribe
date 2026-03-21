#![allow(dead_code)]

pub use qube_audio::{audio, vad};
pub use qube_stt::stt;
pub use qube_support::{config, safe_path};

pub mod ai_formatting {
    pub fn has_repetition_loop(text: &str) -> bool {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() < 4 {
            return false;
        }

        let mut consecutive_count = 1;
        for i in 1..words.len() {
            if words[i].eq_ignore_ascii_case(words[i - 1]) {
                consecutive_count += 1;
                if consecutive_count >= 3 {
                    return true;
                }
            } else {
                consecutive_count = 1;
            }
        }

        for pattern_len in 1..=3 {
            if words.len() < pattern_len * 3 {
                continue;
            }

            for i in 0..=words.len() - pattern_len {
                let pattern = &words[i..i + pattern_len];
                let mut repeat_count = 1;
                let mut j = i + pattern_len;

                while j + pattern_len <= words.len() {
                    let next = &words[j..j + pattern_len];
                    if pattern
                        .iter()
                        .zip(next.iter())
                        .all(|(a, b)| a.eq_ignore_ascii_case(b))
                    {
                        repeat_count += 1;
                        j += pattern_len;
                    } else {
                        break;
                    }
                }

                if repeat_count >= 3 {
                    return true;
                }
            }
        }

        false
    }

    pub fn remove_simple_repetitions(text: &str) -> String {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return text.to_string();
        }

        let normalize_word = |word: &str| -> String {
            word.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        };

        let mut result = Vec::new();
        let mut i = 0;
        while i < words.len() {
            let mut best_pattern_len = 1;
            let mut best_repeat_count = 1;

            for pattern_len in (1..=3).rev() {
                if i + pattern_len > words.len() {
                    continue;
                }

                let pattern: Vec<String> = words[i..i + pattern_len]
                    .iter()
                    .map(|word| normalize_word(word))
                    .collect();
                let mut repeat_count = 1;
                let mut j = i + pattern_len;

                while j + pattern_len <= words.len() {
                    let next: Vec<String> = words[j..j + pattern_len]
                        .iter()
                        .map(|word| normalize_word(word))
                        .collect();
                    if pattern == next {
                        repeat_count += 1;
                        j += pattern_len;
                    } else {
                        break;
                    }
                }

                if repeat_count >= 2
                    && (pattern_len > best_pattern_len || repeat_count > best_repeat_count)
                {
                    best_pattern_len = pattern_len;
                    best_repeat_count = repeat_count;
                }
            }

            result.extend(
                words[i..i + best_pattern_len]
                    .iter()
                    .map(|word| word.trim_end_matches(',').to_string()),
            );
            i += best_pattern_len * best_repeat_count;
        }

        result.join(" ")
    }
}

pub mod embedder {
    pub fn embed(_text: &str) -> anyhow::Result<Vec<f32>> {
        anyhow::bail!("embedding backend not linked into qube-ws")
    }
}

pub mod pipeline;
pub mod qube;
