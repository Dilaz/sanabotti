use dotenvy::dotenv;
use miette::IntoDiagnostic;
use std::env;
use tracing::info;

use crate::Error;

pub struct Config {
    pub discord_token: String,
    pub channel_id: u64,
    pub dictionary_path: String,
    pub bot_activity: String,
    pub llm_batch_size: usize,
    pub batch_timeout_secs: u64,
}

pub fn load_config() -> miette::Result<Config> {
    info!("Loading configuration");

    // Load environment variables
    dotenv().ok();

    // Get required environment variables
    let discord_token = env::var("DISCORD_TOKEN")
        .into_diagnostic()
        .map_err(|_| Error::Config("Missing DISCORD_TOKEN".to_string()))?;

    let channel_id = env::var("TARGET_CHANNEL_ID")
        .into_diagnostic()
        .map_err(|_| Error::Config("Missing TARGET_CHANNEL_ID".to_string()))?
        .parse::<u64>()
        .into_diagnostic()
        .map_err(|_| Error::Config("Invalid TARGET_CHANNEL_ID".to_string()))?;

    let dictionary_path =
        env::var("DICTIONARY_FILE_PATH").unwrap_or_else(|_| "./data/finnish_words.txt".to_string());

    let bot_activity = env::var("BOT_ACTIVITY").unwrap_or_else(|_| "Finnish Word Game".to_string());

    let llm_batch_size = env::var("LLM_BATCH_SIZE")
        .unwrap_or_else(|_| "2".to_string())
        .parse::<usize>()
        .into_diagnostic()
        .map_err(|_| Error::Config("Invalid LLM_BATCH_SIZE".to_string()))?;

    let batch_timeout_secs = env::var("LLM_BATCH_TIMEOUT_SECS")
        .unwrap_or_else(|_| "86400".to_string()) // 24 hours default
        .parse::<u64>()
        .into_diagnostic()
        .map_err(|_| Error::Config("Invalid LLM_BATCH_TIMEOUT_SECS".to_string()))?;

    Ok(Config {
        discord_token,
        channel_id,
        dictionary_path,
        bot_activity,
        llm_batch_size,
        batch_timeout_secs,
    })
}
