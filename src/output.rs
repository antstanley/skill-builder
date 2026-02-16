//! Centralized output abstraction supporting human-friendly and agent-consumable modes.

use console::Style;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

const SPINNER_TEMPLATE: &str = "  {spinner:.green} {msg}";
const PROGRESS_TEMPLATE: &str = "  {msg} [{bar:30.green/dim}] {pos}/{len}";

/// Output handler that adapts between rich human output and structured agent output.
pub struct Output {
    agent_mode: bool,
    verbose: bool,
    term: console::Term,
    no_color: bool,
}

impl Output {
    /// Create a new Output instance.
    ///
    /// Agent mode activates if `agent_mode_flag` is true OR `SB_AGENT_OUTPUT=1` env var is set.
    /// Verbose mode activates if `verbose_flag` is true OR `SB_VERBOSE=1` env var is set.
    /// Colors are disabled if `NO_COLOR` env var is set or stdout is not a TTY.
    #[must_use]
    pub fn new(agent_mode_flag: bool, verbose_flag: bool) -> Self {
        let term = console::Term::stderr();
        let agent_mode =
            agent_mode_flag || std::env::var("SB_AGENT_OUTPUT").unwrap_or_default() == "1";
        let verbose =
            verbose_flag || std::env::var("SB_VERBOSE").unwrap_or_default() == "1";
        let no_color = std::env::var("NO_COLOR").is_ok() || !console::colors_enabled();

        Self {
            agent_mode,
            verbose,
            term,
            no_color,
        }
    }

    /// Whether agent output mode is active.
    #[must_use]
    pub const fn is_agent_mode(&self) -> bool {
        self.agent_mode
    }

    /// Whether verbose output mode is active.
    #[must_use]
    pub const fn is_verbose(&self) -> bool {
        self.verbose
    }

    /// Print a verbose-only debug message (no-op unless verbose mode is active).
    pub fn verbose(&self, msg: &str) {
        if !self.verbose {
            return;
        }
        if self.agent_mode {
            let _ = self.term.write_line(&format!("[DEBUG] {msg}"));
        } else if self.no_color {
            let _ = self.term.write_line(&format!("  [verbose] {msg}"));
        } else {
            let style = Style::new().dim();
            let _ = self.term.write_line(&format!("  {}", style.apply_to(msg)));
        }
    }

    /// Print a success status line.
    pub fn status(&self, prefix: &str, msg: &str) {
        if self.agent_mode {
            let _ = self.term.write_line(&format!("[OK] {prefix}: {msg}"));
        } else if self.no_color {
            let _ = self.term.write_line(&format!("{prefix}: {msg}"));
        } else {
            let check = Style::new().green().apply_to("\u{2713}");
            let bold_prefix = Style::new().bold().apply_to(prefix);
            let _ = self
                .term
                .write_line(&format!("{check} {bold_prefix} {msg}"));
        }
    }

    /// Print an informational message.
    pub fn info(&self, msg: &str) {
        if self.agent_mode {
            let _ = self.term.write_line(&format!("[INFO] {msg}"));
        } else {
            let _ = self.term.write_line(msg);
        }
    }

    /// Print an indented step message.
    pub fn step(&self, msg: &str) {
        if self.agent_mode {
            let _ = self.term.write_line(&format!("[STEP] {msg}"));
        } else {
            let _ = self.term.write_line(&format!("  {msg}"));
        }
    }

    /// Print a warning message.
    pub fn warn(&self, msg: &str) {
        if self.agent_mode {
            let _ = self.term.write_line(&format!("[WARN] {msg}"));
        } else if self.no_color {
            let _ = self.term.write_line(&format!("Warning: {msg}"));
        } else {
            let warn = Style::new().yellow().apply_to("\u{26a0}");
            let _ = self.term.write_line(&format!("{warn} {msg}"));
        }
    }

    /// Print an error message to stderr.
    pub fn error(&self, msg: &str) {
        if self.agent_mode {
            let _ = self.term.write_line(&format!("[ERROR] {msg}"));
        } else if self.no_color {
            let _ = self.term.write_line(&format!("Error: {msg}"));
        } else {
            let cross = Style::new().red().apply_to("\u{2717}");
            let style = Style::new().red().bold();
            let _ = self
                .term
                .write_line(&format!("{} {}", cross, style.apply_to(msg)));
        }
    }

    /// Print a bold header.
    pub fn header(&self, msg: &str) {
        if self.agent_mode {
            let _ = self.term.write_line(&format!("[INFO] {msg}"));
        } else if self.no_color {
            let _ = self.term.write_line(msg);
        } else {
            let style = Style::new().bold();
            let _ = self.term.write_line(&format!("{}", style.apply_to(msg)));
        }
    }

    /// Print a blank line (no-op in agent mode).
    pub fn newline(&self) {
        if !self.agent_mode {
            let _ = self.term.write_line("");
        }
    }

    /// Create a spinner with a message.
    ///
    /// In agent mode, just prints the message and returns a hidden progress bar.
    ///
    /// # Panics
    ///
    /// Panics if the hardcoded spinner template is invalid (should never happen).
    #[must_use]
    pub fn spinner(&self, msg: &str) -> ProgressBar {
        if self.agent_mode || self.no_color || !self.term.is_term() {
            let _ = self.term.write_line(&if self.agent_mode {
                format!("[STEP] {msg}")
            } else {
                format!("  {msg}...")
            });
            ProgressBar::hidden()
        } else {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::with_template(SPINNER_TEMPLATE)
                    .unwrap()
                    .tick_chars("\u{25d0}\u{25d3}\u{25d1}\u{25d2}\u{2713}"),
            );
            pb.set_message(msg.to_string());
            pb.enable_steady_tick(Duration::from_millis(100));
            pb
        }
    }

    /// Create a progress bar with a length and message.
    ///
    /// In agent mode, just prints the message and returns a hidden progress bar.
    ///
    /// # Panics
    ///
    /// Panics if the hardcoded progress template is invalid (should never happen).
    #[must_use]
    pub fn progress_bar(&self, len: u64, msg: &str) -> ProgressBar {
        if self.agent_mode || self.no_color || !self.term.is_term() {
            let _ = self.term.write_line(&if self.agent_mode {
                format!("[STEP] {msg} ({len})")
            } else {
                format!("  {msg} ({len} items)...")
            });
            ProgressBar::hidden()
        } else {
            let pb = ProgressBar::new(len);
            pb.set_style(
                ProgressStyle::with_template(PROGRESS_TEMPLATE)
                    .unwrap()
                    .progress_chars("\u{2588}\u{2591} "),
            );
            pb.set_message(msg.to_string());
            pb
        }
    }

    /// Print a table with aligned columns.
    pub fn table(&self, rows: &[Vec<String>]) {
        if rows.is_empty() {
            return;
        }

        // Calculate column widths
        let num_cols = rows.iter().map(std::vec::Vec::len).max().unwrap_or(0);
        let mut widths = vec![0usize; num_cols];
        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                if i < num_cols {
                    widths[i] = widths[i].max(cell.len());
                }
            }
        }

        for row in rows {
            let mut parts = Vec::new();
            for (i, cell) in row.iter().enumerate() {
                if i < num_cols.saturating_sub(1) {
                    parts.push(format!("{:<width$}", cell, width = widths[i]));
                } else {
                    parts.push(cell.clone());
                }
            }
            let line = format!("  {}", parts.join("  "));
            if self.agent_mode {
                let _ = self.term.write_line(&format!("[INFO] {}", line.trim()));
            } else {
                let _ = self.term.write_line(&line);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_mode_from_flag() {
        let output = Output::new(true, false);
        assert!(output.is_agent_mode());
    }

    #[test]
    fn test_human_mode_default() {
        let output = Output::new(false, false);
        assert!(!output.is_agent_mode());
    }

    #[test]
    fn test_verbose_mode_from_flag() {
        let output = Output::new(false, true);
        assert!(output.is_verbose());
    }

    #[test]
    fn test_spinner_returns_hidden_in_agent_mode() {
        let output = Output::new(true, false);
        let pb = output.spinner("Loading...");
        // Hidden progress bar has length 0
        assert_eq!(pb.length(), None);
    }

    #[test]
    fn test_progress_bar_returns_hidden_in_agent_mode() {
        let output = Output::new(true, false);
        let pb = output.progress_bar(10, "Downloading");
        // In agent mode we get a hidden bar
        assert_eq!(pb.length(), None);
    }
}
