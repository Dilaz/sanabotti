use actix::{Actor, Addr, AsyncContext, Context, Handler, Message};
use serde_json;
use std::collections::VecDeque;
use std::env;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, error, info};

use crate::actors::game_state::{GameStateActor, MarkWordValidity};
use crate::actors::message_reaction::{
    DeleteReaction, MessageReactionActor, EMOJI_CHECK, EMOJI_CROSS, EMOJI_QUESTION,
};
use crate::config::Config;
use crate::validation::llm::{LLMValidator, ProperNounResponse};

/// Message to validate a proper noun
#[derive(Message)]
#[rtype(result = "()")]
pub struct ValidateProperNoun {
    pub word: String,
    pub message_id: u64,
    pub game_state: Addr<GameStateActor>,
    pub message_reaction: Addr<MessageReactionActor>,
}

/// Batch validation trigger message (internal)
#[derive(Message)]
#[rtype(result = "()")]
struct TriggerBatchValidation;

/// Entry in the validation queue
struct QueueEntry {
    word: String,
    message_id: u64,
    game_state: Addr<GameStateActor>,
    message_reaction: Addr<MessageReactionActor>,
}

/// Actor that handles LLM validation of proper nouns
pub struct LLMValidatorActor {
    llm_validator: Arc<Mutex<LLMValidator>>,
    queue: VecDeque<QueueEntry>,
    last_batch_time: Instant,
    max_batch_size: usize,
    batch_timeout_secs: u64,
}

impl LLMValidatorActor {
    pub fn new(config: &Config) -> Self {
        // Get the model name from environment variables with a default value
        let model = env::var("LLM_MODEL").unwrap_or_else(|_| "gemini-pro".to_string());

        // Set GEMINI_API_KEY environment variable in your system or config for the client
        let llm_validator = Arc::new(Mutex::new(LLMValidator::new(&model)));

        Self {
            llm_validator,
            queue: VecDeque::new(),
            last_batch_time: Instant::now(),
            max_batch_size: config.llm_batch_size,
            batch_timeout_secs: config.batch_timeout_secs,
        }
    }

    /// Check if we should trigger batch validation
    fn should_trigger_batch(&self) -> bool {
        self.queue.len() >= self.max_batch_size
            || (!self.queue.is_empty()
                && self.last_batch_time.elapsed() > Duration::from_secs(self.batch_timeout_secs))
    }
}

impl Default for LLMValidatorActor {
    fn default() -> Self {
        // Use default settings for the default implementation
        let model = env::var("LLM_MODEL").unwrap_or_else(|_| "gemini-pro".to_string());
        let llm_validator = Arc::new(Mutex::new(LLMValidator::new(&model)));

        Self {
            llm_validator,
            queue: VecDeque::new(),
            last_batch_time: Instant::now(),
            max_batch_size: 2,         // Default value
            batch_timeout_secs: 86400, // 24 hours default
        }
    }
}

impl Actor for LLMValidatorActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Set up periodic check for batch validation timeout
        ctx.run_interval(Duration::from_secs(10), |act, ctx| {
            if act.should_trigger_batch() {
                ctx.address().do_send(TriggerBatchValidation);
            }
        });
    }
}

impl Handler<ValidateProperNoun> for LLMValidatorActor {
    type Result = ();

    fn handle(&mut self, msg: ValidateProperNoun, ctx: &mut Context<Self>) -> Self::Result {
        // Add to queue
        self.queue.push_back(QueueEntry {
            word: msg.word,
            message_id: msg.message_id,
            game_state: msg.game_state,
            message_reaction: msg.message_reaction,
        });

        // Check if we should trigger batch validation
        if self.should_trigger_batch() {
            ctx.address().do_send(TriggerBatchValidation);
        }
    }
}

impl Handler<TriggerBatchValidation> for LLMValidatorActor {
    type Result = ();

    fn handle(&mut self, _msg: TriggerBatchValidation, _ctx: &mut Context<Self>) -> Self::Result {
        if self.queue.is_empty() {
            return;
        }

        debug!("Triggering batch validation for {} words", self.queue.len());

        // Clone items for validation
        let mut entries = Vec::new();
        while let Some(entry) = self.queue.pop_front() {
            entries.push(entry);
            if entries.len() >= self.max_batch_size {
                break;
            }
        }

        // Update last batch time
        self.last_batch_time = Instant::now();

        // Create word list for batch validation
        let words: Vec<String> = entries.iter().map(|e| e.word.clone()).collect();

        // Convert words to JSON string
        let words_json = match serde_json::to_string(&words) {
            Ok(json) => json,
            Err(e) => {
                error!("Error serializing words to JSON: {}", e);
                return;
            }
        };

        // Clone the Arc for async processing
        let validator = self.llm_validator.clone();

        // Process the batch in a separate thread to avoid LocalSet issues
        let handle = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                info!("Validating batch of {} words with LLM", words.len());
                // Get lock and perform batch validation with JSON string
                let mut guard = validator.lock().await;
                let validation_result = guard.validate_json_batch(&words_json).await;
                // Drop the guard as soon as possible
                drop(guard);

                let results: std::collections::HashMap<String, ProperNounResponse> =
                    match validation_result {
                        Ok(batch_results) => batch_results,
                        Err(e) => {
                            error!("Error in batch validation: {}", e);
                            std::collections::HashMap::new()
                        }
                    };

                // Process each entry with the results from batch validation
                for entry in entries {
                    let word = &entry.word;
                    if let Some(response) = results.get(word) {
                        let is_valid = response.is_proper_noun;

                        // Delete question mark reaction if present
                        debug!("Deleting question mark reaction for word '{}'", word);
                        entry.message_reaction.do_send(DeleteReaction {
                            message_id: entry.message_id,
                            reaction: EMOJI_QUESTION,
                        });

                        if is_valid {
                            // Mark as valid in game state
                            debug!(
                                "LLM validated '{}' as a proper noun, marking as valid",
                                word
                            );
                            entry.game_state.do_send(MarkWordValidity {
                                message_id: entry.message_id,
                                is_valid: true,
                            });

                            // Add checkmark reaction
                            entry.message_reaction.do_send(
                                crate::actors::message_reaction::AddReaction {
                                    message_id: entry.message_id,
                                    reaction: EMOJI_CHECK,
                                },
                            );

                            info!("'{}' validated as proper noun by LLM", word);
                        } else {
                            // Add X reaction
                            debug!(
                                "LLM rejected '{}' as a proper noun, marking as invalid",
                                word
                            );
                            entry.message_reaction.do_send(
                                crate::actors::message_reaction::AddReaction {
                                    message_id: entry.message_id,
                                    reaction: EMOJI_CROSS,
                                },
                            );

                            info!("'{}' rejected as proper noun by LLM", word);
                        }
                    } else {
                        error!("Word '{}' not found in batch results", word);
                        // Add X reaction as fallback
                        entry.message_reaction.do_send(
                            crate::actors::message_reaction::AddReaction {
                                message_id: entry.message_id,
                                reaction: EMOJI_CROSS,
                            },
                        );
                    }
                }
            });
        });

        // Don't wait for the thread
        std::mem::drop(handle);
    }
}
