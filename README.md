# Numerobotti - Finnish Word Game Discord Bot

This Discord bot monitors a specific channel for a Finnish word game where players create new Finnish words by changing, removing, or adding one letter to previous words. The bot validates words against a Finnish dictionary and uses LLM for proper noun validation.

## Architecture

The bot is built using a multi-actor model with the following components:

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│                 │     │                 │     │                 │
│  Discord Bot    │────▶│ Word Validator  │────▶│  Game State     │
│                 │     │                 │     │                 │
└─────────────────┘     └────────┬────────┘     └─────────────────┘
                                 │
                     ┌───────────┴───────────┐
                     │                       │
           ┌─────────▼─────────┐   ┌─────────▼─────────┐
           │                   │   │                   │
           │  LLM Validator    │   │ Message Reaction  │
           │                   │   │                   │
           └───────────────────┘   └───────────────────┘
```

- **Discord Bot**: Interfaces with Discord, receives messages, and initializes the actor system
- **Word Validator**: Validates words against a Finnish dictionary and game rules
- **Game State**: Maintains the game state, including word history and rule enforcement
- **LLM Validator**: Validates proper nouns using an LLM (batched for efficiency)
- **Message Reaction**: Manages adding/clearing reactions to messages

## Setup

### Standard Setup

1. Clone this repository
2. Copy `.env.example` to `.env` and fill in your configuration values
3. Ensure you have a Finnish dictionary file (text file with one word per line) and update the path in `.env`
4. Install Rust if you haven't already (https://rustup.rs/)
5. Run the bot:

```bash
cargo run --release
```

### Getting the Finnish Word List

You can download the Finnish word list from the Institute for the Languages of Finland (Kotus):
1. Visit [kotus.fi/sanakirjat/kielitoimiston-sanakirja/nykysuomen-sana-aineistot/nykysuomen-sanalista/](https://kotus.fi/sanakirjat/kielitoimiston-sanakirja/nykysuomen-sana-aineistot/nykysuomen-sanalista/)
2. Download the latest word list file (e.g., nykysuomensanalista2024.csv)
3. Process it using the following command to create a proper word list:

```bash
cat nykysuomensanalista2024.csv | cut -d '     ' -f 1 | uniq > finnish_words.txt
```

4. Move the resulting `finnish_words.txt` to your configured dictionary path (default: `./data/finnish_words.txt`)

### Docker Setup

1. Clone this repository
2. Copy `.env.example` to `.env` and fill in your configuration values
3. Build and run using Docker Compose:

```bash
docker-compose up -d
```

To view logs:

```bash
docker-compose logs -f
```

To stop the bot:

```bash
docker-compose down
```

## Features

- Validates Finnish words against a dictionary
- Uses LLM to validate proper nouns not found in the dictionary
- Reacts to messages to indicate word validity
- Enforces game rules (one letter change/addition/removal)
- Tracks game history to prevent word reuse

## Configuration

The following environment variables can be set in your `.env` file:

- `DISCORD_TOKEN`: Your Discord bot token (required)
- `TARGET_CHANNEL_ID`: The ID of the channel to monitor (required)
- `DICTIONARY_FILE_PATH`: Path to the Finnish word list file (default: `./data/finnish_words.txt`)
- `BOT_ACTIVITY`: Custom activity status for the bot (default: "Finnish Word Game")
- `LLM_BATCH_SIZE`: Number of words to batch for LLM validation (default: 2)
- `LLM_BATCH_TIMEOUT_SECS`: Timeout for LLM batching in seconds (default: 86400 - 24 hours)

See `.env.example` for all configuration options.

## License

MIT

For more information on the MIT License, visit [choosealicense.com/licenses/mit/](https://choosealicense.com/licenses/mit/).
