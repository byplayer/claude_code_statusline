# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-06-09

### Changed

- Model name is now hidden by default; set `CC_STATUSLINE_MODEL=1` to show it (replaces `CC_STATUSLINE_NO_MODEL`)

## [0.4.0] - 2026-04-13

### Added

- Cost display after branch name in status line
- GitHub Actions workflow to run tests on PR

## [0.3.0] - 2026-02-14

### Added

- `CC_STATUSLINE_NO_MODEL` environment variable to hide model name from status line output

## [0.2.0] - 2025-01-22

### Added

- GitHub installation instructions in README

### Fixed

- Add stdin timeout and improve error handling

## [0.1.0] - 2025-12-25

### Added

- Initial release
- Parse Claude Code status JSON from stdin
- Display model name with 🤖 emoji
- Display current directory with 📁 emoji
- Display git branch with 🌿 emoji (when in git repository)
- Display token count with 🪙 emoji (formatted as K/M units)
- Display context usage percentage with color coding:
  - Green: < 70%
  - Yellow: 70-89%
  - Red: >= 90%
- Support for `workspace.current_dir` and `cwd` fields
- Support for multiple token types: `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`
- Unit tests for token formatting, status line building, and git branch detection
