//! SEARCH/REPLACE block parser for surgical inline edits.
//!
//! Format (git-style delimiters, inspired by Aider):
//! ```text
//! <<<<<<< SEARCH
//! fragment_to_find
//! =======
//! replacement_fragment
//! >>>>>>> REPLACE
//! ```
//!
//! Multiple blocks per response are supported. When no blocks are found,
//! the caller falls back to full-text replacement.

use tracing::{debug, info, warn};

/// A single search/replace edit block extracted from an AI response.
#[derive(Debug, Clone, PartialEq)]
pub struct EditBlock {
    pub search: String,
    pub replace: String,
}

/// Errors that can occur when applying edit blocks.
#[derive(Debug, Clone, PartialEq)]
pub enum EditError {
    /// The search text was not found in the original.
    SearchNotFound {
        block_index: usize,
        search_preview: String,
    },
    /// The search text matches multiple locations.
    AmbiguousMatch {
        block_index: usize,
        count: usize,
    },
    /// Two blocks match overlapping regions of the original text.
    OverlappingBlocks {
        block_a: usize,
        block_b: usize,
    },
    /// The edit touches more than MAX_CHANGE_RATIO of the original text.
    ChangeTooLarge {
        ratio: f64,
    },
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::SearchNotFound {
                block_index,
                search_preview,
            } => write!(
                f,
                "Block #{}: search text not found: {:?}",
                block_index, search_preview
            ),
            EditError::AmbiguousMatch { block_index, count } => write!(
                f,
                "Block #{}: search text matches {} locations (ambiguous)",
                block_index, count
            ),
            EditError::OverlappingBlocks { block_a, block_b } => write!(
                f,
                "Blocks #{} and #{} match overlapping regions",
                block_a, block_b
            ),
            EditError::ChangeTooLarge { ratio } => write!(
                f,
                "Edit touches {:.0}% of selection (limit: {:.0}%)",
                ratio * 100.0,
                MAX_CHANGE_RATIO * 100.0
            ),
        }
    }
}

impl std::error::Error for EditError {}

// ── Parsing ──────────────────────────────────────────────────────────────────

const BLOCK_START: &str = "<<<<<<< SEARCH";
const BLOCK_SEPARATOR: &str = "=======";
const BLOCK_END: &str = ">>>>>>> REPLACE";

/// Maximum fraction of the original text that edit blocks may touch.
/// Beyond this threshold the edit is rejected (caller should use full replacement).
const MAX_CHANGE_RATIO: f64 = 0.5;

/// Minimum text length for the size-ratio guard to activate.
/// Short selections are exempt — the ratio is misleading for tiny texts.
const MIN_RATIO_GUARD_BYTES: usize = 200;

/// Parse SEARCH/REPLACE blocks from an AI response.
///
/// Returns `None` if no valid blocks are found (caller should use full replacement).
/// Returns `Some(vec)` with at least one block when parsing succeeds.
pub fn parse_edit_blocks(response: &str) -> Option<Vec<EditBlock>> {
    // Strip markdown fences first — AI may wrap the whole response in ```
    let text = strip_outer_markdown_fence(response);

    let mut blocks = Vec::new();
    let mut remaining = text.as_str();

    while let Some(start_pos) = remaining.find(BLOCK_START) {
        // Move past <<<SEARCH
        let after_start = &remaining[start_pos + BLOCK_START.len()..];
        // Skip optional newline after <<<SEARCH
        let after_start = after_start.strip_prefix('\n').unwrap_or(after_start);

        // Find separator ===
        let Some(sep_pos) = after_start.find(&format!("\n{BLOCK_SEPARATOR}\n")) else {
            // Try without leading newline (separator at start)
            if let Some(sep_pos) = after_start.find(&format!("{BLOCK_SEPARATOR}\n")) {
                if sep_pos == 0 {
                    let search = String::new();
                    let after_sep = &after_start[BLOCK_SEPARATOR.len() + 1..];
                    if let Some(end_pos) = after_sep.find(BLOCK_END) {
                        let replace = after_sep[..end_pos].trim_end_matches('\n').to_string();
                        blocks.push(EditBlock { search, replace });
                        remaining = &after_sep[end_pos + BLOCK_END.len()..];
                        continue;
                    }
                }
            }
            warn!("SEARCH/REPLACE: found <<<SEARCH but no === separator");
            remaining = after_start;
            continue;
        };

        let search = after_start[..sep_pos].to_string();

        // Move past \n===\n
        let after_sep = &after_start[sep_pos + 1 + BLOCK_SEPARATOR.len() + 1..];

        // Find REPLACE>>>
        let Some(end_pos) = after_sep.find(BLOCK_END) else {
            warn!("SEARCH/REPLACE: found === but no REPLACE>>>");
            remaining = after_sep;
            continue;
        };

        let replace = after_sep[..end_pos].trim_end_matches('\n').to_string();
        blocks.push(EditBlock { search, replace });

        remaining = &after_sep[end_pos + BLOCK_END.len()..];
    }

    if blocks.is_empty() {
        debug!("No SEARCH/REPLACE blocks found in AI response");
        None
    } else {
        info!("Parsed {} SEARCH/REPLACE block(s)", blocks.len());
        Some(blocks)
    }
}

// ── Applying ─────────────────────────────────────────────────────────────────

/// Apply edit blocks to the original text.
///
/// Blocks are applied sequentially. Each block's SEARCH text must match exactly
/// one location in the (progressively modified) text.
pub fn apply_edit_blocks(original: &str, blocks: &[EditBlock]) -> Result<String, EditError> {
    let mut result = original.to_string();

    for (i, block) in blocks.iter().enumerate() {
        let preview: String = if block.search.chars().count() > 60 {
            format!("{}...", block.search.chars().take(60).collect::<String>())
        } else {
            block.search.clone()
        };

        // Strategy 1: Exact match
        let count = result.matches(&block.search).count();
        if count == 1 {
            result = result.replacen(&block.search, &block.replace, 1);
            debug!("Block #{}: exact match applied", i);
            continue;
        }
        if count > 1 {
            return Err(EditError::AmbiguousMatch {
                block_index: i,
                count,
            });
        }

        // Strategy 2: Trimmed trailing whitespace per line
        if let Some(new_result) = try_whitespace_normalized_match(&result, block) {
            result = new_result;
            debug!("Block #{}: whitespace-normalized match applied", i);
            continue;
        }

        // Strategy 3: Fuzzy match (only for single-block edits, >80% similarity)
        if blocks.len() == 1 {
            if let Some(new_result) = try_fuzzy_match(&result, block) {
                result = new_result;
                info!("Block #{}: fuzzy match applied (single block)", i);
                continue;
            }
        }

        return Err(EditError::SearchNotFound {
            block_index: i,
            search_preview: preview,
        });
    }

    Ok(result)
}

// ── Validated apply (pre-validation + overlap/size checks) ──────────────────

/// A matched block with its byte range in the original text.
struct MatchedBlock {
    block_index: usize,
    start: usize,
    end: usize,
    replace: String,
}

/// Pre-validate all blocks against the original text, check for overlaps and
/// size limits, then apply all replacements in one pass (last-to-first).
///
/// This is the recommended entry point for inline edit. Unlike `apply_edit_blocks`
/// (which applies sequentially), this validates everything upfront.
pub fn validate_and_apply(original: &str, blocks: &[EditBlock]) -> Result<String, EditError> {
    // 1. Find match positions for every block in the ORIGINAL text.
    let mut matched: Vec<MatchedBlock> = Vec::with_capacity(blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        let (start, end) = find_match_position(original, block, i, blocks.len())?;
        matched.push(MatchedBlock {
            block_index: i,
            start,
            end,
            replace: block.replace.clone(),
        });
    }

    // 2. Sort by start position and check overlaps.
    matched.sort_by_key(|m| m.start);
    for w in matched.windows(2) {
        if w[0].end > w[1].start {
            return Err(EditError::OverlappingBlocks {
                block_a: w[0].block_index,
                block_b: w[1].block_index,
            });
        }
    }

    // 3. Size limit: reject if edit touches >50% of the original.
    //    Skip for short selections — ratio is misleading for small texts.
    if original.len() >= MIN_RATIO_GUARD_BYTES {
        let touched: usize = matched.iter().map(|m| m.end - m.start).sum();
        let ratio = touched as f64 / original.len() as f64;
        if ratio > MAX_CHANGE_RATIO {
            return Err(EditError::ChangeTooLarge { ratio });
        }
    }

    // 4. Apply from last to first (preserves byte offsets of earlier blocks).
    let mut result = original.to_string();
    for m in matched.iter().rev() {
        result.replace_range(m.start..m.end, &m.replace);
    }

    info!(
        "validate_and_apply: {} block(s) applied ({} → {} chars)",
        blocks.len(),
        original.len(),
        result.len()
    );
    Ok(result)
}

/// Find the byte range `(start, end)` of a block's SEARCH text in the original.
fn find_match_position(
    original: &str,
    block: &EditBlock,
    block_index: usize,
    _total_blocks: usize,
) -> Result<(usize, usize), EditError> {
    let preview = make_preview(&block.search);

    // Strategy 1: Exact match
    let count = original.matches(&block.search).count();
    if count == 1 {
        let start = original.find(&block.search).unwrap();
        let end = start + block.search.len();
        debug!("Block #{}: exact match at {}..{}", block_index, start, end);
        return Ok((start, end));
    }
    if count > 1 {
        return Err(EditError::AmbiguousMatch {
            block_index,
            count,
        });
    }

    // Strategy 2: Whitespace-normalized
    if let Some((start, end)) = find_whitespace_normalized_position(original, &block.search) {
        debug!(
            "Block #{}: whitespace-normalized match at {}..{}",
            block_index, start, end
        );
        return Ok((start, end));
    }

    // Strategy 3: Fuzzy match is intentionally EXCLUDED from validate_and_apply.
    // For surgical edits, only exact and whitespace-normalized matches are safe.
    // Fuzzy matches risk applying edits to the wrong location.
    // The legacy `apply_edit_blocks()` still supports fuzzy for backwards compat.

    Err(EditError::SearchNotFound {
        block_index,
        search_preview: preview,
    })
}

/// Find byte range via whitespace-normalized matching.
fn find_whitespace_normalized_position(text: &str, search: &str) -> Option<(usize, usize)> {
    let normalize = |s: &str| -> String {
        s.lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    };

    let norm_text = normalize(text);
    let norm_search = normalize(search);

    if norm_search.is_empty() {
        return None;
    }
    if norm_text.matches(&norm_search).count() != 1 {
        return None;
    }

    let norm_pos = norm_text.find(&norm_search)?;
    let start = map_normalized_pos_to_original(text, norm_pos);
    let end = map_normalized_pos_to_original(text, norm_pos + norm_search.len());
    Some((start, end))
}

/// Truncate a search string for error messages.
fn make_preview(search: &str) -> String {
    if search.chars().count() > 60 {
        format!("{}...", search.chars().take(60).collect::<String>())
    } else {
        search.to_string()
    }
}

// ── Legacy apply (sequential, kept for backwards compat) ────────────────────

/// Try matching after stripping trailing whitespace from each line.
fn try_whitespace_normalized_match(text: &str, block: &EditBlock) -> Option<String> {
    let normalize = |s: &str| -> String {
        s.lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    };

    let norm_text = normalize(text);
    let norm_search = normalize(&block.search);

    if norm_search.is_empty() {
        return None;
    }

    let count = norm_text.matches(&norm_search).count();
    if count != 1 {
        return None;
    }

    // Find the position in normalized text, then map back to original.
    let norm_pos = norm_text.find(&norm_search)?;

    // Count which original byte offset corresponds to norm_pos.
    // Walk both strings in parallel.
    let orig_start = map_normalized_pos_to_original(text, norm_pos);
    let orig_end = map_normalized_pos_to_original(text, norm_pos + norm_search.len());

    let mut result = String::with_capacity(text.len());
    result.push_str(&text[..orig_start]);
    result.push_str(&block.replace);
    result.push_str(&text[orig_end..]);
    Some(result)
}

/// Map a byte position in normalized text back to the original text.
fn map_normalized_pos_to_original(original: &str, norm_pos: usize) -> usize {
    let mut orig_idx = 0;
    let mut norm_idx = 0;

    let orig_bytes = original.as_bytes();
    let orig_len = orig_bytes.len();

    while norm_idx < norm_pos && orig_idx < orig_len {
        if orig_bytes[orig_idx] == b'\n' {
            // In normalized: newline is kept
            orig_idx += 1;
            norm_idx += 1;
        } else if orig_bytes[orig_idx] == b' ' || orig_bytes[orig_idx] == b'\t' {
            // Check if this is trailing whitespace (followed by \n or end)
            let mut peek = orig_idx;
            while peek < orig_len
                && (orig_bytes[peek] == b' ' || orig_bytes[peek] == b'\t')
            {
                peek += 1;
            }
            if peek >= orig_len || orig_bytes[peek] == b'\n' {
                // Trailing whitespace — skipped in normalized
                orig_idx = peek;
            } else {
                // Not trailing — kept in normalized
                orig_idx += 1;
                norm_idx += 1;
            }
        } else {
            orig_idx += 1;
            norm_idx += 1;
        }
    }

    orig_idx
}

/// Try fuzzy matching: if >80% similar, treat as match.
fn try_fuzzy_match(text: &str, block: &EditBlock) -> Option<String> {
    // Simple approach: find the best-matching window of similar length
    let search_len = block.search.len();
    if search_len == 0 || text.len() < search_len {
        return None;
    }

    let search_lines: Vec<&str> = block.search.lines().collect();
    let text_lines: Vec<&str> = text.lines().collect();

    if search_lines.is_empty() || text_lines.len() < search_lines.len() {
        return None;
    }

    let mut best_score = 0.0f64;
    let mut best_start = 0usize;

    // Slide a window of search_lines.len() over text_lines
    for start in 0..=(text_lines.len() - search_lines.len()) {
        let window = &text_lines[start..start + search_lines.len()];
        let score = line_similarity(window, &search_lines);
        if score > best_score {
            best_score = score;
            best_start = start;
        }
    }

    if best_score < 0.8 {
        debug!(
            "Fuzzy match best score {:.2} < 0.80 threshold",
            best_score
        );
        return None;
    }

    info!("Fuzzy match: score={:.2} at line {}", best_score, best_start);

    // Reconstruct: replace the matched window
    let mut result_lines: Vec<&str> = Vec::new();
    result_lines.extend_from_slice(&text_lines[..best_start]);

    // Add replacement lines
    let replace_lines: Vec<&str> = block.replace.lines().collect();
    result_lines.extend_from_slice(&replace_lines);

    result_lines.extend_from_slice(&text_lines[best_start + search_lines.len()..]);

    // Preserve original trailing newline
    let mut result = result_lines.join("\n");
    if text.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }

    Some(result)
}

/// Compute line-by-line similarity between two slices of lines (0.0–1.0).
fn line_similarity(a: &[&str], b: &[&str]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let total: f64 = a
        .iter()
        .zip(b.iter())
        .map(|(la, lb)| str_similarity(la.trim(), lb.trim()))
        .sum();

    total / a.len() as f64
}

/// Simple character-level similarity (Dice coefficient on bigrams).
fn str_similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let chars_a: Vec<char> = a.chars().collect();
    let chars_b: Vec<char> = b.chars().collect();
    if chars_a.len() < 2 || chars_b.len() < 2 {
        return if a == b { 1.0 } else { 0.0 };
    }

    let bigrams_a: Vec<(char, char)> = chars_a.windows(2).map(|w| (w[0], w[1])).collect();
    let bigrams_b: Vec<(char, char)> = chars_b.windows(2).map(|w| (w[0], w[1])).collect();

    let mut matches = 0;
    let mut used = vec![false; bigrams_b.len()];
    for ba in &bigrams_a {
        for (j, bb) in bigrams_b.iter().enumerate() {
            if !used[j] && ba == bb {
                matches += 1;
                used[j] = true;
                break;
            }
        }
    }

    (2 * matches) as f64 / (bigrams_a.len() + bigrams_b.len()) as f64
}

// ── Markdown fence stripping ─────────────────────────────────────────────────

/// Remove outer markdown code fences from AI response.
///
/// Handles patterns like:
/// ```text
/// code here
/// ```
///
/// If the response contains SEARCH/REPLACE blocks inside a fence, the fence
/// is stripped so the parser can find the blocks.
pub fn strip_markdown_fences(text: &str) -> String {
    let trimmed = text.trim();
    // Check for opening ```<lang>\n and closing \n```
    if let Some(rest) = trimmed.strip_prefix("```") {
        // Find end of first line (language tag)
        if let Some(newline_pos) = rest.find('\n') {
            let after_lang = &rest[newline_pos + 1..];
            // Strip closing fence
            if let Some(content) = after_lang.strip_suffix("```") {
                return content.trim_end_matches('\n').to_string();
            }
            // Also handle ``` with trailing whitespace
            let trimmed_end = after_lang.trim_end();
            if let Some(content) = trimmed_end.strip_suffix("```") {
                return content.trim_end_matches('\n').to_string();
            }
        }
    }
    text.to_string()
}

/// Internal: strip outer markdown fence for block parsing (preserves more structure).
fn strip_outer_markdown_fence(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some(newline_pos) = rest.find('\n') {
            let after_lang = &rest[newline_pos + 1..];
            if let Some(content) = after_lang.strip_suffix("```") {
                return content.to_string();
            }
            let trimmed_end = after_lang.trim_end();
            if let Some(content) = trimmed_end.strip_suffix("```") {
                return content.to_string();
            }
        }
    }
    text.to_string()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_block() {
        let response = "\
<<<<<<< SEARCH
fn foo() {
    println!(\"hello\");
}
=======
fn bar() {
    println!(\"hello\");
}
>>>>>>> REPLACE";

        let blocks = parse_edit_blocks(response).expect("should parse");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].search, "fn foo() {\n    println!(\"hello\");\n}");
        assert_eq!(blocks[0].replace, "fn bar() {\n    println!(\"hello\");\n}");
    }

    #[test]
    fn test_parse_multiple_blocks() {
        let response = "\
<<<<<<< SEARCH
let x = 1;
=======
let x = 2;
>>>>>>> REPLACE

<<<<<<< SEARCH
let y = 3;
=======
let y = 4;
>>>>>>> REPLACE";

        let blocks = parse_edit_blocks(response).expect("should parse");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].search, "let x = 1;");
        assert_eq!(blocks[0].replace, "let x = 2;");
        assert_eq!(blocks[1].search, "let y = 3;");
        assert_eq!(blocks[1].replace, "let y = 4;");
    }

    #[test]
    fn test_parse_no_blocks() {
        let response = "Here is the translated text:\n\nHello world!";
        assert!(parse_edit_blocks(response).is_none());
    }

    #[test]
    fn test_parse_blocks_in_markdown_fence() {
        let response = "\
```
<<<<<<< SEARCH
old line
=======
new line
>>>>>>> REPLACE
```";

        let blocks = parse_edit_blocks(response).expect("should parse inside fence");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].search, "old line");
        assert_eq!(blocks[0].replace, "new line");
    }

    #[test]
    fn test_apply_exact_match() {
        let original = "fn foo() {\n    let x = 1;\n    let y = 2;\n}";
        let blocks = vec![EditBlock {
            search: "let x = 1;".to_string(),
            replace: "let x = 42;".to_string(),
        }];

        let result = apply_edit_blocks(original, &blocks).unwrap();
        assert_eq!(result, "fn foo() {\n    let x = 42;\n    let y = 2;\n}");
    }

    #[test]
    fn test_apply_multiple_blocks() {
        let original = "let a = 1;\nlet b = 2;\nlet c = 3;";
        let blocks = vec![
            EditBlock {
                search: "let a = 1;".to_string(),
                replace: "let a = 10;".to_string(),
            },
            EditBlock {
                search: "let c = 3;".to_string(),
                replace: "let c = 30;".to_string(),
            },
        ];

        let result = apply_edit_blocks(original, &blocks).unwrap();
        assert_eq!(result, "let a = 10;\nlet b = 2;\nlet c = 30;");
    }

    #[test]
    fn test_apply_whitespace_mismatch() {
        let original = "fn foo() {  \n    let x = 1;  \n}\n";
        let blocks = vec![EditBlock {
            search: "fn foo() {\n    let x = 1;\n}".to_string(),
            replace: "fn bar() {\n    let x = 1;\n}".to_string(),
        }];

        let result = apply_edit_blocks(original, &blocks).unwrap();
        assert!(result.contains("fn bar()"));
    }

    #[test]
    fn test_apply_search_not_found() {
        let original = "let x = 1;";
        let blocks = vec![EditBlock {
            search: "let y = 2;".to_string(),
            replace: "let y = 3;".to_string(),
        }];

        let err = apply_edit_blocks(original, &blocks).unwrap_err();
        assert!(matches!(err, EditError::SearchNotFound { .. }));
    }

    #[test]
    fn test_apply_ambiguous_match() {
        let original = "let x = 1;\nlet x = 1;";
        let blocks = vec![EditBlock {
            search: "let x = 1;".to_string(),
            replace: "let x = 2;".to_string(),
        }];

        let err = apply_edit_blocks(original, &blocks).unwrap_err();
        assert!(matches!(err, EditError::AmbiguousMatch { count: 2, .. }));
    }

    #[test]
    fn test_apply_empty_replace_deletes_text() {
        let original = "line1\nline_to_remove\nline3";
        let blocks = vec![EditBlock {
            search: "line_to_remove\n".to_string(),
            replace: String::new(),
        }];

        let result = apply_edit_blocks(original, &blocks).unwrap();
        assert_eq!(result, "line1\nline3");
    }

    #[test]
    fn test_strip_markdown_fences_rust() {
        let input = "```rust\nfn main() {}\n```";
        assert_eq!(strip_markdown_fences(input), "fn main() {}");
    }

    #[test]
    fn test_strip_markdown_fences_plain() {
        let input = "```\nhello world\n```";
        assert_eq!(strip_markdown_fences(input), "hello world");
    }

    #[test]
    fn test_strip_markdown_fences_no_fence() {
        let input = "just plain text";
        assert_eq!(strip_markdown_fences(input), "just plain text");
    }

    #[test]
    fn test_strip_preserves_inner_content() {
        let input = "```python\ndef foo():\n    return 42\n```";
        assert_eq!(strip_markdown_fences(input), "def foo():\n    return 42");
    }

    #[test]
    fn test_fuzzy_match_single_block() {
        let original = "fn calculate() {\n    let result = a + b;\n    return result;\n}";
        let blocks = vec![EditBlock {
            search: "fn calculate() {\n    let result = a + b;\n    return result;\n}"
                .to_string(),
            replace: "fn calculate() {\n    let sum = a + b;\n    return sum;\n}".to_string(),
        }];

        let result = apply_edit_blocks(original, &blocks).unwrap();
        assert!(result.contains("let sum = a + b;"));
    }

    #[test]
    fn test_parse_with_surrounding_text() {
        let response = "Sure, here's the change:\n\n<<<<<<< SEARCH\nold code\n=======\nnew code\n>>>>>>> REPLACE\n\nThis renames the function.";

        let blocks = parse_edit_blocks(response).expect("should parse");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].search, "old code");
        assert_eq!(blocks[0].replace, "new code");
    }

    #[test]
    fn test_preview_non_ascii_no_panic() {
        // Ensure preview truncation doesn't panic on multi-byte chars
        let long_polish = "ąęćźżółń ".repeat(10); // ~90 chars, multi-byte
        let blocks = vec![EditBlock {
            search: long_polish,
            replace: "replacement".to_string(),
        }];

        // Should return SearchNotFound, not panic
        let err = apply_edit_blocks("some text", &blocks).unwrap_err();
        assert!(matches!(err, EditError::SearchNotFound { .. }));
    }

    #[test]
    fn test_edit_error_display() {
        let err = EditError::SearchNotFound {
            block_index: 0,
            search_preview: "let x = 1;".to_string(),
        };
        assert!(err.to_string().contains("not found"));

        let err = EditError::AmbiguousMatch {
            block_index: 1,
            count: 3,
        };
        assert!(err.to_string().contains("3 locations"));
    }

    // ── validate_and_apply tests ────────────────────────────────────────────

    #[test]
    fn test_validate_and_apply_single_block() {
        let original = "fn foo() {\n    let x = 1;\n    let y = 2;\n}";
        let blocks = vec![EditBlock {
            search: "let x = 1;".to_string(),
            replace: "let x = 42;".to_string(),
        }];

        let result = validate_and_apply(original, &blocks).unwrap();
        assert_eq!(result, "fn foo() {\n    let x = 42;\n    let y = 2;\n}");
    }

    #[test]
    fn test_validate_and_apply_multi_block() {
        let original = "let a = 1;\nlet b = 2;\nlet c = 3;";
        let blocks = vec![
            EditBlock {
                search: "let a = 1;".to_string(),
                replace: "let a = 10;".to_string(),
            },
            EditBlock {
                search: "let c = 3;".to_string(),
                replace: "let c = 30;".to_string(),
            },
        ];

        let result = validate_and_apply(original, &blocks).unwrap();
        assert_eq!(result, "let a = 10;\nlet b = 2;\nlet c = 30;");
    }

    #[test]
    fn test_validate_and_apply_reverse_order_blocks() {
        // Blocks given in reverse order of their position in text — should still work.
        let original = "AAA\nBBB\nCCC";
        let blocks = vec![
            EditBlock {
                search: "CCC".to_string(),
                replace: "ccc".to_string(),
            },
            EditBlock {
                search: "AAA".to_string(),
                replace: "aaa".to_string(),
            },
        ];

        let result = validate_and_apply(original, &blocks).unwrap();
        assert_eq!(result, "aaa\nBBB\nccc");
    }

    #[test]
    fn test_validate_overlapping_blocks() {
        let original = "abcdefghij";
        let blocks = vec![
            EditBlock {
                search: "bcde".to_string(),
                replace: "BCDE".to_string(),
            },
            EditBlock {
                search: "defg".to_string(),
                replace: "DEFG".to_string(),
            },
        ];

        let err = validate_and_apply(original, &blocks).unwrap_err();
        assert!(matches!(err, EditError::OverlappingBlocks { .. }));
    }

    #[test]
    fn test_validate_change_too_large() {
        // original = 200 bytes (exactly MIN_RATIO_GUARD_BYTES)
        let original = "A".repeat(100) + &"B".repeat(100);
        assert_eq!(original.len(), 200);

        // search = 120 bytes (60% of original) → over 50% limit
        let search = "A".repeat(100) + &"B".repeat(20);
        let replace = "X".repeat(120);
        let blocks = vec![EditBlock { search, replace }];

        let err = validate_and_apply(&original, &blocks).unwrap_err();
        assert!(matches!(err, EditError::ChangeTooLarge { .. }));
    }

    #[test]
    fn test_validate_adjacent_non_overlapping() {
        // Two blocks that are adjacent but don't overlap — should succeed.
        let original = "AABBCC";
        let blocks = vec![
            EditBlock {
                search: "AA".to_string(),
                replace: "aa".to_string(),
            },
            EditBlock {
                search: "CC".to_string(),
                replace: "cc".to_string(),
            },
        ];

        let result = validate_and_apply(original, &blocks).unwrap();
        assert_eq!(result, "aaBBcc");
    }

    #[test]
    fn test_validate_within_size_limit() {
        // Original is 20 chars, editing 8 of them (40%) — under the 50% limit.
        let original = "aaaabbbbccccddddeeee";
        let blocks = vec![EditBlock {
            search: "bbbbcccc".to_string(),
            replace: "BBBBCCCC".to_string(),
        }];

        let result = validate_and_apply(original, &blocks).unwrap();
        assert_eq!(result, "aaaaBBBBCCCCddddeeee");
    }

    #[test]
    fn test_validate_error_display_new_variants() {
        let err = EditError::OverlappingBlocks {
            block_a: 0,
            block_b: 1,
        };
        assert!(err.to_string().contains("overlapping"));

        let err = EditError::ChangeTooLarge { ratio: 0.75 };
        assert!(err.to_string().contains("75%"));
    }
}
