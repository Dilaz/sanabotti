use actix::{Actor, Context, Handler, Message};
use std::collections::VecDeque;
use tracing::info;

use crate::validation::rules::RulesValidator;

/// The maximum number of previous words to store
const MAX_HISTORY: usize = 2;

#[derive(Debug, Clone)]
pub struct WordEntry {
    pub word: String,
    pub user_id: u64,
    pub message_id: u64,
    pub is_valid: bool,
}

/// Message to register a new word
#[derive(Message)]
#[rtype(result = "bool")]
pub struct RegisterWord {
    pub word: String,
    pub user_id: u64,
    pub message_id: u64,
}

/// Message to check if a word is valid according to game rules
#[derive(Message)]
#[rtype(result = "bool")]
pub struct ValidateGameRules {
    pub word: String,
}

/// Message to get the last valid word
#[derive(Message)]
#[rtype(result = "Option<String>")]
pub struct GetLastValidWord;

/// Message to mark a word as valid or invalid
#[derive(Message)]
#[rtype(result = "()")]
pub struct MarkWordValidity {
    pub message_id: u64,
    pub is_valid: bool,
}

/// Message to reset the game state
#[derive(Message)]
#[rtype(result = "()")]
pub struct ResetGame;

/// Actor that maintains the game state
pub struct GameStateActor {
    /// History of words in the game
    word_history: VecDeque<WordEntry>,

    /// Rules validator
    rules_validator: RulesValidator,

    /// The last valid word in the game
    last_valid_word: Option<String>,

    /// The last word that follows game rules (might be pending LLM validation)
    last_game_rule_word: Option<String>,
}

impl Default for GameStateActor {
    fn default() -> Self {
        Self::new()
    }
}

impl GameStateActor {
    pub fn new() -> Self {
        Self {
            word_history: VecDeque::with_capacity(MAX_HISTORY),
            rules_validator: RulesValidator::default(),
            last_valid_word: None,
            last_game_rule_word: None,
        }
    }

    /// Add a word to the history and maintain maximum size
    fn add_to_history(&mut self, entry: WordEntry) {
        self.word_history.push_back(entry);

        // Keep history at maximum size
        if self.word_history.len() > MAX_HISTORY {
            self.word_history.pop_front();
        }
    }
}

impl Actor for GameStateActor {
    type Context = Context<Self>;
}

impl Handler<RegisterWord> for GameStateActor {
    type Result = bool;

    fn handle(&mut self, msg: RegisterWord, _ctx: &mut Context<Self>) -> Self::Result {
        // Create the entry (initially not validated)
        let entry = WordEntry {
            word: msg.word.clone(),
            user_id: msg.user_id,
            message_id: msg.message_id,
            is_valid: false,
        };

        info!(
            "Registering word '{}' (message ID: {})",
            msg.word, msg.message_id
        );

        // Add to history
        self.add_to_history(entry);

        // Return true as acknowledgment
        true
    }
}

impl Handler<ValidateGameRules> for GameStateActor {
    type Result = bool;

    fn handle(&mut self, msg: ValidateGameRules, _ctx: &mut Context<Self>) -> Self::Result {
        info!("Validating game rules for word: '{}'", msg.word);

        // Use last_game_rule_word if available, otherwise fall back to last_valid_word
        let reference_word = self
            .last_game_rule_word
            .as_ref()
            .or(self.last_valid_word.as_ref());

        if let Some(last_word) = reference_word {
            info!("Comparing with last rule-valid word: '{}'", last_word);
            let is_valid = self.rules_validator.is_valid_move(last_word, &msg.word);

            // If valid, update the last_game_rule_word and add to rules validator
            if is_valid {
                info!(
                    "Word '{}' follows game rules, updating last_game_rule_word",
                    msg.word
                );
                self.last_game_rule_word = Some(msg.word.clone());
                self.rules_validator.add_word(&msg.word);
            }

            info!("Word '{}' follows game rules: {}", msg.word, is_valid);
            is_valid
        } else {
            // If there's no last valid word, consider first word valid
            // and add it to the used words list
            info!(
                "No previous valid word, accepting '{}' as first word",
                msg.word
            );
            self.last_game_rule_word = Some(msg.word.clone());
            self.rules_validator.add_word(&msg.word);
            true
        }
    }
}

impl Handler<GetLastValidWord> for GameStateActor {
    type Result = Option<String>;

    fn handle(&mut self, _msg: GetLastValidWord, _ctx: &mut Context<Self>) -> Self::Result {
        self.last_valid_word.clone()
    }
}

impl Handler<MarkWordValidity> for GameStateActor {
    type Result = ();

    fn handle(&mut self, msg: MarkWordValidity, _ctx: &mut Context<Self>) -> Self::Result {
        info!(
            "Marking message {} as {}",
            msg.message_id,
            if msg.is_valid { "valid" } else { "invalid" }
        );

        // Find the entry by message ID and update its validity
        let mut updated = false;
        for entry in &mut self.word_history {
            if entry.message_id == msg.message_id {
                entry.is_valid = msg.is_valid;
                updated = true;

                // If valid, update the last valid word
                if msg.is_valid {
                    info!(
                        "Updating last valid word from {} to: {}",
                        self.last_valid_word.as_deref().unwrap_or("<none>"),
                        entry.word
                    );
                    self.last_valid_word = Some(entry.word.clone());
                }

                break;
            }
        }

        if !updated {
            info!(
                "Could not find message {} in word history to mark validity",
                msg.message_id
            );
        }
    }
}

impl Handler<ResetGame> for GameStateActor {
    type Result = ();

    fn handle(&mut self, _msg: ResetGame, _ctx: &mut Context<Self>) -> Self::Result {
        // Clear history and reset rules validator
        self.word_history.clear();
        self.rules_validator.reset();
        self.last_valid_word = None;
        self.last_game_rule_word = None;

        info!("Game state has been reset");
    }
}
