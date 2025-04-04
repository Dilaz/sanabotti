use actix::{Actor, Addr, Context, Handler, Message};
use std::thread;
use tracing::{info, warn};

use crate::actors::game_state::{GameStateActor, ValidateGameRules};
use crate::actors::llm_validator::LLMValidatorActor;
use crate::actors::message_reaction::MessageReactionActor;
use crate::error::Result;
use crate::validation::dictionary::DictionaryValidator;

/// Message to validate a word
#[derive(Message)]
#[rtype(result = "()")]
pub struct ValidateWord {
    pub word: String,
    pub message_id: u64,
    pub user_id: u64,
}

/// Actor that validates words against a dictionary and game rules
pub struct WordValidatorActor {
    dictionary_validator: DictionaryValidator,
    game_state: Addr<GameStateActor>,
    llm_validator: Addr<LLMValidatorActor>,
    message_reaction: Addr<MessageReactionActor>,
}

impl WordValidatorActor {
    pub fn new(
        dictionary_path: &str,
        game_state: Addr<GameStateActor>,
        llm_validator: Addr<LLMValidatorActor>,
        message_reaction: Addr<MessageReactionActor>,
    ) -> Result<Self> {
        let dictionary_validator = DictionaryValidator::new(dictionary_path)?;

        Ok(Self {
            dictionary_validator,
            game_state,
            llm_validator,
            message_reaction,
        })
    }
}

impl Actor for WordValidatorActor {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Context<Self>) {
        info!("WordValidatorActor started");
    }
}

impl Handler<ValidateWord> for WordValidatorActor {
    type Result = ();

    fn handle(&mut self, msg: ValidateWord, _ctx: &mut Context<Self>) -> Self::Result {
        info!("===============================");
        info!("RECEIVED WORD FOR VALIDATION: '{}'", msg.word);
        info!("===============================");

        let word = msg.word.trim().to_lowercase();

        info!(
            "Validating word: '{}' (message_id: {})",
            word, msg.message_id
        );

        // Skip empty words
        if word.is_empty() {
            info!("Skipping empty word");
            return;
        }

        // Skip words with numbers and non-alphabetic characters
        if !word.chars().all(|c| c.is_alphabetic()) || word.chars().any(|c| c.is_ascii_digit()) {
            info!("Skipping word with numbers or non-alphabetic characters");
            return;
        }

        // First, check if it follows game rules
        let game_state = self.game_state.clone();
        let message_reaction = self.message_reaction.clone();
        let message_id = msg.message_id;

        // Registers the word in game state
        info!("Registering word '{}' in game state", word);
        self.game_state
            .do_send(crate::actors::game_state::RegisterWord {
                word: word.clone(),
                user_id: msg.user_id,
                message_id: msg.message_id,
            });

        // Check if the word is in dictionary
        let is_in_dictionary = self.dictionary_validator.is_valid_word(&word);
        info!("Word '{}' in dictionary: {}", word, is_in_dictionary);

        // Store word for later use
        let word_clone = word.clone();
        let llm_validator = self.llm_validator.clone();

        // Use a separate thread to handle async operations without LocalSet
        let handle = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            // Use a timeout to ensure the thread doesn't hang forever
            rt.block_on(async {
                // Always check game rules first
                info!("Checking if '{}' follows game rules", word_clone);
                match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    game_state.send(ValidateGameRules { word: word_clone.clone() })
                ).await {
                    Ok(result) => {
                        match result {
                            Ok(is_valid_move) => {
                                if is_valid_move {
                                    // Word follows game rules
                                    if is_in_dictionary {
                                        // Valid word and valid move, add checkmark
                                        info!("Adding ✅ reaction to message {}", message_id);
                                        message_reaction.do_send(crate::actors::message_reaction::AddReaction {
                                            message_id,
                                            reaction: '✅',
                                        });

                                        // Mark as valid in game state
                                        game_state.do_send(crate::actors::game_state::MarkWordValidity {
                                            message_id,
                                            is_valid: true,
                                        });

                                        info!("Word '{}' is valid (in dictionary and follows rules)", word_clone);
                                    } else {
                                        // Word not in dictionary but follows rules, send to LLM validator
                                        info!("Adding ❓ reaction to message {}", message_id);
                                        message_reaction.do_send(crate::actors::message_reaction::AddReaction {
                                            message_id,
                                            reaction: '❓',
                                        });

                                        // Send to LLM validator for proper noun check with capitalized word
                                        let capitalized_word = word_clone.chars()
                                            .enumerate()
                                            .map(|(i, c)| if i == 0 { c.to_uppercase().to_string() } else { c.to_string() })
                                            .collect::<String>();

                                        info!("Sending '{}' to LLM validator", capitalized_word);
                                        llm_validator.do_send(crate::actors::llm_validator::ValidateProperNoun {
                                            word: capitalized_word,
                                            message_id,
                                            game_state: game_state.clone(),
                                            message_reaction: message_reaction.clone(),
                                        });

                                        info!("Word '{}' not in dictionary, sent to LLM for validation", word_clone);
                                    }
                                } else {
                                    // Word doesn't follow game rules, add X (regardless of dictionary status)
                                    info!("Adding ❌ reaction to message {}", message_id);
                                    message_reaction.do_send(crate::actors::message_reaction::AddReaction {
                                        message_id,
                                        reaction: '❌',
                                    });

                                    info!("Word '{}' doesn't follow game rules, marked as invalid", word_clone);
                                }
                            },
                            Err(e) => {
                                warn!("Failed to validate game rules for '{}': {:?}", word_clone, e);
                            }
                        }
                    },
                    Err(_) => {
                        warn!("Timeout while validating game rules for '{}'", word_clone);
                    }
                }
            });
        });

        // Don't block the actor system by waiting for the thread
        std::mem::drop(handle);
        info!("Game rules validation thread for '{}' started", word);
    }
}
