# cc_sline

A status line generator for Claude Code. Reads JSON input from stdin and outputs a formatted status line with model info, directory, git branch, token usage, and context percentage.

## Output Example

```text
🤖 Claude Opus | 📁 my-project | 🌿 main | 🪙 65.0K | 40%
```

### Components

| Icon | Description                                                                    |
| ---- | ------------------------------------------------------------------------------ |
| 🤖   | Model name                                                                     |
| 📁   | Current directory                                                              |
| 🌿   | Git branch (only shown in git repositories)                                    |
| 🪙   | Token count (formatted as K/M)                                                 |
| %    | Context usage percentage (color-coded: green < 70%, yellow 70-89%, red >= 90%) |

## Requirements

- Rust 1.70+
- Git (for branch detection)

## Build

```bash
cargo build --release
```

The binary will be at `target/release/cc_sline`.

## Install

Install from GitHub:

```bash
cargo install --git https://github.com/byplayer/claude_code_statusline.git --root ~/.local
```

Or install to `~/.local/bin` from local source:

```bash
cargo install --path . --root ~/.local
```

Or build and copy manually:

```bash
cargo build --release && cp target/release/cc_sline ~/.local/bin/
```

## Test

```bash
cargo test
```

## Usage

Pipe JSON data to stdin:

```bash
echo '{"model":{"display_name":"Claude Opus"},"cwd":"/path/to/project","context_window":{"context_window_size":200000,"current_usage":{"input_tokens":50000}}}' | cc_sline
```

### Input JSON Schema

```json
{
  "model": {
    "display_name": "string"
  },
  "workspace": {
    "current_dir": "string"
  },
  "cwd": "string",
  "context_window": {
    "context_window_size": 0,
    "current_usage": {
      "input_tokens": 0,
      "cache_creation_input_tokens": 0,
      "cache_read_input_tokens": 0
    }
  }
}
```

- `workspace.current_dir` takes precedence over `cwd`
- All fields are optional (defaults to "Unknown" model, "." directory, 0 tokens)

## Environment Variables

### `CC_STATUSLINE_NO_MODEL`

Set to `1` to hide the model name from the status line output.

```bash
CC_STATUSLINE_NO_MODEL=1 claude
```

Output without model:

```text
📁 my-project | 🌿 main | 🪙 65.0K | 40%
```

## Debug Logging

Set the `STATUSLINE_DEBUG` environment variable to enable debug logging:

```bash
STATUSLINE_DEBUG=1 claude
```

Logs are written to `~/.claude/status_line_debug.log` with the following format:

```text
[2025-02-07 14:30:45.123 pid:12345] message text here
```

Each entry includes a timestamp, process ID, and message. The log tracks stdin reading, git branch detection, and status line building operations.

Log files are automatically rotated when the size exceeds 1MB. Up to 5 rotated files are kept (`status_line_debug.log.1` through `status_line_debug.log.5`), with older files being deleted.

## License

MIT
