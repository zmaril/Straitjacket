# Proposal: project boundaries for monorepos

Status: proposal / open for discussion. Written 2026-07-05.

## The problem, precisely

Most of straitjacket's rules are **per-file**: they read one file's text and
decide. `color`, `emoji`, `slop-prose`, `file-size`, `deep-nesting`,
`one-component`, `effect-in-component` never look at a second file, so a monorepo
is no different to them than a single package. They need nothing here.

Three rules are **cross-file** — they compare one file against the rest of the
tree — and those are the ones that misbehave in a monorepo:

- **`duplication`** runs the cpd-finder pass **once over every scan path**
  (`main.rs:195`). It reports a clone between *any* two files it sees, including
  two files in unrelated packages.
- **`prop-drilling`** and **`store-passthrough`** consult a single
  `ComponentIndex` built over **every** React file in the scan (`main.rs:175`),
  keyed by the **bare component name** (`react.rs:63`). A `Button` in
  `packages/web` and a `Button` in `packages/admin` collide in that map, and
  prop-forwarding chains (`--prop-chains`) get stitched together across packages
  that never import each other.

The `duplication` case is the one that bites first and hardest. In a monorepo the
whole point of a package boundary is that two packages *don't* share a helper —
they're deployed and versioned apart. Straitjacket currently flags the
boilerplate they legitimately hold in common (config scaffolding, a generated
API client copied into two apps, two similar-but-independent components) as an
error, and tells you to "factor out a shared helper" across a boundary you
deliberately drew. That advice is often wrong for a monorepo, and the noise is
unactionable.

### What you can do about it today

Nothing structural. The only lever is to spray `straitjacket-allow:duplication`
markers on one side of every cross-package clone, or `allow-file:duplication` on
whole files. That doesn't scale: the markers multiply with every package pair,
they suppress *intended* in-package clones on the same file too, and a new
package silently reintroduces the noise. The suppression markers are a per-line
escape hatch; they were never meant to encode a repo's package topology.

## The idea

Let a repo **declare its project boundaries once**, and have every cross-file
analysis partition on them: a file is only ever compared against files in the
**same project subtree**. Comparisons never cross a boundary.

This turns "declare the topology once" into the replacement for "spray markers
forever," which is the right shape for a structural fact about the repo.

Concretely, a *project* is a directory. Its subtree — down to the next nested
project root, if any — is one comparison unit. Everything above every project
root is the *root project*. A repo that declares no projects has exactly one
project (the whole scan), so behaviour is identical to today. That keeps the
change fully backward compatible.

## Recommended design

### MVP — `projects:` globs in the root config

Add one key to `.straitjacket.yaml` (`FileConfig` in `config.rs`):

```yaml
# .straitjacket.yaml at the repo root
projects:
  - packages/*
  - services/*
```

Each directory matching a glob is a **project root**. The cross-file passes
partition the scanned file list by each file's **nearest ancestor project root**
(files under no project root land in the root project), then:

- **`duplication`** runs the cpd-finder pass **once per project**, over that
  project's subtree only. The root-project pass excludes the project subtrees
  (cpd-finder's `RunConfig.ignore` already takes globs — pass `packages/*/**`
  etc.), so a repo-level script isn't compared against package internals and
  cross-package clones are simply never generated.
- **`prop-drilling` / `store-passthrough`** build **one `ComponentIndex` per
  project** and scan each project's React files against its own index, so a name
  collision across packages can't happen and chains can't span packages.
- **`--prop-chains`** stitches edges within each project and reports per project.

Cost is roughly unchanged: cpd hashing is ~linear in tokens, so N smaller passes
total about the same as one big pass, and they do strictly fewer cross-file
comparisons.

Why this first: zero new file types, the topology is visible in one place, and it
maps directly onto how workspaces are already declared elsewhere
(`pnpm-workspace.yaml` globs, `package.json` `workspaces`, Cargo `[workspace]
members`). Someone reading the root config sees the whole partition at a glance.

### Phase 2 — auto-detection and per-project config

- **Auto-detect** boundaries from the workspace manifest when the user opts in
  with `projects: auto` — read `pnpm-workspace.yaml` / `package.json`
  `workspaces` / Cargo `[workspace] members`. Most monorepos then need zero
  straitjacket-specific configuration.
- **Per-project config**: generalize config discovery (today `find_config` walks
  up to the *nearest single* `.straitjacket.yaml`, `config.rs:45`) into a
  **nearest-wins layering** — a `.straitjacket.yaml` inside a project dir
  overrides the root config for that subtree, so one package can bump
  `max-lines` or `--skip` a rule locally without touching its siblings. This is
  the larger change (config becomes a stack, not a single file); worth splitting
  out.

### Phase 3 — deliberate cross-project checks

Occasionally you *do* want a cross-package clone flagged: two packages
copy-pasted the same util that should have been a shared package. Add an opt-in,
e.g. a per-project `shared: true` designation (a package everyone may legitimately
duplicate *from*) or a `duplication: cross-project` mode. Explicitly out of the
MVP — the default should be "boundaries are walls."

## What does *not* change

- Every per-file rule. No partitioning, no new behaviour.
- The suppression markers (`straitjacket-allow[-file]`). They keep working
  per-file and per-line exactly as documented. Project boundaries are the
  *structural* tool; markers stay the *local* escape hatch. The win is that you
  stop needing markers to paper over package topology — the boundary does it.
- SARIF / JSON / text output shapes. A finding still points at a file and line;
  it's just that cross-boundary findings are no longer generated.
- The single-package case. No `projects:` key ⇒ one project ⇒ today's behaviour,
  byte for byte.

## Alternatives considered

- **A sentinel file per project dir** (e.g. an empty `.straitjacket-project`).
  Con: scatters the topology across the tree, so no one place shows the whole
  partition; also invents a second config artifact. A central glob list reads
  better and matches how workspaces are already declared. (Auto-detection in
  Phase 2 removes even the glob list for most repos.)
- **A project *marker* line** (`straitjacket-project` text in some file,
  mirroring the allow markers). Con: a project boundary is a directory-level
  fact, not a line-level one; encoding it as in-file text is a category error and
  makes "which dir is the root?" ambiguous. Markers are right for per-line
  suppression and wrong for directory topology.
- **Reuse "a dir with a `.straitjacket.yaml` is a boundary."** Con: conflates two
  independent wishes — "override some settings here" and "draw an analysis wall
  here." You often want one without the other. Keep them separable: `projects:`
  draws walls; a nested config (Phase 2) overrides settings.

## Rough implementation sketch (MVP)

1. `config.rs`: add `projects: Option<Vec<String>>` to `FileConfig`; carry it
   into `Resolved` / a new field on the scan.
2. A small `projects` module: given the config globs and the collected file list,
   return `project_root(path) -> PathBuf` (nearest matching ancestor, or the
   root-project sentinel).
3. `main.rs` per-file loop: unchanged — per-file rules don't care.
4. React index: replace the single `engine.set_component_index(...)` with one
   index per project bucket; scan each bucket's files against its own index. The
   engine already isolates the index behind `set_component_index`, so this is
   "build a map of `project -> ComponentIndex`" and pick per file (or run the
   React pass bucket-by-bucket).
5. Duplication: replace the single `duplication::detect(&resolved.paths, …)` call
   with one call per project root over that root's path, and one root-project call
   that `ignore`s the project globs. Concatenate findings; the existing
   `is_suppressed` marker filter still applies to each.
6. Docs: a `guides/monorepos.mdx` page and a `projects:` row in the config
   reference.

Test coverage worth adding: a fixture monorepo with an identical helper in two
packages asserts **zero** `duplication` findings by default, and a same-named
component in two packages asserts no cross-package prop-drilling — then flip
`projects:` off and assert both fire, to pin the boundary behaviour.
