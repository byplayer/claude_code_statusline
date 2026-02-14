use chrono::Local;
use serde::Deserialize;
use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const MAX_LOG_SIZE: u64 = 1_048_576; // 1MB
const MAX_LOG_FILES: u32 = 5;

fn rotate_log(log_path: &str) {
    let size = match std::fs::metadata(log_path) {
        Ok(m) => m.len(),
        Err(_) => return,
    };

    if size < MAX_LOG_SIZE {
        return;
    }

    // Delete the oldest log file
    let oldest = format!("{}.{}", log_path, MAX_LOG_FILES);
    let _ = std::fs::remove_file(&oldest);

    // Rename .log.{n-1} -> .log.{n} (from highest to lowest)
    for n in (2..=MAX_LOG_FILES).rev() {
        let from = format!("{}.{}", log_path, n - 1);
        let to = format!("{}.{}", log_path, n);
        let _ = std::fs::rename(&from, &to);
    }

    // Rename .log -> .log.1
    let _ = std::fs::rename(log_path, format!("{}.1", log_path));
}

fn debug_log(message: &str) {
    if std::env::var("STATUSLINE_DEBUG").is_err() {
        return;
    }

    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return,
    };

    let log_path = format!("{}/.claude/status_line_debug.log", home);

    rotate_log(&log_path);

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let pid = std::process::id();

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = writeln!(file, "[{} pid:{}] {}", timestamp, pid, message);
    }
}

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

#[derive(Deserialize, Default)]
struct StatusData {
    model: Option<Model>,
    workspace: Option<Workspace>,
    cwd: Option<String>,
    context_window: Option<ContextWindow>,
}

fn get_git_branch(dir: &str) -> Option<String> {
    debug_log(&format!("get_git_branch: dir={}", dir));

    // Try symbolic-ref first (works even without commits)
    debug_log("git symbolic-ref start");
    let output = Command::new("git")
        .args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(dir)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    debug_log("git symbolic-ref done");

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();
        if !branch.is_empty() {
            return Some(branch);
        }
    }

    // Fallback to rev-parse (for detached HEAD)
    debug_log("git rev-parse start");
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    debug_log("git rev-parse done");

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

fn build_status_line(input: &str) -> String {
    let show_model = std::env::var("CC_STATUSLINE_NO_MODEL")
        .map(|v| v != "1")
        .unwrap_or(true);
    build_status_line_impl(input, show_model)
}

fn build_status_line_impl(input: &str, show_model: bool) -> String {
    let data: StatusData = if input.trim().is_empty() {
        StatusData::default()
    } else {
        serde_json::from_str(input).unwrap_or_default()
    };

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

    if show_model {
        format!(
            "\u{1F916} {} | \u{1F4C1} {}{} | \u{1FA99} {} | {}{}%\x1b[0m",
            model, current_dir, git_branch, token_display, percentage_color, percentage
        )
    } else {
        format!(
            "\u{1F4C1} {}{} | \u{1FA99} {} | {}{}%\x1b[0m",
            current_dir, git_branch, token_display, percentage_color, percentage
        )
    }
}

fn read_stdin_with_timeout(timeout: Duration) -> Result<String, String> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut input = String::new();
        let result = io::stdin().read_to_string(&mut input);
        let _ = tx.send(result.map(|_| input));
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(input)) => Ok(input),
        Ok(Err(e)) => Err(format!("Error reading stdin: {}", e)),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err("Error: No input received within 3 seconds".to_string())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("Error: stdin reader unexpectedly disconnected".to_string())
        }
    }
}

fn main() {
    debug_log("=== START ===");

    debug_log("waiting for stdin...");
    let input = match read_stdin_with_timeout(Duration::from_secs(3)) {
        Ok(input) => {
            debug_log(&format!("stdin received: {} bytes", input.len()));
            input
        }
        Err(e) => {
            debug_log(&format!("stdin error: {}", e));
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    debug_log("building status line...");
    let result = build_status_line(&input);
    debug_log("status line built");

    println!("{}", result);
    debug_log("=== END ===");
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

        let result = build_status_line(input);
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

        let result = build_status_line(input);
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

        let result = build_status_line(input);
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
        let result = build_status_line(input_green);
        assert!(result.contains("\x1b[32m")); // Green

        // Yellow (70-89%)
        let input_yellow = r#"{
            "cwd": "/tmp",
            "context_window": {
                "context_window_size": 100000,
                "current_usage": {"input_tokens": 60000}
            }
        }"#;
        let result = build_status_line(input_yellow);
        assert!(result.contains("\x1b[33m")); // Yellow

        // Red (>= 90%)
        let input_red = r#"{
            "cwd": "/tmp",
            "context_window": {
                "context_window_size": 100000,
                "current_usage": {"input_tokens": 75000}
            }
        }"#;
        let result = build_status_line(input_red);
        assert!(result.contains("\x1b[31m")); // Red
    }

    #[test]
    fn test_build_status_line_invalid_json() {
        let input = "not valid json";
        let result = build_status_line(input);
        // Invalid JSON should return default values
        assert!(result.contains("🤖 Unknown"));
        assert!(result.contains("📁 ."));
        assert!(result.contains("🪙 0"));
        assert!(result.contains("0%"));
    }

    #[test]
    fn test_build_status_line_empty_input() {
        let result = build_status_line("");
        assert!(result.contains("🤖 Unknown"));
        assert!(result.contains("📁 ."));
        assert!(result.contains("🪙 0"));
        assert!(result.contains("0%"));
    }

    #[test]
    fn test_build_status_line_whitespace_input() {
        let result = build_status_line("   \n\t  ");
        assert!(result.contains("🤖 Unknown"));
        assert!(result.contains("📁 ."));
    }

    #[test]
    fn test_build_status_line_empty_context() {
        let input = r#"{
            "model": {"display_name": "Test"},
            "cwd": "/tmp"
        }"#;

        let result = build_status_line(input);
        assert!(result.contains("🪙 0"));
        assert!(result.contains("0%"));
    }

    #[test]
    fn test_build_status_line_no_model() {
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

        let result = build_status_line_impl(input, false);
        assert!(!result.contains("🤖"));
        assert!(!result.contains("Claude Opus"));
        assert!(result.starts_with("📁 tmp"));
        assert!(result.contains("🪙 65.0K"));
    }

    #[test]
    fn test_get_git_branch_non_git_dir() {
        let result = get_git_branch("/tmp");
        assert!(result.is_none());
    }
}
