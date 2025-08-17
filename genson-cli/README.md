# Genson CLI

Standalone command-line tool for JSON schema inference using genson-rs.

## Usage

```bash
# From file
cargo run -- input.json

# From stdin
echo '{"name": "test", "value": 42}' | cargo run

# With options
cargo run -- --ndjson data.jsonl
cargo run -- --no-ignore-array array-data.json
```

## License

MIT License