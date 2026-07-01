//! `duplication` — copy/paste detection compiled directly into straitjacket via the
//! `cpd-finder` library (the engine behind jscpd 5, which is itself Rust). No
//! external binary to install and no Node: straitjacket walks and tokenizes the
//! tree itself and reports every clone.
//!
//! The policy matches straitjacket's max-by-default stance: **a structure may appear
//! only once.** Any clone of at least `min_tokens` tokens is an `Error`. This is a
//! cross-file, whole-run analysis, so it runs once over the scan paths rather than
//! per file.

use std::path::PathBuf;

use cpd_finder::orchestrate::{run, RunConfig};

use crate::finding::{Finding, Severity};

const RULE: &str = "duplication";

/// Detect duplicated code across `paths` and return one finding per clone. `ignore`
/// holds extra glob patterns to exclude (e.g. `**/*.json`). Detection failures
/// degrade to an empty result rather than aborting the whole scan.
pub fn detect(
    paths: &[PathBuf],
    respect_ignore: bool,
    min_tokens: usize,
    ignore: &[String],
) -> Vec<Finding> {
    let config = RunConfig {
        paths: paths.to_vec(),
        min_tokens,
        no_gitignore: !respect_ignore,
        ignore: ignore.to_vec(),
        ..RunConfig::default()
    };
    let Ok(result) = run(&config) else {
        return Vec::new();
    };

    result
        .clones
        .into_iter()
        .map(|clone| {
            let a = clone.fragment_a;
            let b = clone.fragment_b;
            let lines = a.end.line.saturating_sub(a.start.line) + 1;
            Finding {
                rule: RULE.to_string(),
                path: tidy(&a.source_id),
                line: a.start.line as usize,
                col: a.start.column as usize,
                matched: format!("{lines} lines, {} tokens", clone.token_count),
                message: format!(
                    "duplicated code — this block also appears at {}:{}. LLMs clone-and-tweak; factor out a shared helper.",
                    tidy(&b.source_id),
                    b.start.line
                ),
                severity: Severity::Error,
            }
        })
        .collect()
}

/// Drop a leading `./` from a source path for display.
fn tidy(path: &str) -> String {
    path.strip_prefix("./").unwrap_or(path).to_string()
}
