use actix::{Actor, Context, Handler, Message};
use poise::serenity_prelude as serenity;
use std::sync::Arc;
use std::thread;
use tracing::{error, info, warn};

/// Emoji constants for reactions
pub const EMOJI_CHECK: char = '✅';
pub const EMOJI_CROSS: char = '❌';
pub const EMOJI_QUESTION: char = '❓';

/// Message to add a reaction to a Discord message
#[derive(Message)]
#[rtype(result = "()")]
pub struct AddReaction {
    pub message_id: u64,
    pub reaction: char,
}

/// Message to clear reactions from a Discord message
#[derive(Message)]
#[rtype(result = "()")]
pub struct ClearReactions {
    pub message_id: u64,
}

/// Message to delete a specific reaction from a Discord message
#[derive(Message)]
#[rtype(result = "()")]
pub struct DeleteReaction {
    pub message_id: u64,
    pub reaction: char,
}

/// Actor that manages Discord message reactions
pub struct MessageReactionActor {
    discord_ctx: Arc<serenity::Context>,
    channel_id: serenity::ChannelId,
}

impl MessageReactionActor {
    pub fn new(discord_ctx: Arc<serenity::Context>, channel_id: serenity::ChannelId) -> Self {
        Self {
            discord_ctx,
            channel_id,
        }
    }
}

impl Actor for MessageReactionActor {
    type Context = Context<Self>;
}

impl Handler<AddReaction> for MessageReactionActor {
    type Result = ();

    fn handle(&mut self, msg: AddReaction, _ctx: &mut Context<Self>) -> Self::Result {
        let discord_ctx = self.discord_ctx.clone();
        let channel_id = self.channel_id;
        let message_id = serenity::MessageId::new(msg.message_id);
        let reaction = msg.reaction; // Using char directly

        info!(
            "Attempting to add reaction '{}' to message {}",
            reaction, message_id
        );

        // Use std::thread to handle Discord API calls without requiring LocalSet
        let handle = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                info!("Starting to process reaction '{}' for message {}", reaction, message_id);

                // Simplified permissions check
                let bot_id = discord_ctx.cache.current_user().id;
                info!("Bot ID for permissions check: {:?}", bot_id);

                match channel_id.message(&discord_ctx, message_id).await {
                    Ok(message) => {
                        info!("Successfully fetched message {}, adding reaction '{}'", message_id, reaction);

                        // Try to add the reaction
                        match message.react(&discord_ctx, reaction).await {
                            Ok(_) => {
                                info!("Successfully added reaction '{}' to message {}", reaction, message_id);
                            },
                            Err(e) => {
                                error!("Failed to add reaction '{}' to message {}: {}", reaction, message_id, e);
                                // Try to diagnose the issue
                                if e.to_string().contains("Missing Access") || e.to_string().contains("Missing Permissions") {
                                    warn!("Bot lacks permission to add reactions. Please ensure it has the ADD_REACTIONS permission.");
                                } else if e.to_string().contains("Unknown Message") {
                                    warn!("Message {} not found. It may have been deleted.", message_id);
                                }
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to fetch message {}: {}", message_id, e);
                        if e.to_string().contains("Unknown Message") {
                            warn!("Message {} not found. It may have been deleted or the bot cannot access it.", message_id);
                        }
                    }
                }
            });
        });

        // Don't wait for the thread to complete
        std::mem::drop(handle);
    }
}

impl Handler<DeleteReaction> for MessageReactionActor {
    type Result = ();

    fn handle(&mut self, msg: DeleteReaction, _ctx: &mut Context<Self>) -> Self::Result {
        let discord_ctx = self.discord_ctx.clone();
        let channel_id = self.channel_id;
        let message_id = serenity::MessageId::new(msg.message_id);
        let reaction = msg.reaction;

        // Use std::thread to handle Discord API calls without requiring LocalSet
        let handle = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                match channel_id.message(&discord_ctx, message_id).await {
                    Ok(message) => {
                        if let Err(e) = message.delete_reaction_emoji(&discord_ctx, reaction).await
                        {
                            error!(
                                "Failed to delete reaction '{}' from message {}: {}",
                                reaction, message_id, e
                            );
                        } else {
                            info!(
                                "Deleted reaction '{}' from message {}",
                                reaction, message_id
                            );
                        }
                    }
                    Err(e) => {
                        error!("Failed to fetch message {}: {}", message_id, e);
                    }
                }
            });
        });

        // Don't wait for the thread to complete
        std::mem::drop(handle);
    }
}

impl Handler<ClearReactions> for MessageReactionActor {
    type Result = ();

    fn handle(&mut self, msg: ClearReactions, _ctx: &mut Context<Self>) -> Self::Result {
        let discord_ctx = self.discord_ctx.clone();
        let channel_id = self.channel_id;
        let message_id = serenity::MessageId::new(msg.message_id);

        // Use std::thread to handle Discord API calls without requiring LocalSet
        let handle = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                match channel_id.message(&discord_ctx, message_id).await {
                    Ok(message) => {
                        if let Err(e) = message.delete_reactions(&discord_ctx).await {
                            error!("Failed to clear reactions from message: {}", e);
                        } else {
                            info!("Cleared all reactions from message {}", message_id);
                        }
                    }
                    Err(e) => {
                        error!("Failed to fetch message {}: {}", message_id, e);
                    }
                }
            });
        });

        // Don't wait for the thread to complete
        std::mem::drop(handle);
    }
}
