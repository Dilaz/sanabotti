pub mod actors;
pub mod config;
pub mod discord;
pub mod error;
pub mod validation;

// Re-export error types for convenience
pub use error::{DictionaryError, Error, LLMError, Result, ValidationError};

// Common types used across the application
pub struct Data {
    pub channel_id: poise::serenity_prelude::ChannelId,
    pub word_validator: actix::Addr<actors::WordValidatorActor>,
}
