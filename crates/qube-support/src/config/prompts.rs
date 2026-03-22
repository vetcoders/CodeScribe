use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use super::Config;

const FORMATTING_PROMPT: &str = r#"TRANSCRIPTION FORMATTER

You clean up ASR transcripts, fix obvious grammar issues, and keep technical words intact.
"#;

const ASSISTIVE_PROMPT: &str = r#"Jesteś asystentem tekstowym.

Pomagaj zwięźle, zachowuj kontekst rozmowy i odpowiadaj plain textem.
"#;

pub fn get_formatting_prompt_path() -> PathBuf {
    prompts_dir().join("formatting_prompt.txt")
}

pub fn get_assistive_prompt_path() -> PathBuf {
    prompts_dir().join("assistive_prompt.txt")
}

pub fn get_formatting_prompt() -> String {
    read_or_bootstrap(get_formatting_prompt_path(), FORMATTING_PROMPT)
}

pub fn get_assistive_prompt() -> String {
    read_or_bootstrap(get_assistive_prompt_path(), ASSISTIVE_PROMPT)
}

pub fn reset_to_defaults() -> Result<()> {
    write_prompt(get_formatting_prompt_path(), FORMATTING_PROMPT)?;
    write_prompt(get_assistive_prompt_path(), ASSISTIVE_PROMPT)?;
    Ok(())
}

fn prompts_dir() -> PathBuf {
    let dir = Config::config_dir().join("prompts");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn read_or_bootstrap(path: PathBuf, default_contents: &str) -> String {
    if let Ok(contents) = fs::read_to_string(&path) {
        return contents;
    }

    let _ = write_prompt(path.clone(), default_contents);
    default_contents.to_string()
}

fn write_prompt(path: PathBuf, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create prompt dir: {}", parent.display()))?;
    }
    fs::write(&path, contents).with_context(|| format!("write prompt: {}", path.display()))?;
    Ok(())
}
