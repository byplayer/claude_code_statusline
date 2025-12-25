use serde::Deserialize;
use std::io::{self, Read};
use std::path::Path;
use std::process::Command;

#[derive(Deserialize, Default)]
struct Model {
    display_name: Option<String>,
}

#[derive(Deserialize, Default)]
struct Workspace {
    current_dir: Option<String>,
}

#[derive(Deserialize, Default)]
struct CurrentUsage {
    input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

#[derive(Deserialize, Default)]
struct ContextWindow {
    context_window_size: Option<u64>,
    current_usage: Option<CurrentUsage>,
}

#[derive(Deserialize)]
struct StatusData {
    model: Option<Model>,
    workspace: Option<Workspace>,
    cwd: Option<String>,
    context_window: Option<ContextWindow>,
}

fn get_git_branch(dir: &str) -> Option<String> {
    // Try symbolic-ref first (works even without commits)
    let output = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(dir)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();
        if !branch.is_empty() {
            return Some(branch);
        }
    }

    // Fallback to rev-parse (for detached HEAD)
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();
        if !branch.is_empty() {
            return Some(branch);
        }
    }
    None
}

fn format_token_count(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

fn build_status_line(input: &str) -> Result<String, serde_json::Error> {
    let data: StatusData = serde_json::from_str(input)?;

    let model = data
        .model
        .and_then(|m| m.display_name)
        .unwrap_or_else(|| "Unknown".to_string());

    let current_dir_path = data
        .workspace
        .and_then(|w| w.current_dir)
        .or(data.cwd)
        .unwrap_or_else(|| ".".to_string());

    let current_dir = Path::new(&current_dir_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(".")
        .to_string();

    let git_branch = get_git_branch(&current_dir_path)
        .map(|b| format!(" | \u{1F33F} {}", b))
        .unwrap_or_default();

    let context_window = data.context_window.unwrap_or_default();
    let context_size = context_window.context_window_size.unwrap_or(0);
    let current_usage = context_window.current_usage.unwrap_or_default();

    let auto_compact_limit = (context_size as f64 * 0.8) as u64;

    let current_tokens = current_usage.input_tokens.unwrap_or(0)
        + current_usage.cache_creation_input_tokens.unwrap_or(0)
        + current_usage.cache_read_input_tokens.unwrap_or(0);

    let percentage = if auto_compact_limit > 0 {
        std::cmp::min(100, (current_tokens * 100 / auto_compact_limit) as u32)
    } else {
        0
    };

    let token_display = format_token_count(current_tokens);

    let percentage_color = if percentage >= 90 {
        "\x1b[31m" // Red
    } else if percentage >= 70 {
        "\x1b[33m" // Yellow
    } else {
        "\x1b[32m" // Green
    };

    Ok(format!(
        "\u{1F916} {} | \u{1F4C1} {}{} | \u{1FA99} {} | {}{}%\x1b[0m",
        model, current_dir, git_branch, token_display, percentage_color, percentage
    ))
}

fn main() {
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        eprintln!("Error reading stdin: {}", e);
        std::process::exit(1);
    }

    match build_status_line(&input) {
        Ok(status_line) => println!("{}", status_line),
        Err(e) => {
            eprintln!("Error parsing JSON: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_token_count_small() {
        assert_eq!(format_token_count(0), "0");
        assert_eq!(format_token_count(500), "500");
        assert_eq!(format_token_count(999), "999");
    }

    #[test]
    fn test_format_token_count_thousands() {
        assert_eq!(format_token_count(1000), "1.0K");
        assert_eq!(format_token_count(1500), "1.5K");
        assert_eq!(format_token_count(10000), "10.0K");
        assert_eq!(format_token_count(999999), "1000.0K");
    }

    #[test]
    fn test_format_token_count_millions() {
        assert_eq!(format_token_count(1000000), "1.0M");
        assert_eq!(format_token_count(1500000), "1.5M");
        assert_eq!(format_token_count(10000000), "10.0M");
    }

    #[test]
    fn test_build_status_line_basic() {
        let input = r#"{
            "model": {"display_name": "Claude Opus"},
            "cwd": "/tmp",
            "context_window": {
                "context_window_size": 200000,
                "current_usage": {
                    "input_tokens": 50000,
                    "cache_creation_input_tokens": 10000,
                    "cache_read_input_tokens": 5000
                }
            }
        }"#;

        let result = build_status_line(input).unwrap();
        assert!(result.contains("🤖 Claude Opus"));
        assert!(result.contains("📁 tmp"));
        assert!(result.contains("🪙 65.0K"));
        assert!(result.contains("40%"));
    }

    #[test]
    fn test_build_status_line_unknown_model() {
        let input = r#"{
            "cwd": "/tmp",
            "context_window": {
                "context_window_size": 100000,
                "current_usage": {"input_tokens": 1000}
            }
        }"#;

        let result = build_status_line(input).unwrap();
        assert!(result.contains("🤖 Unknown"));
    }

    #[test]
    fn test_build_status_line_workspace_over_cwd() {
        let input = r#"{
            "model": {"display_name": "Sonnet"},
            "workspace": {"current_dir": "/home/user/project"},
            "cwd": "/tmp",
            "context_window": {
                "context_window_size": 100000,
                "current_usage": {"input_tokens": 1000}
            }
        }"#;

        let result = build_status_line(input).unwrap();
        assert!(result.contains("📁 project"));
    }

    #[test]
    fn test_build_status_line_percentage_colors() {
        // Green (< 70%)
        let input_green = r#"{
            "cwd": "/tmp",
            "context_window": {
                "context_window_size": 100000,
                "current_usage": {"input_tokens": 10000}
            }
        }"#;
        let result = build_status_line(input_green).unwrap();
        assert!(result.contains("\x1b[32m")); // Green

        // Yellow (70-89%)
        let input_yellow = r#"{
            "cwd": "/tmp",
            "context_window": {
                "context_window_size": 100000,
                "current_usage": {"input_tokens": 60000}
            }
        }"#;
        let result = build_status_line(input_yellow).unwrap();
        assert!(result.contains("\x1b[33m")); // Yellow

        // Red (>= 90%)
        let input_red = r#"{
            "cwd": "/tmp",
            "context_window": {
                "context_window_size": 100000,
                "current_usage": {"input_tokens": 75000}
            }
        }"#;
        let result = build_status_line(input_red).unwrap();
        assert!(result.contains("\x1b[31m")); // Red
    }

    #[test]
    fn test_build_status_line_invalid_json() {
        let input = "not valid json";
        assert!(build_status_line(input).is_err());
    }

    #[test]
    fn test_build_status_line_empty_context() {
        let input = r#"{
            "model": {"display_name": "Test"},
            "cwd": "/tmp"
        }"#;

        let result = build_status_line(input).unwrap();
        assert!(result.contains("🪙 0"));
        assert!(result.contains("0%"));
    }

    #[test]
    fn test_get_git_branch_non_git_dir() {
        let result = get_git_branch("/tmp");
        assert!(result.is_none());
    }
}
