//! Project boundaries for monorepos.
//!
//! A directory containing a `.straitjacket.toml` marker is a *project root*. The
//! cross-file analyses — `duplication` and the React forwarding rules
//! (`prop-drilling`, `store-passthrough`) — partition on these roots and never
//! compare files across a boundary, so two independent packages don't get flagged
//! for the boilerplate they legitimately share. A repo with no markers is a single
//! project (the whole scan), so nothing changes for a non-monorepo.

use std::path::{Path, PathBuf};

use anyhow::Context;
use ignore::WalkBuilder;
use serde::Deserialize;

/// Marker filename: its presence in a directory declares that directory a project root.
pub const PROJECT_MARKER: &str = ".straitjacket.toml";

/// Contents of a `.straitjacket.toml`. The file's *presence* is what draws the
/// boundary; every field is optional and reserved for future per-project settings.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProjectFile {
    /// Optional human-facing name for the project.
    pub name: Option<String>,
}

/// Drop a leading `./` so roots discovered by the walker compare equal to the
/// `./`-stripped paths the duplication pass and `display_path` produce.
fn norm(p: &Path) -> &Path {
    p.strip_prefix("./").unwrap_or(p)
}

/// The project roots discovered under the scan paths — used to map any file to the
/// project it belongs to.
#[derive(Debug, Default, Clone)]
pub struct Projects {
    /// Project root directories, longest path first so the *nearest* enclosing root
    /// wins for nested projects.
    roots: Vec<PathBuf>,
}

impl Projects {
    /// Walk `roots` for `.straitjacket.toml` markers, recording each one's directory as
    /// a project root. Hidden-file filtering is off (the marker is a dotfile) but ignore
    /// files are still honoured when `respect_ignore` is set, so a gitignored vendored
    /// tree doesn't declare phantom projects. A malformed marker is a hard error — a
    /// typo'd boundary is a mistake worth surfacing.
    pub fn discover(roots: &[PathBuf], respect_ignore: bool) -> anyhow::Result<Self> {
        let mut iter = roots.iter();
        let Some(first) = iter.next() else {
            return Ok(Self::default());
        };
        let mut builder = WalkBuilder::new(first);
        for r in iter {
            builder.add(r);
        }
        builder
            .hidden(false)
            .git_ignore(respect_ignore)
            .git_global(respect_ignore)
            .git_exclude(respect_ignore)
            .ignore(respect_ignore)
            .parents(respect_ignore);

        let mut dirs = Vec::new();
        for result in builder.build() {
            let Ok(entry) = result else { continue };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            if entry.file_name() != PROJECT_MARKER {
                continue;
            }
            let path = entry.path();
            // Validate the TOML so a typo surfaces instead of silently mis-declaring.
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            toml::from_str::<ProjectFile>(&text)
                .with_context(|| format!("parsing {}", path.display()))?;
            let dir = path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            dirs.push(dir);
        }
        // Longest first: the nearest enclosing project root wins for nested projects.
        dirs.sort_by_key(|d| std::cmp::Reverse(d.as_os_str().len()));
        Ok(Self { roots: dirs })
    }

    /// The project a file belongs to: the nearest ancestor project root, or `None` for
    /// the *root project* (files under no marker). Returned normalized (no `./`).
    pub fn root_for(&self, path: &Path) -> Option<PathBuf> {
        let path = norm(path);
        self.roots
            .iter()
            .map(|r| norm(r))
            .find(|root| path.starts_with(root))
            .map(Path::to_path_buf)
    }

    /// Whether two files live in the same project — the condition for a cross-file rule
    /// to compare them.
    pub fn same(&self, a: &Path, b: &Path) -> bool {
        self.root_for(a) == self.root_for(b)
    }

    /// Whether any project boundary exists (lets callers skip partitioning when there's
    /// nothing to partition).
    pub fn is_partitioned(&self) -> bool {
        !self.roots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projects(roots: &[&str]) -> Projects {
        let mut roots: Vec<PathBuf> = roots.iter().map(PathBuf::from).collect();
        roots.sort_by_key(|r| std::cmp::Reverse(r.as_os_str().len()));
        Projects { roots }
    }

    #[test]
    fn no_boundaries_is_one_project() {
        let p = Projects::default();
        assert!(!p.is_partitioned());
        // Everything is the root project (None), so any two files "match".
        assert!(p.same(Path::new("a/x.ts"), Path::new("b/y.ts")));
        assert_eq!(p.root_for(Path::new("a/x.ts")), None);
    }

    #[test]
    fn files_map_to_their_enclosing_root() {
        let p = projects(&["packages/web", "packages/admin"]);
        assert_eq!(
            p.root_for(Path::new("packages/web/src/a.ts")),
            Some(PathBuf::from("packages/web"))
        );
        assert_eq!(
            p.root_for(Path::new("packages/admin/b.ts")),
            Some(PathBuf::from("packages/admin"))
        );
        // A file above every project root belongs to the root project.
        assert_eq!(p.root_for(Path::new("scripts/tool.ts")), None);
    }

    #[test]
    fn cross_project_files_do_not_match() {
        let p = projects(&["packages/web", "packages/admin"]);
        assert!(!p.same(
            Path::new("packages/web/a.ts"),
            Path::new("packages/admin/b.ts")
        ));
        assert!(p.same(
            Path::new("packages/web/a.ts"),
            Path::new("packages/web/deep/b.ts")
        ));
    }

    #[test]
    fn nearest_root_wins_for_nested_projects() {
        let p = projects(&["packages/web", "packages/web/plugins/chart"]);
        assert_eq!(
            p.root_for(Path::new("packages/web/plugins/chart/a.ts")),
            Some(PathBuf::from("packages/web/plugins/chart"))
        );
        assert_eq!(
            p.root_for(Path::new("packages/web/src/a.ts")),
            Some(PathBuf::from("packages/web"))
        );
        // A file in the inner project isn't in the same project as one in the outer.
        assert!(!p.same(
            Path::new("packages/web/plugins/chart/a.ts"),
            Path::new("packages/web/src/b.ts")
        ));
    }

    #[test]
    fn leading_dot_slash_is_normalized() {
        let p = projects(&["./packages/web"]);
        assert_eq!(
            p.root_for(Path::new("packages/web/a.ts")),
            Some(PathBuf::from("packages/web"))
        );
        assert_eq!(
            p.root_for(Path::new("./packages/web/a.ts")),
            Some(PathBuf::from("packages/web"))
        );
    }
}
