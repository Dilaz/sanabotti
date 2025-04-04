use miette::{Diagnostic, SourceSpan};
use std::io;
use thiserror::Error;

/// Primary error type for the Sanabotti application
#[derive(Error, Debug, Diagnostic)]
pub enum BotError {
    #[error("Discord API error: {0}")]
    #[diagnostic(code(sanabotti::discord_error))]
    Discord(#[from] serenity::Error),

    #[error("Environment configuration error: {0}")]
    #[diagnostic(code(sanabotti::config_error))]
    Config(String),

    #[error("Dictionary error: {0}")]
    #[diagnostic(code(sanabotti::dictionary_error))]
    Dictionary(#[from] DictionaryError),

    #[error("Validation error: {0}")]
    #[diagnostic(code(sanabotti::validation_error))]
    Validation(#[from] ValidationError),

    #[error("Actor system error: {0}")]
    #[diagnostic(code(sanabotti::actor_error))]
    Actor(String),

    #[error("I/O error: {0}")]
    #[diagnostic(code(sanabotti::io_error))]
    Io(#[from] io::Error),

    #[error("LLM error: {0}")]
    #[diagnostic(code(sanabotti::llm_error))]
    LLM(#[from] LLMError),

    #[error("Message reaction error: {0}")]
    #[diagnostic(code(sanabotti::reaction_error))]
    Reaction(String),
}

/// Dictionary-specific errors
#[derive(Error, Debug, Diagnostic)]
pub enum DictionaryError {
    #[error("Failed to load dictionary file: {0}")]
    #[diagnostic(code(sanabotti::dictionary::load_error))]
    LoadError(#[from] io::Error),

    #[error("Dictionary file format error: {0}")]
    #[diagnostic(code(sanabotti::dictionary::format_error))]
    FormatError(String),

    #[error("Dictionary is empty")]
    #[diagnostic(code(sanabotti::dictionary::empty))]
    EmptyDictionary,
}

/// Validation-specific errors
#[derive(Error, Debug, Diagnostic)]
pub enum ValidationError {
    #[error("Word not found in dictionary: {0}")]
    #[diagnostic(code(sanabotti::validation::not_found))]
    NotInDictionary(String),

    #[error("Word does not follow game rules: {reason}")]
    #[diagnostic(code(sanabotti::validation::rule_violation))]
    RuleViolation {
        #[source_code]
        word: String,

        #[label("This part violates the rules")]
        span: Option<SourceSpan>,

        reason: String,
    },

    #[error("Word has been used before")]
    #[diagnostic(code(sanabotti::validation::already_used))]
    AlreadyUsed(String),
}

/// LLM-specific errors
#[derive(Error, Debug, Diagnostic)]
pub enum LLMError {
    #[error("API error: {0}")]
    #[diagnostic(code(sanabotti::llm::api_error))]
    ApiError(String),

    #[error("Response parsing error: {0}")]
    #[diagnostic(code(sanabotti::llm::parse_error))]
    ParseError(String),

    #[error("Rate limit exceeded")]
    #[diagnostic(code(sanabotti::llm::rate_limit))]
    RateLimit,

    #[error("Timeout waiting for LLM response")]
    #[diagnostic(code(sanabotti::llm::timeout))]
    Timeout,
}

// Re-export error types for convenience
pub use BotError as Error;

/// Create a result type that uses our error type
pub type Result<T> = std::result::Result<T, Error>;
