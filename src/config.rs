//! Scan configuration assembled from CLI flags and handed to the [`Engine`].

/// Default line budget for the `file-size` rule. LLMs tend to produce sprawling
/// single files; 1500 lines is a generous ceiling before that's worth a look.
pub const DEFAULT_MAX_LINES: usize = 1500;

/// Default sliding-window size (in bytes/chars) for the `slop-prose` density check.
pub const DEFAULT_PROSE_WINDOW: usize = 400;

/// Default minimum token run for the `duplication` rule to count as a clone.
pub const DEFAULT_DUP_MIN_TOKENS: usize = 50;

#[derive(Debug, Clone)]
pub struct Config {
    /// Line budget for the `file-size` rule. `None` disables the rule.
    pub max_lines: Option<usize>,
    /// Run the `slop-prose` analyzer. On by default — straitjacket runs at its max
    /// and you ratchet down with `--skip slop-prose`. Tier-0 artifacts hard-fail;
    /// Tier 1–3 density warns/fails.
    pub slop_prose: bool,
    /// Sliding-window size for the `slop-prose` density check.
    pub prose_window: usize,
    /// Run the compiled-in `duplication` (copy/paste) detector. On by default.
    pub duplication: bool,
    /// Minimum token run for `duplication` to count a clone.
    pub dup_min_tokens: usize,
    /// Skip `.json` files. On by default — JSON is usually generated/config data,
    /// not human-written prose or code. Turn off to scan it too.
    pub skip_json: bool,
    /// `one-component`: at most one React component per `.tsx`/`.jsx` file.
    pub one_component: bool,
    /// `effect-in-component`: no `useEffect` in a file that declares a component.
    pub effect_in_component: bool,
    /// `prop-drilling`: a component's prop must not be forwarded unchanged into a
    /// child component (keep every component within one hop of its data).
    pub prop_drilling: bool,
    /// `store-passthrough`: a `use*Store` value must not be forwarded unchanged into
    /// a child component (the child should read the store directly).
    pub store_passthrough: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_lines: Some(DEFAULT_MAX_LINES),
            slop_prose: true,
            prose_window: DEFAULT_PROSE_WINDOW,
            duplication: true,
            dup_min_tokens: DEFAULT_DUP_MIN_TOKENS,
            skip_json: true,
            one_component: true,
            effect_in_component: true,
            prop_drilling: true,
            store_passthrough: true,
        }
    }
}
