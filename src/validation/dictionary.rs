use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use tracing::info;

use crate::error::{DictionaryError, Result};

pub struct DictionaryValidator {
    words: HashSet<String>,
}

impl DictionaryValidator {
    pub fn new(dictionary_path: &str) -> Result<Self> {
        let mut words = HashSet::new();

        info!("Loading dictionary from {}", dictionary_path);

        let file = File::open(Path::new(dictionary_path)).map_err(DictionaryError::LoadError)?;

        let reader = io::BufReader::new(file);

        for line in reader.lines() {
            let line = line.map_err(DictionaryError::LoadError)?;
            let word = line.trim().to_lowercase();
            if !word.is_empty() {
                words.insert(word);
            }
        }

        if words.is_empty() {
            return Err(DictionaryError::EmptyDictionary.into());
        }

        info!("Loaded {} words from dictionary", words.len());

        Ok(Self { words })
    }

    pub fn is_valid_word(&self, word: &str) -> bool {
        let word = word.trim().to_lowercase();
        self.words.contains(&word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_dictionary_validator() -> std::io::Result<()> {
        // Create a temporary dictionary file
        let mut file = NamedTempFile::new()?;
        writeln!(file, "kissa")?;
        writeln!(file, "koira")?;
        writeln!(file, "talo")?;

        let validator = DictionaryValidator::new(file.path().to_str().unwrap()).unwrap();

        assert!(validator.is_valid_word("kissa"));
        assert!(validator.is_valid_word("KISSA")); // Case insensitive
        assert!(validator.is_valid_word("koira"));
        assert!(validator.is_valid_word("talo"));
        assert!(!validator.is_valid_word("autossa"));
        assert!(!validator.is_valid_word(""));

        Ok(())
    }

    #[test]
    fn test_empty_dictionary() -> std::io::Result<()> {
        // Create an empty dictionary file
        let file = NamedTempFile::new()?;

        let result = DictionaryValidator::new(file.path().to_str().unwrap());
        assert!(result.is_err());

        if let Err(e) = result {
            match e {
                crate::error::Error::Dictionary(DictionaryError::EmptyDictionary) => {}
                _ => panic!("Expected EmptyDictionary error"),
            }
        }

        Ok(())
    }
}
