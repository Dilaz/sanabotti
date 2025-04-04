use miette::SourceSpan;
use std::collections::HashSet;

use crate::error::{Result, ValidationError};

/// Validates that a word follows the game rules in relation to a previous word
#[derive(Debug, Clone, Default)]
pub struct RulesValidator {
    /// Set of previously used words in the current game
    used_words: HashSet<String>,
}

impl RulesValidator {
    /// Check if the new word follows the game rules in relation to the previous word:
    /// 1. One letter changed, added, or removed
    /// 2. Not previously used in this game session
    ///
    /// Returns Ok(()) if valid, or appropriate error if not
    pub fn validate_move(&mut self, previous_word: &str, new_word: &str) -> Result<()> {
        let previous = previous_word.trim().to_lowercase();
        let new = new_word.trim().to_lowercase();

        // Check if the word has been used before
        if self.used_words.contains(&new) {
            return Err(ValidationError::AlreadyUsed(new.clone()).into());
        }

        // Check if the word follows the one-letter rule
        let (is_valid, violation_span) = check_one_letter_difference(&previous, &new);
        if !is_valid {
            return Err(ValidationError::RuleViolation {
                word: new.clone(),
                span: violation_span,
                reason: "Word must differ by exactly one letter (added, removed, or changed)"
                    .to_string(),
            }
            .into());
        }

        // Valid move - add the word to the used words set
        self.used_words.insert(new);
        Ok(())
    }

    /// Backward-compatible version that returns a boolean
    pub fn is_valid_move(&mut self, previous_word: &str, new_word: &str) -> bool {
        self.validate_move(previous_word, new_word).is_ok()
    }

    /// Add a word to the list of used words (for initialization)
    pub fn add_word(&mut self, word: &str) {
        let word = word.trim().to_lowercase();
        self.used_words.insert(word);
    }

    /// Get the number of words used so far
    pub fn word_count(&self) -> usize {
        self.used_words.len()
    }

    /// Reset the game state
    pub fn reset(&mut self) {
        self.used_words.clear();
    }
}

/// Check if two words differ by exactly one letter (changed, added, or removed)
/// Returns (is_valid, optional_violation_span)
fn check_one_letter_difference(word1: &str, word2: &str) -> (bool, Option<SourceSpan>) {
    let len1 = word1.chars().count();
    let len2 = word2.chars().count();

    // If length difference is more than 1, return false
    if (len1 as isize - len2 as isize).abs() > 1 {
        return (false, None);
    }

    // Convert to character vectors for easier comparison
    let chars1: Vec<char> = word1.chars().collect();
    let chars2: Vec<char> = word2.chars().collect();

    // If lengths are equal, one letter might have been changed
    if len1 == len2 {
        let mut differences = 0;

        for i in 0..len1 {
            if chars1[i] != chars2[i] {
                differences += 1;
                if differences > 1 {
                    return (false, None);
                }
            }
        }

        // One letter change is valid, no change is invalid
        if differences == 1 {
            return (true, None);
        } else {
            // No change - the words are identical
            return (false, Some(SourceSpan::from((0, word2.len()))));
        }
    }

    // At this point we know lengths differ by exactly 1
    // Check if one letter was added or removed

    let (shorter, longer) = if len1 < len2 {
        (&chars1, &chars2)
    } else {
        (&chars2, &chars1)
    };

    let mut long_idx = 0;
    let mut short_idx = 0;
    let mut found_difference = false;

    // Check if we can match the shorter word to the longer one
    // by skipping exactly one letter in the longer word
    while short_idx < shorter.len() && long_idx < longer.len() {
        if shorter[short_idx] == longer[long_idx] {
            short_idx += 1;
            long_idx += 1;
        } else {
            // Found a difference, can only have one
            if found_difference {
                return (false, None);
            }
            found_difference = true;
            long_idx += 1;
        }
    }

    // Should have processed all chars in the shorter word
    // and either all or all but one in the longer word
    (true, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one_letter_difference() {
        // One letter changed
        assert!(check_one_letter_difference("kissa", "kassa").0);

        // One letter added
        assert!(check_one_letter_difference("kissa", "kissan").0);

        // One letter removed
        assert!(check_one_letter_difference("kissan", "kissa").0);

        // More than one letter changed
        assert!(!check_one_letter_difference("kissa", "koira").0);

        // No change
        assert!(!check_one_letter_difference("kissa", "kissa").0);

        // Too many letters different
        assert!(!check_one_letter_difference("kissa", "kissoilla").0);
    }

    #[test]
    fn test_rules_validator() {
        let mut validator = RulesValidator::default();

        // First word, consider it valid as there's no previous word
        validator.add_word("kissa");

        // Valid moves: one letter changed, added, or removed
        assert!(validator.is_valid_move("kissa", "kassa")); // Change
        assert!(validator.is_valid_move("kissa", "kissat")); // Add
        assert!(validator.is_valid_move("kissat", "kissa")); // Remove

        // Invalid: No change
        assert!(!validator.is_valid_move("kissa", "kissa"));
        let result = validator.validate_move("kissa", "kissa");
        assert!(result.is_err());

        // Invalid: More than one letter different
        assert!(!validator.is_valid_move("kissa", "koira"));
        let result = validator.validate_move("kissa", "koira");
        assert!(result.is_err());

        // Invalid: Word used before
        assert!(!validator.is_valid_move("kissat", "kassa")); // Already used "kassa"
        let result = validator.validate_move("kissat", "kassa");
        assert!(result.is_err());
        if let Err(e) = result {
            match e {
                crate::error::Error::Validation(ValidationError::AlreadyUsed(_)) => {}
                _ => panic!("Expected AlreadyUsed error"),
            }
        }

        // Check word count
        assert_eq!(validator.word_count(), 4); // "kissa", "kassa", "kissat", plus "kissa" from initialization

        // Reset and check
        validator.reset();
        assert_eq!(validator.word_count(), 0);
    }
}
