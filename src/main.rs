use tokio::signal;
use tokio::task::LocalSet;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use sanabotti::{config, discord};

#[actix_rt::main]
async fn main() -> miette::Result<()> {
    // Set up logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sanabotti=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Finnish Word Game Discord Bot");

    // Load configuration
    let config = config::load_config()?;

    // Create a local task set to ensure local tasks work properly
    let local = LocalSet::new();

    // Run the Discord bot within the local task set
    // Also handle application shutdown gracefully
    local
        .run_until(async {
            tokio::select! {
                result = discord::setup_bot(
                    config.discord_token.clone(),
                    config.channel_id,
                    config.dictionary_path.clone(),
                    config.bot_activity.clone(),
                    config
                ) => result,
                _ = signal::ctrl_c() => {
                    info!("Received shutdown signal, stopping bot");
                    Ok(())
                }
            }
        })
        .await
}
