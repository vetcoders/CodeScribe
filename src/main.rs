//! CodeScribe CLI - Local speech-to-text transcription
//!
//! Lightweight CLI for direct audio file transcription.
//! For tray app + GUI, use CodeScribe.app (Tauri bundle).
//!
//! Created by M&K (c)2026 VetCoders

use anyhow::Result;
use clap::Parser;
use codescribe::{audio, whisper};
use std::path::PathBuf;

/// CodeScribe CLI - Local speech-to-text transcription
///
/// For the full app with tray icon and hotkeys, run CodeScribe.app
#[derive(Parser)]
#[command(name = "codescribe")]
#[command(version)]
#[command(author = "VetCoders <hello@vetcoders.io>")]
#[command(about = "Local speech-to-text transcription", long_about = None)]
struct Cli {
    /// Open config file in editor (creates default if missing)
    #[arg(long)]
    config: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Transcribe an audio file using local Whisper
    Transcribe {
        /// Path to audio file (wav, mp3, m4a)
        file: PathBuf,

        /// Language code (e.g., pl, en). Default: auto-detect
        #[arg(short, long)]
        language: Option<String>,

        /// Format output using AI (Ollama)
        #[arg(short, long)]
        format: bool,

        /// LLM model for formatting
        #[arg(long, default_value = "qwen3-coder:480b-cloud")]
        llm: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle --config flag
    if cli.config {
        return handle_config_command();
    }

    // Handle subcommands
    match cli.command {
        Some(Commands::Transcribe {
            file,
            language,
            format,
            llm,
        }) => handle_transcribe_command(file, language, format, llm).await,
        None => {
            eprintln!("CodeScribe CLI - Local speech-to-text transcription");
            eprintln!();
            eprintln!("Usage:");
            eprintln!("  codescribe transcribe <file>     Transcribe audio file");
            eprintln!("  codescribe --config              Open config file");
            eprintln!();
            eprintln!("For the full app with tray icon and hotkeys, run CodeScribe.app");
            Ok(())
        }
    }
}

/// Handle --config flag: create default config and open in editor
fn handle_config_command() -> Result<()> {
    use std::fs;
    use std::process::Command;

    let config_dir = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
        .join(".codescribe");
    let config_path = config_dir.join(".env");

    // Create directory if needed
    fs::create_dir_all(&config_dir)?;

    // Create default config if missing
    if !config_path.exists() {
        let default_config = include_str!("config/default_env.txt");
        fs::write(&config_path, default_config)?;
        println!("Created default config: {}", config_path.display());
    } else {
        println!("Config exists: {}", config_path.display());
    }

    // Open in editor
    #[cfg(target_os = "macos")]
    {
        use std::io::IsTerminal;
        if !std::io::stdin().is_terminal() {
            println!("Opening in default text editor");
            Command::new("open").arg("-t").arg(&config_path).status()?;
            return Ok(());
        }
    }

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            for editor in &["code", "nvim", "vim", "nano"] {
                if Command::new("which")
                    .arg(editor)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
                {
                    return editor.to_string();
                }
            }
            "nano".to_string()
        });

    println!("Opening in: {}", editor);
    Command::new(&editor).arg(&config_path).status()?;

    Ok(())
}

/// Handle `codescribe transcribe <file>` command
async fn handle_transcribe_command(
    file: PathBuf,
    language: Option<String>,
    format: bool,
    llm_model: String,
) -> Result<()> {
    use std::time::Instant;

    // Check file exists
    if !file.exists() {
        anyhow::bail!("File not found: {}", file.display());
    }

    eprintln!("CodeScribe Local Transcription");
    eprintln!("Audio: {}", file.display());

    // Initialize Whisper
    eprintln!("Loading Whisper model...");
    let start = Instant::now();
    whisper::init()?;

    if whisper::embedded::is_embedded_available() {
        eprintln!("Model: embedded (zero I/O)");
    } else if let Ok(path) = whisper::get_model_path() {
        eprintln!("Model: {}", path.display());
    }
    eprintln!("Language: {}", language.as_deref().unwrap_or("auto-detect"));
    eprintln!("Model loaded in {:?}", start.elapsed());

    // Detect language if not specified
    let lang = if let Some(l) = language {
        l
    } else {
        eprintln!("Detecting language...");
        let start = Instant::now();
        let (samples, sample_rate) = audio::load_audio_file(&file)?;
        let detected = whisper::detect_language(&samples, sample_rate)?;
        eprintln!("Detected: {} ({:?})", detected, start.elapsed());
        detected
    };

    // Transcribe
    eprintln!("Transcribing...");
    let start = Instant::now();
    let raw_text = whisper::transcribe_file(&file, Some(&lang))?;
    eprintln!("Transcription time: {:?}", start.elapsed());

    // Format with AI if requested
    let final_text = if format {
        eprintln!("Formatting with AI ({})...", llm_model);
        let start = Instant::now();
        match format_with_ollama(&raw_text, &llm_model, &lang).await {
            Ok(formatted) => {
                eprintln!("Formatted in {:?}", start.elapsed());
                formatted
            }
            Err(e) => {
                eprintln!("Formatting failed: {} - using raw text", e);
                raw_text
            }
        }
    } else {
        raw_text
    };

    eprintln!();

    // Output transcription to stdout (pipeable)
    println!("{}", final_text);

    Ok(())
}

/// Format transcription using Ollama LLM
async fn format_with_ollama(text: &str, model: &str, lang: &str) -> Result<String> {
    let host = std::env::var("LLM_HOST")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());

    let endpoint = format!("{}/api/chat", host.trim_end_matches('/'));

    let system_prompt = format!(
        r#"You are a transcription formatter. Clean up and format the following speech-to-text transcription.

Rules:
- Fix punctuation, capitalization, and obvious speech recognition errors
- Remove filler words (um, uh, like) and repetitions
- Structure into clear paragraphs where appropriate
- Keep the original meaning and language ({})
- Use bullet points or numbered lists if the content is enumerating items
- Do NOT add any commentary, just output the formatted text
- Do NOT translate - keep the original language"#,
        lang
    );

    #[derive(serde::Serialize)]
    struct OllamaRequest {
        model: String,
        messages: Vec<OllamaMessage>,
        stream: bool,
        options: OllamaOptions,
    }

    #[derive(serde::Serialize)]
    struct OllamaMessage {
        role: &'static str,
        content: String,
    }

    #[derive(serde::Serialize)]
    struct OllamaOptions {
        temperature: f32,
        num_predict: u32,
    }

    #[derive(serde::Deserialize)]
    struct OllamaResponse {
        message: Option<OllamaMessageResponse>,
    }

    #[derive(serde::Deserialize)]
    struct OllamaMessageResponse {
        content: String,
    }

    let request = OllamaRequest {
        model: model.to_string(),
        messages: vec![
            OllamaMessage {
                role: "system",
                content: system_prompt,
            },
            OllamaMessage {
                role: "user",
                content: text.to_string(),
            },
        ],
        stream: false,
        options: OllamaOptions {
            temperature: 0.1,
            num_predict: 0,
        },
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Ollama HTTP {} - {}", status, body);
    }

    let ollama_response: OllamaResponse = response.json().await?;

    ollama_response
        .message
        .map(|m| m.content.trim().to_string())
        .ok_or_else(|| anyhow::anyhow!("Empty Ollama response"))
}
