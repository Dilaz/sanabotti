use actix::Actor;
use miette::IntoDiagnostic;
use poise::serenity_prelude as serenity;
use std::sync::Arc;
use std::thread;
use tokio::sync::oneshot;
use tracing::{error, info};

use crate::{
    actors::{
        word_validator::ValidateWord, GameStateActor, LLMValidatorActor, MessageReactionActor,
        WordValidatorActor,
    },
    config::Config,
    Data, Error,
};

pub async fn setup_bot(
    token: String,
    channel_id: u64,
    dictionary_path: String,
    activity: String,
    config: Config,
) -> miette::Result<()> {
    info!("Setting up Discord bot");

    // Create a channel to receive actor addresses from the actor system thread
    let (tx, rx) = oneshot::channel();

    // Create an exit signal channel
    let (exit_tx, exit_rx) = tokio::sync::oneshot::channel();

    // Start the actor system in a separate thread
    let _actor_thread = thread::spawn(move || {
        // Create a new actix system
        let system = actix_rt::System::new();

        system.block_on(async {
            // Use a LocalSet to allow spawn_local operations
            let local = tokio::task::LocalSet::new();

            local
                .run_until(async {
                    // Initialize actors
                    let game_state = GameStateActor::new().start();
                    let llm_validator = LLMValidatorActor::new(&config).start();

                    // Log actor addresses
                    info!("Game state actor address: {:?}", game_state);
                    info!("LLM validator actor address: {:?}", llm_validator);

                    // Send the addresses to the main thread
                    if let Err(e) = tx.send((game_state, llm_validator)) {
                        error!("Failed to send actor addresses: {:?}", e);
                    }

                    // IMPORTANT: Keep this thread running until the application exits
                    // This ensures the actors continue to process messages
                    // If this is removed, the thread will exit and the actors will stop working
                    match exit_rx.await {
                        Ok(_) => info!("Actor system shutting down gracefully"),
                        Err(_) => info!("Actor system shutdown channel closed"),
                    }
                })
                .await;
        });

        info!("Actor system thread exiting");
    });

    // Receive actor addresses from the actor system thread
    let (game_state, llm_validator) = rx.await.map_err(|e| {
        error!("Failed to receive actor addresses: {}", e);
        miette::miette!("Failed to initialize actor system")
    })?;

    let options = poise::FrameworkOptions {
        event_handler: move |_ctx,
                             event,
                             _framework: poise::FrameworkContext<'_, Data, Error>,
                             data: &Data| {
            Box::pin(async move {
                if let serenity::FullEvent::Message { new_message } = event {
                    // Process only messages from the target channel
                    if new_message.channel_id == data.channel_id {
                        info!(
                            "Received message in target channel: {}",
                            new_message.content
                        );

                        // Skip messages from the bot itself
                        if new_message.author.bot {
                            return Ok(());
                        }

                        // Extract the word from the message
                        let content = new_message.content.trim();

                        // Skip empty messages or commands
                        if content.is_empty() || content.starts_with('!') {
                            return Ok(());
                        }

                        // Send the word for validation
                        info!(
                            "Attempting to send word '{}' to word validator actor",
                            content
                        );
                        data.word_validator.do_send(ValidateWord {
                            word: content.to_string(),
                            message_id: new_message.id.get(),
                            user_id: new_message.author.id.get(),
                        });

                        info!("Sent word '{}' for validation", content);
                    }
                }
                Ok(())
            })
        },
        ..Default::default()
    };

    // Save these values for later use
    let dictionary_path_clone = dictionary_path.clone();
    let channel_id_clone = channel_id;

    // Create framework
    let framework = poise::Framework::builder()
        .options(options)
        .setup(move |ctx, ready, framework| {
            // Capture moved values
            let dictionary_path = dictionary_path_clone.clone();
            let channel_id = channel_id_clone;
            let activity = activity.clone();
            let game_state = game_state.clone();
            let llm_validator = llm_validator.clone();

            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands)
                    .await
                    .map_err(Error::Discord)?;

                // Set the bot's status with the configured activity
                info!("{} is connected!", ready.user.name);
                ctx.set_presence(
                    Some(serenity::ActivityData::playing(&activity)),
                    serenity::OnlineStatus::Online,
                );
                info!("Setting activity to {}", activity);

                // Create a properly type-erased, 'static Context
                let ctx = Arc::new(ctx.clone());
                let channel_id = serenity::ChannelId::new(channel_id);

                // Start the message_reaction actor in a new thread to avoid LocalSet issues
                let (msg_tx, msg_rx) = tokio::sync::oneshot::channel();
                let _message_thread = thread::spawn(move || {
                    let system = actix_rt::System::new();
                    system.block_on(async {
                        let local = tokio::task::LocalSet::new();
                        local
                            .run_until(async {
                                let actor =
                                    MessageReactionActor::new(ctx.clone(), channel_id).start();

                                // Send actor address back
                                if let Err(e) = msg_tx.send(actor) {
                                    error!(
                                        "Failed to send message reaction actor address: {:?}",
                                        e
                                    );
                                }

                                // IMPORTANT: Keep this thread running until the application exits
                                // This ensures the actor continues to process messages
                                // If this is removed, the thread will exit and the actor will stop working
                                tokio::signal::ctrl_c().await.ok();
                            })
                            .await
                    })
                });

                // Get the actor address without joining the thread
                let message_reaction = msg_rx.await.map_err(|_| {
                    Error::Actor("Failed to get message reaction actor address".into())
                })?;

                // Create the word validator actor
                let validator = match WordValidatorActor::new(
                    &dictionary_path,
                    game_state,
                    llm_validator,
                    message_reaction,
                ) {
                    Ok(validator) => validator,
                    Err(e) => {
                        error!("Failed to initialize word validator: {}", e);
                        return Err(e);
                    }
                };

                // Start the word validator in a new thread
                let (word_tx, word_rx) = tokio::sync::oneshot::channel();
                let _validator_thread = thread::spawn(move || {
                    let system = actix_rt::System::new();
                    system.block_on(async {
                        let local = tokio::task::LocalSet::new();
                        local
                            .run_until(async {
                                let actor = validator.start();

                                // Send actor address back
                                if let Err(e) = word_tx.send(actor) {
                                    error!("Failed to send word validator actor address: {:?}", e);
                                }

                                // IMPORTANT: Keep this thread running until the application exits
                                // This ensures the actor continues to process messages
                                // If this is removed, the thread will exit and the actor will stop working
                                tokio::signal::ctrl_c().await.ok();
                            })
                            .await
                    })
                });

                // Get the actor address without joining the thread
                let word_validator = word_rx.await.map_err(|_| {
                    Error::Actor("Failed to get word validator actor address".into())
                })?;

                info!("Word validation system initialized successfully");

                // Return the data with initialized actors
                Ok(Data {
                    channel_id,
                    word_validator,
                })
            })
        })
        .build();

    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await
        .into_diagnostic()?;

    info!("Starting bot...");
    client
        .start()
        .await
        .map_err(Error::Discord)
        .into_diagnostic()?;

    // Make sure to drop exit_tx when function exits to signal cleanup
    let _exit_signal: tokio::sync::oneshot::Sender<()> = exit_tx;

    Ok(())
}
