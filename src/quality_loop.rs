//! Self-improving quality loop for batch transcription evaluation.
//!
//! Flow: batch -> report -> regression analysis -> tuning updates -> re-run later.
//!
//! Created by M&K (c)2026 VetCoders

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::quality_report::{QualityReport, QualityReportConfig, ReportSummary};
use crate::safe_path::{safe_canonicalize, safe_read_to_string};

const DEFAULT_REGRESSION_THRESHOLD: f32 = 0.02;
const DEFAULT_SIMILARITY: f32 = 0.93;
const DEFAULT_NOVELTY: f32 = 0.12;

#[derive(Debug, Clone)]
pub struct QualityLoopConfig {
    pub report_config: QualityReportConfig,
    pub baseline_report: Option<PathBuf>,
    pub history_path: PathBuf,
    pub regression_threshold: f32,
    pub apply_updates: bool,
    pub update_lexicon: bool,
    pub update_gate: bool,
    pub update_prompts: bool,
    pub update_embeddings: bool,
    pub max_lexicon_updates: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoopAnalysis {
    pub generated_at: String,
    pub current_report: String,
    pub baseline_report: Option<String>,
    pub summary: LoopSummary,
    pub regressions: Vec<RegressionFinding>,
    pub updates: Vec<UpdateAction>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoopSummary {
    pub total_entries: usize,
    pub compared_entries: usize,
    pub regression_count: usize,
    pub improvement_count: usize,
    pub post_worse_ratio: Option<f32>,
    pub ai_worse_ratio: Option<f32>,
    pub gate_drop_rate: Option<f32>,
    pub suspicious_rate: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegressionFinding {
    pub id: String,
    pub metric: String,
    pub current: f32,
    pub baseline: f32,
    pub delta: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateAction {
    pub kind: String,
    pub detail: String,
    pub applied: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoopHistoryEntry {
    generated_at: String,
    report_dir: String,
    report_json: String,
    summary: ReportSummary,
}

pub async fn run(config: QualityLoopConfig) -> Result<PathBuf> {
    let report_config = config.report_config.clone();
    let output_dir = crate::quality_report::run(report_config).await?;
    let report_path = output_dir.join("report.json");
    let report = load_report(&report_path)?;

    let baseline_path = resolve_baseline(&config, &output_dir);
    let baseline_report = baseline_path
        .as_ref()
        .and_then(|path| load_report(path).ok());

    let (regressions, regression_summary) = analyze_regressions(
        &report,
        baseline_report.as_ref(),
        config.regression_threshold,
    );

    let mut updates = Vec::new();
    let signals = QualitySignals::from_report(&report, config.regression_threshold);
    let postprocess_stats = PostprocessStats::from_report(&report);

    if config.update_gate
        && let Some(update) =
            propose_gate_update(&signals, &postprocess_stats, config.apply_updates)?
    {
        updates.push(update);
    }

    if config.update_embeddings
        && let Some(update) =
            propose_embedding_update(&signals, &postprocess_stats, config.apply_updates)?
    {
        updates.push(update);
    }

    if config.update_prompts
        && let Some(update) = propose_prompt_tuning(&signals, &report, config.apply_updates)?
    {
        updates.push(update);
    }

    if config.update_lexicon
        && let Some(update) =
            propose_lexicon_updates(&report, config.max_lexicon_updates, config.apply_updates)?
    {
        updates.push(update);
    }

    let analysis = LoopAnalysis {
        generated_at: Local::now().to_rfc3339(),
        current_report: report_path.to_string_lossy().to_string(),
        baseline_report: baseline_path.map(|p| p.to_string_lossy().to_string()),
        summary: regression_summary,
        regressions,
        updates,
    };

    write_analysis_files(&output_dir, &analysis)?;
    append_history(&config.history_path, &report, &output_dir)?;

    Ok(output_dir)
}

fn load_report(path: &Path) -> Result<QualityReport> {
    let data = safe_read_to_string(path)
        .with_context(|| format!("Failed to read report {}", path.display()))?;
    serde_json::from_str(&data).context("Failed to parse report.json")
}

fn resolve_baseline(config: &QualityLoopConfig, output_dir: &Path) -> Option<PathBuf> {
    if let Some(path) = config.baseline_report.as_ref() {
        return Some(resolve_report_path(path));
    }

    let history = read_last_history(&config.history_path)?;
    let history_path = PathBuf::from(&history.report_json);
    if history_path.exists() && history_path != output_dir.join("report.json") {
        return Some(history_path);
    }

    None
}

fn resolve_report_path(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join("report.json")
    } else {
        path.to_path_buf()
    }
}

fn read_last_history(path: &Path) -> Option<LoopHistoryEntry> {
    let content = safe_read_to_string(path).ok()?;
    for line in content.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<LoopHistoryEntry>(trimmed) {
            return Some(entry);
        }
    }
    None
}

fn append_history(path: &Path, report: &QualityReport, output_dir: &Path) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let entry = LoopHistoryEntry {
        generated_at: report.generated_at.clone(),
        report_dir: output_dir.to_string_lossy().to_string(),
        report_json: output_dir.join("report.json").to_string_lossy().to_string(),
        summary: report.summary.clone(),
    };

    let line = serde_json::to_string(&entry)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

fn write_analysis_files(output_dir: &Path, analysis: &LoopAnalysis) -> Result<()> {
    let json_path = output_dir.join("analysis.json");
    let md_path = output_dir.join("analysis.md");

    let json = serde_json::to_string_pretty(analysis)?;
    fs::write(json_path, json)?;

    let md = render_analysis_markdown(analysis);
    fs::write(md_path, md)?;

    Ok(())
}

fn render_analysis_markdown(analysis: &LoopAnalysis) -> String {
    let mut out = String::new();
    out.push_str("# CodeScribe Quality Loop Analysis\n\n");
    out.push_str(&format!("Generated: {}\n\n", analysis.generated_at));
    out.push_str(&format!("- Current report: {}\n", analysis.current_report));
    if let Some(baseline) = &analysis.baseline_report {
        out.push_str(&format!("- Baseline report: {}\n", baseline));
    }

    out.push_str("\n## Summary\n\n");
    out.push_str(&format!(
        "- Entries compared: {}/{}\n",
        analysis.summary.compared_entries, analysis.summary.total_entries
    ));
    out.push_str(&format!(
        "- Regressions: {}, Improvements: {}\n",
        analysis.summary.regression_count, analysis.summary.improvement_count
    ));

    if let Some(rate) = analysis.summary.post_worse_ratio {
        out.push_str(&format!("- Post worse ratio: {:.2}\n", rate));
    }
    if let Some(rate) = analysis.summary.ai_worse_ratio {
        out.push_str(&format!("- AI worse ratio: {:.2}\n", rate));
    }
    if let Some(rate) = analysis.summary.gate_drop_rate {
        out.push_str(&format!("- Gate drop rate: {:.2}\n", rate));
    }
    if let Some(rate) = analysis.summary.suspicious_rate {
        out.push_str(&format!("- Suspicious rate: {:.2}\n", rate));
    }

    if !analysis.regressions.is_empty() {
        out.push_str("\n## Regressions\n\n");
        out.push_str("| ID | Metric | Current | Baseline | Delta |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
        for reg in analysis.regressions.iter().take(50) {
            out.push_str(&format!(
                "| {} | {} | {:.3} | {:.3} | {:.3} |\n",
                reg.id, reg.metric, reg.current, reg.baseline, reg.delta
            ));
        }
    }

    if !analysis.updates.is_empty() {
        out.push_str("\n## Updates\n\n");
        for update in &analysis.updates {
            out.push_str(&format!(
                "- {}: {} (applied={})\n",
                update.kind, update.detail, update.applied
            ));
        }
    }

    out
}

fn analyze_regressions(
    report: &QualityReport,
    baseline: Option<&QualityReport>,
    threshold: f32,
) -> (Vec<RegressionFinding>, LoopSummary) {
    let mut regressions = Vec::new();
    let mut improvements = 0usize;
    let mut compared = 0usize;

    if let Some(base) = baseline {
        let base_map = base
            .entries
            .iter()
            .map(|entry| (entry.id.clone(), entry))
            .collect::<HashMap<_, _>>();

        for entry in &report.entries {
            let Some(base_entry) = base_map.get(&entry.id) else {
                continue;
            };
            compared += 1;
            compare_metric(
                &entry.id,
                "raw_wer",
                entry.metrics.raw_wer,
                base_entry.metrics.raw_wer,
                threshold,
                &mut regressions,
                &mut improvements,
            );
            compare_metric(
                &entry.id,
                "post_wer",
                entry.metrics.post_wer,
                base_entry.metrics.post_wer,
                threshold,
                &mut regressions,
                &mut improvements,
            );
            compare_metric(
                &entry.id,
                "ai_wer",
                entry.metrics.ai_wer,
                base_entry.metrics.ai_wer,
                threshold,
                &mut regressions,
                &mut improvements,
            );
            compare_metric(
                &entry.id,
                "cloud_wer",
                entry.metrics.cloud_wer,
                base_entry.metrics.cloud_wer,
                threshold,
                &mut regressions,
                &mut improvements,
            );
            compare_metric(
                &entry.id,
                "raw_cer",
                entry.metrics.raw_cer,
                base_entry.metrics.raw_cer,
                threshold,
                &mut regressions,
                &mut improvements,
            );
            compare_metric(
                &entry.id,
                "post_cer",
                entry.metrics.post_cer,
                base_entry.metrics.post_cer,
                threshold,
                &mut regressions,
                &mut improvements,
            );
            compare_metric(
                &entry.id,
                "ai_cer",
                entry.metrics.ai_cer,
                base_entry.metrics.ai_cer,
                threshold,
                &mut regressions,
                &mut improvements,
            );
            compare_metric(
                &entry.id,
                "cloud_cer",
                entry.metrics.cloud_cer,
                base_entry.metrics.cloud_cer,
                threshold,
                &mut regressions,
                &mut improvements,
            );
        }
    }

    let signals = QualitySignals::from_report(report, threshold);
    let post_stats = PostprocessStats::from_report(report);

    let summary = LoopSummary {
        total_entries: report.entries.len(),
        compared_entries: compared,
        regression_count: regressions.len(),
        improvement_count: improvements,
        post_worse_ratio: signals.post_worse_ratio,
        ai_worse_ratio: signals.ai_worse_ratio,
        gate_drop_rate: post_stats.gate_drop_rate(),
        suspicious_rate: post_stats.suspicious_rate(),
    };

    (regressions, summary)
}

fn compare_metric(
    id: &str,
    metric: &str,
    current: Option<f32>,
    baseline: Option<f32>,
    threshold: f32,
    regressions: &mut Vec<RegressionFinding>,
    improvements: &mut usize,
) {
    let (Some(current), Some(baseline)) = (current, baseline) else {
        return;
    };
    let delta = current - baseline;
    if delta > threshold {
        regressions.push(RegressionFinding {
            id: id.to_string(),
            metric: metric.to_string(),
            current,
            baseline,
            delta,
        });
    } else if delta < -threshold {
        *improvements += 1;
    }
}

struct QualitySignals {
    post_worse_ratio: Option<f32>,
    ai_worse_ratio: Option<f32>,
    avg_raw_wer: Option<f32>,
    avg_post_wer: Option<f32>,
    avg_ai_wer: Option<f32>,
}

impl QualitySignals {
    fn from_report(report: &QualityReport, threshold: f32) -> Self {
        let mut post_worse = 0usize;
        let mut post_total = 0usize;
        let mut ai_worse = 0usize;
        let mut ai_total = 0usize;

        for entry in &report.entries {
            if let (Some(raw), Some(post)) = (entry.metrics.raw_wer, entry.metrics.post_wer) {
                post_total += 1;
                if post > raw + threshold {
                    post_worse += 1;
                }
            }
            if let (Some(post), Some(ai)) = (entry.metrics.post_wer, entry.metrics.ai_wer) {
                ai_total += 1;
                if ai > post + threshold {
                    ai_worse += 1;
                }
            }
        }

        Self {
            post_worse_ratio: ratio(post_worse, post_total),
            ai_worse_ratio: ratio(ai_worse, ai_total),
            avg_raw_wer: report.summary.avg_raw_wer,
            avg_post_wer: report.summary.avg_post_wer,
            avg_ai_wer: report.summary.avg_ai_wer,
        }
    }
}

fn ratio(numer: usize, denom: usize) -> Option<f32> {
    if denom == 0 {
        None
    } else {
        Some(numer as f32 / denom as f32)
    }
}

struct PostprocessStats {
    input_chunks: u64,
    gate_drops: u64,
    suspicious: u64,
    embeddings_enabled: Option<bool>,
}

impl PostprocessStats {
    fn from_report(report: &QualityReport) -> Self {
        let mut input = 0u64;
        let mut gate = 0u64;
        let mut suspicious = 0u64;
        let mut embeddings = None;

        for entry in &report.entries {
            let Some(stats) = entry.postprocess_stats.as_ref() else {
                continue;
            };
            input += stats.input_chunks;
            gate += stats.gate_drops;
            suspicious += stats.suspicious_chunks;
            embeddings = match embeddings {
                None => Some(stats.embeddings_enabled),
                Some(value) if value == stats.embeddings_enabled => Some(value),
                Some(_) => None,
            };
        }

        Self {
            input_chunks: input,
            gate_drops: gate,
            suspicious,
            embeddings_enabled: embeddings,
        }
    }

    fn gate_drop_rate(&self) -> Option<f32> {
        if self.input_chunks == 0 {
            None
        } else {
            Some(self.gate_drops as f32 / self.input_chunks as f32)
        }
    }

    fn suspicious_rate(&self) -> Option<f32> {
        if self.input_chunks == 0 {
            None
        } else {
            Some(self.suspicious as f32 / self.input_chunks as f32)
        }
    }
}

fn propose_gate_update(
    signals: &QualitySignals,
    stats: &PostprocessStats,
    apply: bool,
) -> Result<Option<UpdateAction>> {
    let Some(post_worse_ratio) = signals.post_worse_ratio else {
        return Ok(None);
    };
    let env_path = Config::config_dir().join(".env");
    let similarity = read_env_f32(
        &env_path,
        "CODESCRIBE_STREAM_SIMILARITY",
        DEFAULT_SIMILARITY,
    );
    let novelty = read_env_f32(&env_path, "CODESCRIBE_STREAM_NOVELTY", DEFAULT_NOVELTY);

    let mut new_similarity = similarity;
    let mut new_novelty = novelty;
    let mut reason = None;

    let avg_regression = match (signals.avg_post_wer, signals.avg_raw_wer) {
        (Some(post), Some(raw)) => post > raw + DEFAULT_REGRESSION_THRESHOLD,
        _ => false,
    };

    if post_worse_ratio >= 0.30 || avg_regression {
        new_similarity = (similarity + 0.01).min(0.98);
        new_novelty = (novelty - 0.01).max(0.05);
        reason = Some("postprocess regressions detected, relaxing gate".to_string());
    } else if post_worse_ratio < 0.10
        && let Some(suspicious_rate) = stats.suspicious_rate()
        && suspicious_rate > 0.25
    {
        new_similarity = (similarity - 0.01).max(0.85);
        new_novelty = (novelty + 0.01).min(0.30);
        reason = Some("high suspicious rate, tightening gate".to_string());
    }

    if new_similarity == similarity && new_novelty == novelty {
        return Ok(None);
    }

    let mut applied = false;
    if apply {
        applied |= update_env_var(
            &env_path,
            "CODESCRIBE_STREAM_SIMILARITY",
            &format!("{:.3}", new_similarity),
        )?;
        applied |= update_env_var(
            &env_path,
            "CODESCRIBE_STREAM_NOVELTY",
            &format!("{:.3}", new_novelty),
        )?;
    }

    let detail = format!(
        "CODESCRIBE_STREAM_SIMILARITY {:.3} -> {:.3}, CODESCRIBE_STREAM_NOVELTY {:.3} -> {:.3} ({})",
        similarity,
        new_similarity,
        novelty,
        new_novelty,
        reason.unwrap_or_else(|| "tuned".into())
    );

    Ok(Some(UpdateAction {
        kind: "gate_thresholds".into(),
        detail,
        applied,
    }))
}

fn propose_embedding_update(
    signals: &QualitySignals,
    stats: &PostprocessStats,
    apply: bool,
) -> Result<Option<UpdateAction>> {
    let Some(embeddings_enabled) = stats.embeddings_enabled else {
        return Ok(None);
    };
    let env_path = Config::config_dir().join(".env");

    if !embeddings_enabled {
        if let Some(suspicious_rate) = stats.suspicious_rate()
            && suspicious_rate > 0.20
        {
            let applied = if apply {
                update_env_var(&env_path, "CODESCRIBE_STREAM_DISABLE_EMBEDDINGS", "0")?
            } else {
                false
            };
            return Ok(Some(UpdateAction {
                kind: "embeddings".into(),
                detail: "Enable embeddings (suspicious rate high)".into(),
                applied,
            }));
        }
    } else if let Some(post_worse_ratio) = signals.post_worse_ratio
        && post_worse_ratio > 0.40
        && let Some(gate_rate) = stats.gate_drop_rate()
        && gate_rate > 0.40
    {
        let applied = if apply {
            update_env_var(&env_path, "CODESCRIBE_STREAM_DISABLE_EMBEDDINGS", "1")?
        } else {
            false
        };
        return Ok(Some(UpdateAction {
            kind: "embeddings".into(),
            detail: "Disable embeddings (gate too aggressive)".into(),
            applied,
        }));
    }

    Ok(None)
}

fn propose_prompt_tuning(
    signals: &QualitySignals,
    report: &QualityReport,
    apply: bool,
) -> Result<Option<UpdateAction>> {
    let Some(ai_worse_ratio) = signals.ai_worse_ratio else {
        return Ok(None);
    };
    let Some(avg_ai) = signals.avg_ai_wer else {
        return Ok(None);
    };
    let Some(avg_post) = signals.avg_post_wer else {
        return Ok(None);
    };

    if ai_worse_ratio < 0.30 && avg_ai <= avg_post + DEFAULT_REGRESSION_THRESHOLD {
        return Ok(None);
    }

    let now: DateTime<Local> = Local::now();
    let tuning = format!(
        "# AUTO-TUNING {}\n\
- Preserve original wording; do not paraphrase.\n\
- Keep technical terms and identifiers verbatim.\n\
- If unsure, keep the word as-is.\n\
- Keep bracketed tags like [NIEWYRAZNE] unchanged.\n",
        now.format("%Y-%m-%d %H:%M:%S")
    );

    let prompts_dir = Config::config_dir().join("prompts");
    fs::create_dir_all(&prompts_dir)?;
    let path = prompts_dir.join("formatting_tuning.txt");

    let applied = if apply {
        let existing = fs::read_to_string(&path).unwrap_or_default();
        if existing.trim() != tuning.trim() {
            fs::write(&path, &tuning)?;
            true
        } else {
            false
        }
    } else {
        false
    };

    Ok(Some(UpdateAction {
        kind: "prompt_tuning".into(),
        detail: format!(
            "formatting_tuning.txt updated (ai_worse_ratio={:.2}, avg_ai_wer={:.3}, avg_post_wer={:.3}, entries={})",
            ai_worse_ratio,
            avg_ai,
            avg_post,
            report.entries.len()
        ),
        applied,
    }))
}

fn propose_lexicon_updates(
    report: &QualityReport,
    max_updates: usize,
    apply: bool,
) -> Result<Option<UpdateAction>> {
    let suggestions = extract_lexicon_suggestions(report, max_updates);
    if suggestions.is_empty() {
        return Ok(None);
    }

    let path = Config::config_dir().join("lexicon.custom.jsonl");
    let applied = if apply {
        apply_lexicon_suggestions(&path, &suggestions)?
    } else {
        false
    };

    let detail = format!(
        "lexicon.custom.jsonl suggestions={} (top: {})",
        suggestions.len(),
        suggestions
            .iter()
            .take(3)
            .map(|s| format!("{}<-{}", s.term, s.mis))
            .collect::<Vec<_>>()
            .join(", ")
    );

    Ok(Some(UpdateAction {
        kind: "lexicon".into(),
        detail,
        applied,
    }))
}

#[derive(Debug)]
struct LexiconSuggestion {
    term: String,
    mis: String,
    count: usize,
}

fn extract_lexicon_suggestions(
    report: &QualityReport,
    max_updates: usize,
) -> Vec<LexiconSuggestion> {
    let mut counts: HashMap<(String, String), usize> = HashMap::new();

    for entry in &report.entries {
        let Some(reference) = entry.transcripts.reference.as_deref() else {
            continue;
        };
        let Some(raw) = entry.transcripts.raw.as_deref() else {
            continue;
        };

        let ref_tokens = normalize_tokens(reference);
        let raw_tokens = normalize_tokens(raw);
        let subs = align_tokens(&ref_tokens, &raw_tokens);

        for (term, mis) in subs {
            if !token_eligible(&term) || !token_eligible(&mis) {
                continue;
            }
            if term.eq_ignore_ascii_case(&mis) {
                continue;
            }
            if word_distance(&term, &mis) > 2 {
                continue;
            }

            let key = (term.clone(), mis.clone());
            *counts.entry(key).or_insert(0) += 1;
        }
    }

    let mut suggestions: Vec<LexiconSuggestion> = counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .map(|((term, mis), count)| LexiconSuggestion { term, mis, count })
        .collect();

    suggestions.sort_by(|a, b| b.count.cmp(&a.count));
    if max_updates > 0 && suggestions.len() > max_updates {
        suggestions.truncate(max_updates);
    }
    suggestions
}

#[derive(Debug, Serialize, Deserialize)]
struct LexiconEntry {
    term: String,
    mispronunciations: Vec<String>,
}

fn apply_lexicon_suggestions(path: &Path, suggestions: &[LexiconSuggestion]) -> Result<bool> {
    let mut entries = read_custom_lexicon(path);
    let mut changed = false;

    for suggestion in suggestions {
        let bucket = entries.entry(suggestion.term.clone()).or_default();
        if bucket.insert(suggestion.mis.clone()) {
            changed = true;
        }
    }

    if changed {
        let mut out = String::new();
        let mut keys: Vec<_> = entries.keys().cloned().collect();
        keys.sort();
        for key in keys {
            let mut mis: Vec<_> = entries[&key].iter().cloned().collect();
            mis.sort();
            let entry = LexiconEntry {
                term: key,
                mispronunciations: mis,
            };
            out.push_str(&serde_json::to_string(&entry)?);
            out.push('\n');
        }
        fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))?;
        fs::write(path, out)?;
    }

    Ok(changed)
}

fn read_custom_lexicon(path: &Path) -> HashMap<String, HashSet<String>> {
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();
    let content = fs::read_to_string(path).unwrap_or_default();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<LexiconEntry>(trimmed) {
            let bucket = map.entry(entry.term).or_default();
            for mis in entry.mispronunciations {
                bucket.insert(mis);
            }
        }
    }
    map
}

fn normalize_tokens(text: &str) -> Vec<String> {
    let mut normalized = String::with_capacity(text.len());
    for ch in text.to_lowercase().chars() {
        if ch.is_alphanumeric() || ch.is_whitespace() {
            normalized.push(ch);
        } else {
            normalized.push(' ');
        }
    }
    normalized
        .split_whitespace()
        .map(|t| t.to_string())
        .collect()
}

fn token_eligible(token: &str) -> bool {
    if token.len() < 3 {
        return false;
    }
    if token.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    true
}

fn align_tokens(reference: &[String], hypothesis: &[String]) -> Vec<(String, String)> {
    let n = reference.len();
    let m = hypothesis.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];

    for (i, row) in dp.iter_mut().enumerate().take(n + 1) {
        row[0] = i;
    }
    for (j, value) in dp[0].iter_mut().enumerate().take(m + 1) {
        *value = j;
    }

    for i in 1..=n {
        for j in 1..=m {
            let cost = if reference[i - 1] == hypothesis[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    let mut subs = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 {
            let cost = if reference[i - 1] == hypothesis[j - 1] {
                0
            } else {
                1
            };
            if dp[i][j] == dp[i - 1][j - 1] + cost {
                if cost == 1 {
                    subs.push((reference[i - 1].clone(), hypothesis[j - 1].clone()));
                }
                i -= 1;
                j -= 1;
                continue;
            }
        }
        if i > 0 && dp[i][j] == dp[i - 1][j] + 1 {
            i -= 1;
        } else if j > 0 {
            j -= 1;
        } else {
            break;
        }
    }

    subs
}

fn word_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    levenshtein(&a_chars, &b_chars)
}

fn levenshtein<T: Eq>(a: &[T], b: &[T]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];

    for (i, item_a) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, item_b) in b.iter().enumerate() {
            let cost = if item_a == item_b { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        prev.clone_from(&cur);
    }

    prev[b.len()]
}

fn read_env_f32(path: &Path, key: &str, default: f32) -> f32 {
    if let Ok(value) = std::env::var(key)
        && let Ok(parsed) = value.parse::<f32>()
    {
        return parsed;
    }

    if let Some(value) = read_env_value(path, key)
        && let Ok(parsed) = value.parse::<f32>()
    {
        return parsed;
    }

    default
}

fn read_env_value(path: &Path, key: &str) -> Option<String> {
    let content = safe_read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        let Some((k, v)) = trimmed.split_once('=') else {
            continue;
        };
        if k.trim() == key {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn update_env_var(path: &Path, key: &str, value: &str) -> Result<bool> {
    let mut lines = Vec::new();
    let mut found = false;
    let mut changed = false;
    let target = format!("{}={}", key, value);

    if path.exists() {
        let content = safe_read_to_string(path)?;
        for line in content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with(&format!("{}=", key)) {
                found = true;
                if line != target {
                    changed = true;
                }
                lines.push(target.clone());
            } else {
                lines.push(line.to_string());
            }
        }
    }

    if !found {
        lines.push(target.clone());
        changed = true;
    }

    if changed {
        let mut output = lines.join("\n");
        output.push('\n');
        let canonical = match safe_canonicalize(path) {
            Ok(path) => path,
            Err(_) => path.to_path_buf(),
        };
        // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path (config path under ~/.codescribe)
        fs::write(canonical, output)?;
    }
    Ok(changed)
}
