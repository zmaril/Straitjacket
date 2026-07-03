//! The regex-backed rules. Each is a [`RegexRule`]: a compiled [`Regex`] plus a
//! `judge` closure that turns a match into the text to report (or `None` to skip a
//! benign match, e.g. a `font-family` already set to a CSS variable). The raw
//! pattern is also handed to the engine's `RegexSet` prefilter.

use regex::{Captures, Regex};

use super::{LineHit, Rule};

/// Web/style source extensions — where colors, fonts, motion and inline SVG live.
const WEB_EXTS: &[&str] = &[
    "css", "scss", "sass", "less", "ts", "tsx", "js", "jsx", "mjs", "cjs", "vue", "svelte", "html",
    "htm",
];

/// Component-source extensions — where hand-rolled inline `<svg>` shows up.
const COMPONENT_EXTS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs", "vue", "svelte"];

type Judge = fn(&Captures) -> Option<String>;

pub struct RegexRule {
    id: &'static str,
    message: &'static str,
    exts: &'static [&'static str],
    pattern: String,
    re: Regex,
    judge: Judge,
}

impl RegexRule {
    fn new(
        id: &'static str,
        message: &'static str,
        exts: &'static [&'static str],
        pattern: &str,
        judge: Judge,
    ) -> Self {
        let re = Regex::new(pattern).expect("built-in rule pattern must compile");
        Self {
            id,
            message,
            exts,
            pattern: pattern.to_string(),
            re,
            judge,
        }
    }
}

impl Rule for RegexRule {
    fn id(&self) -> &'static str {
        self.id
    }

    fn message(&self) -> &'static str {
        self.message
    }

    fn applies_to_ext(&self, ext: &str) -> bool {
        self.exts.contains(&ext)
    }

    fn scan_line(&self, line: &str, out: &mut Vec<LineHit>) {
        for caps in self.re.captures_iter(line) {
            let whole = caps.get(0).expect("group 0 always present");
            if let Some(text) = (self.judge)(&caps) {
                out.push(LineHit {
                    col: whole.start() + 1,
                    matched: text,
                });
            }
        }
    }

    fn prefilter(&self) -> Option<&str> {
        Some(&self.pattern)
    }
}

/// Report the whole match verbatim.
fn whole(caps: &Captures) -> Option<String> {
    Some(caps[0].to_string())
}

/// A `font-family` value is fine when it's a CSS variable, a global keyword, or a
/// bare token/generic word — flag only an inline literal *stack* (a quoted font or a
/// multi-family list like `Inter, sans-serif`).
fn judge_font(caps: &Captures) -> Option<String> {
    let raw = caps.get(1)?.as_str().trim();
    // In a JS object the comma after the value separates properties, not font
    // fallbacks: if what follows the first comma looks like another `key: value`,
    // keep only the value itself (`{ fontFamily: MONO, fontSize: 12 }`). Otherwise
    // it's a CSS fallback list (or a lone trailing comma), so drop just a trailing one.
    let value = match raw.split_once(',') {
        Some((head, tail)) if tail.contains(':') => head.trim(),
        _ => raw.trim_end_matches(',').trim(),
    };
    let lower = value.to_ascii_lowercase();
    // A CSS variable is the good pattern whether it's bare or quoted — both
    // `fontFamily: var(--x)` and `fontFamily: "var(--x)"` just point at a token.
    // Strip one matching pair of surrounding quotes for the var check only (so a
    // quoted *font* like `"Inter"` still trips `is_bare_word == false` below).
    let unquoted = lower
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| lower.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(lower.as_str());
    let is_var = unquoted.starts_with("var(");
    let is_keyword = matches!(
        lower.as_str(),
        "inherit" | "initial" | "unset" | "revert" | ""
    );
    // A bare single word: a token/variable reference (`fontFamily: MONO` — the *good*
    // pattern) or a generic family (`monospace`, `sans-serif`). Not the smell, which is
    // a quoted font or a hardcoded multi-family stack.
    let is_bare_word = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '-'));
    if is_var || is_keyword || is_bare_word {
        None
    } else {
        Some(value.to_string())
    }
}

/// Report a motion declaration without its trailing colon (`transition`, not
/// `transition:`).
fn judge_motion(caps: &Captures) -> Option<String> {
    Some(caps[0].trim_end_matches([' ', ':']).to_string())
}

pub fn pattern_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(RegexRule::new(
            "color",
            "hardcoded color literal (hex / rgb / hsl / oklch / …) — use a theme token or CSS variable so it stays themeable.",
            WEB_EXTS,
            // Hex (#rgb…#rrggbbaa), a CSS color function, or the `color()` function.
            // Function names are lowercase only, so PascalCase constructors like
            // `Color(...)` aren't mistaken for colors; and `color(` must be followed
            // by a real color-space keyword, so English like "color(s)" is ignored.
            r"#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{4}|[0-9a-fA-F]{3})\b|\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch)\([^)\n]*\)|\bcolor\(\s*(?:from|srgb(?:-linear)?|display-p3|a98-rgb|prophoto-rgb|rec2020|xyz(?:-d50|-d65)?)\b[^)\n]*\)",
            whole,
        )),
        Box::new(RegexRule::new(
            "inline-svg",
            "inline <svg> in component code — extract it into a named, reusable icon component.",
            COMPONENT_EXTS,
            r#"<svg[\s/>]|createElement\(\s*["']svg["']"#,
            whole,
        )),
        Box::new(RegexRule::new(
            "inline-font",
            "inline font-family stack — define the font once and reference a CSS variable.",
            WEB_EXTS,
            r"(?i)(?:font-family|fontFamily)\s*:\s*([^;}\n]+)",
            judge_font,
        )),
        Box::new(RegexRule::new(
            "motion",
            "ad-hoc transition/animation — centralize motion so it can be tuned or disabled.",
            WEB_EXTS,
            r"\b(?:transition|animation)(?:-[a-z-]+)?\s*:|@keyframes\b",
            judge_motion,
        )),
    ]
}
