use serde::Serialize;

/// Whether a finding fails the run (`Error`) or is informational (`Warning`). All the
/// deterministic code rules are `Error`; `slop-prose` uses `Warning` for a density
/// that's elevated but below the hard-fail line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// One flagged occurrence. Positions are 1-based; `col` is a byte offset into the
/// line (ripgrep convention), so it's stable regardless of multi-byte glyphs.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Finding {
    /// Rule id that produced this, e.g. `"emoji"`.
    pub rule: String,
    /// Display path of the file (relative to the scan root when possible).
    pub path: String,
    pub line: usize,
    pub col: usize,
    /// The exact text that tripped the rule.
    pub matched: String,
    /// Human-facing explanation of why this is flagged.
    pub message: String,
    /// Whether this fails the run or is only a warning.
    pub severity: Severity,
}

/// 1-based line and byte-column for a byte offset into `text`.
pub fn line_col(text: &str, off: usize) -> (usize, usize) {
    let before = &text[..off.min(text.len())];
    let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
    let col = off - before.rfind('\n').map(|i| i + 1).unwrap_or(0) + 1;
    (line, col)
}
