# Hello, Cortex agent

## Prerequisites

```bash
cargo build -p cortex-cli
# optional: copy env template
# cp .env.example .env
```

## Offline (mock provider)

Default `config/models.toml` uses the **mock** provider — no API key required.

```bash
# From the repo root (so config/models.toml is found):
cargo run -p cortex-cli -- run "What is Cortex?"

# Or after init in any directory:
cargo run -p cortex-cli -- init
cargo run -p cortex-cli -- run "hello" --workspace .
```

## List tools and models

```bash
cargo run -p cortex-cli -- tools list
cargo run -p cortex-cli -- models list
```

## Live model (OpenAI-compatible)

```bash
export OPENAI_API_KEY=sk-...
# Edit .cortex/models.toml:
#   default_model = "openai"
cargo run -p cortex-cli -- run "Add a comment to README" --yolo --max-turns 16
```

## Ollama

```bash
# ollama serve && ollama pull qwen2.5-coder
cargo run -p cortex-cli -- run "Summarize Cargo.toml" --model ollama --yolo
```

## Interactive chat

```bash
cargo run -p cortex-cli -- chat --model ollama --yolo
# type messages; /quit to exit
```
