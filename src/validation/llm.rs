use rig::{completion::Prompt, providers::gemini};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use tracing::{debug, info};

use crate::error::{LLMError, Result};

const PROMPT: &str = "Your task is to validate a list of words and provide information about them. For each word in the provided list, you need to determine if it meets **both** of the following criteria:

1.  **Capitalized:** The word MUST start with an uppercase letter.
2.  **Proper Noun:** The word MUST be a proper noun in either English or Finnish. A proper noun is a name used for an individual person, place, organization, brand, title, month, day, etc. Common nouns (like \"table\", \"house\", \"juokseminen\" [running]), even if capitalized incorrectly or at the start of a sentence, are generally not proper nouns unless they are part of a specific name (e.g., the brand \"Apple\").

**Input:** A list of words.

**Output:** A single JSON array.
*   Each element in the array should be a JSON object representing one word from the input list.
*   Each object must have the following three keys:
    *   `\"word\"`: The original word from the input list (string).
    *   `\"is_proper_noun\"`: A boolean value. `true` if the word meets **both** criteria (is capitalized AND is a proper noun in English or Finnish). `false` otherwise.
    *   `\"explanation\"`: A short explanation **in Finnish** (string).
        *   If `is_proper_noun` is `true`, briefly explain **in Finnish** what the proper noun refers to (e.g., \"Ranskan pääkaupunki\", \"Suomalainen designyritys\", \"Amerikkalainen teknologiayritys\").
        *   If `is_proper_noun` is `false`, briefly state **in Finnish** the reason why it failed the criteria (e.g., \"Yleisnimi, ei erisnimi\", \"Ei isolla alkukirjaimella\", \"Ei tunnistettu sana tai erisnimi\").

**Example:**

If the input list is:
`[\"Microsoft\", \"London\", \"Helsinki\", \"Marimekko\", \"Table\", \"juokseminen\", \"paris\", \"suomi\", \"bababpap\", \"Apple\"]`

The expected output JSON array is:
```json
[
  {
    \"word\": \"Microsoft\",
    \"is_proper_noun\": true,
    \"explanation\": \"Amerikkalainen monikansallinen teknologiayritys.\"
  },
  {
    \"word\": \"London\",
    \"is_proper_noun\": true,
    \"explanation\": \"Englannin ja Yhdistyneen kuningaskunnan pääkaupunki ja suurin kaupunki.\"
  },
  {
    \"word\": \"Helsinki\",
    \"is_proper_noun\": true,
    \"explanation\": \"Suomen pääkaupunki ja väkirikkain kaupunki.\"
  },
  {
    \"word\": \"Marimekko\",
    \"is_proper_noun\": true,
    \"explanation\": \"Suomalainen designyritys, joka tunnetaan rohkeista kuvioistaan.\"
  },
  {
    \"word\": \"Table\",
    \"is_proper_noun\": false,
    \"explanation\": \"Yleisnimi, ei erisnimi.\"
  },
  {
    \"word\": \"juokseminen\",
    \"is_proper_noun\": false,
    \"explanation\": \"Ei isolla alkukirjaimella ja on yleisnimi (tarkoittaa 'running').\"
  },
  {
    \"word\": \"paris\",
    \"is_proper_noun\": false,
    \"explanation\": \"Ei isolla alkukirjaimella (viittaa todennäköisesti Ranskan pääkaupunkiin).\"
  },
  {
    \"word\": \"suomi\",
    \"is_proper_noun\": false,
    \"explanation\": \"Ei isolla alkukirjaimella (Suomen maan nimi).\"
  },
  {
    \"word\": \"bababpap\",
    \"is_proper_noun\": false,
    \"explanation\": \"Ei tunnistettu sana tai erisnimi.\"
  },
  {
    \"word\": \"Apple\",
    \"is_proper_noun\": true,
    \"explanation\": \"Amerikkalainen monikansallinen teknologiayritys (myös hedelmä, mutta iso alkukirjain viittaa brändiin).\"
  }
]```

Now, please validate the following list of words and provide the output strictly in the specified JSON array format:

```json
{}
```";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProperNounResponse {
    pub word: String,
    pub is_proper_noun: bool,
    pub explanation: String,
}

/// Validates if a word is a proper noun using an LLM
#[derive(Default)]
pub struct LLMValidator {
    model: String,
    cache: HashMap<String, ProperNounResponse>,
    client: Option<gemini::Client>,
}

impl LLMValidator {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_string(),
            cache: HashMap::new(),
            client: Some(gemini::Client::from_env()),
        }
    }

    /// Validates a batch of words sent as a JSON string representation of a list
    /// Returns a HashMap with word to validation result mapping
    pub async fn validate_json_batch(
        &mut self,
        words_json: &str,
    ) -> Result<HashMap<String, ProperNounResponse>> {
        // Parse JSON string into a Vec<String>
        let words: Vec<String> = serde_json::from_str(words_json)
            .map_err(|e| LLMError::ApiError(format!("Failed to parse JSON word list: {}", e)))?;

        if words.is_empty() {
            return Ok(HashMap::new());
        }

        info!("JSON batch validating {} words", words.len());

        // First check cache for existing results
        let mut results: HashMap<String, ProperNounResponse> = HashMap::new();
        let mut words_to_check = Vec::new();

        for word in &words {
            let word_lower = word.trim().to_lowercase();

            if let Some(result) = self.cache.get(&word_lower) {
                results.insert(word.clone(), result.clone());
            } else {
                words_to_check.push(word.clone());
            }
        }

        if words_to_check.is_empty() {
            return Ok(results);
        }

        // Construct the prompt with the JSON array of words
        let words_array_json = serde_json::to_string(&words_to_check)
            .map_err(|e| LLMError::ApiError(format!("Failed to serialize words to JSON: {}", e)))?;

        let prompt = PROMPT.replace("{}", &words_array_json);

        debug!("Prompt: {}", prompt);

        // Get a client instance
        let client = self
            .client
            .as_ref()
            .unwrap_or_else(|| panic!("Gemini client not initialized"));

        let agent = client.agent(&self.model).build();

        // Make the API call with all words at once
        let response = agent
            .prompt(prompt)
            .await
            .map_err(|e| LLMError::ApiError(format!("Gemini API request failed: {}", e)))?;

        // Parse the JSON response
        let response_text = response.trim();

        debug!("Response: {}", response_text);

        // Try to extract JSON from the response (in case LLM wrapped it in code blocks or text)
        let json_text = if response_text.contains("[") && response_text.contains("]") {
            let start = response_text.find("[").unwrap_or(0);
            let end = response_text
                .rfind("]")
                .map(|pos| pos + 1)
                .unwrap_or(response_text.len());
            &response_text[start..end]
        } else {
            response_text
        };

        // Parse the JSON array response
        let validation_objects: Vec<ProperNounResponse> =
            serde_json::from_str(json_text).map_err(|e| {
                LLMError::ApiError(format!(
                    "Failed to parse LLM response as JSON: {}, response was: {}",
                    e, response_text
                ))
            })?;

        // Convert to HashMap<String, bool> format
        let validation_results: HashMap<String, bool> = validation_objects
            .into_iter()
            .map(|resp| (resp.word.clone(), resp.is_proper_noun))
            .collect();

        // Update our cache with new results
        for (word, is_valid) in &validation_results {
            self.cache.insert(
                word.trim().to_lowercase(),
                ProperNounResponse {
                    word: word.clone(),
                    is_proper_noun: *is_valid,
                    explanation: "".to_string(),
                },
            );

            // Also add to results
            results.insert(
                word.clone(),
                ProperNounResponse {
                    word: word.clone(),
                    is_proper_noun: *is_valid,
                    explanation: "".to_string(),
                },
            );
        }

        info!("Batch validated {} words with JSON approach", words.len());
        Ok(results)
    }
}
