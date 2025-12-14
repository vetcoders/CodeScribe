//! Demo of the config module capabilities
//!
//! Run with: cargo run --example config_demo

use codescribe::config::{self, AiProvider, Config, HoldMods, Language};

fn main() -> anyhow::Result<()> {
    println!("CodeScribe Config Demo\n");

    // Load config (from .env or defaults)
    let config = Config::load();
    println!("Loaded config:");
    println!("  Hold mods: {:?}", config.hold_mods);
    println!("  Language: {:?}", config.whisper_language);
    println!("  AI provider: {:?}", config.ai_provider);
    println!("  Beep on start: {}", config.beep_on_start);
    println!("  Sound name: {}", config.sound_name);
    println!("  Sound volume: {}", config.sound_volume);
    println!();

    // Demonstrate enum parsing
    println!("Enum parsing examples:");
    println!(
        "  \"ctrl_alt\".parse::<HoldMods>() = {:?}",
        "ctrl_alt".parse::<HoldMods>()
    );
    println!(
        "  \"pl\".parse::<Language>() = {:?}",
        "pl".parse::<Language>()
    );
    println!(
        "  \"ollama\".parse::<AiProvider>() = {:?}",
        "ollama".parse::<AiProvider>()
    );
    println!();

    // Save config to .env
    println!("Saving config to .env...");
    config.save_all_to_env()?;
    println!("Config saved to: {:?}", Config::env_path());
    println!();

    // Demonstrate single-value save
    println!("Updating single value (BEEP_ON_START=false)...");
    config.save_to_env("BEEP_ON_START", "false")?;
    println!();

    // Load again to verify
    let reloaded = Config::load();
    println!("Reloaded config:");
    println!(
        "  Beep on start: {} (should be false)",
        reloaded.beep_on_start
    );
    println!();

    // Demonstrate global config
    println!("Testing global config API:");
    config::init();

    let val = config::get().beep_on_start;
    println!("  Global config beep_on_start: {}", val);

    config::update(|c| {
        c.beep_on_start = true;
        c.sound_volume = 0.8;
    });

    let updated = config::get();
    println!(
        "  After update: beep={}, volume={}",
        updated.beep_on_start, updated.sound_volume
    );
    println!();

    println!("Demo complete!");
    Ok(())
}
