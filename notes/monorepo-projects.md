# Monorepo project boundaries

Status: implemented. Design record for the `.straitjacket.toml` project marker.

## The problem, precisely

Most of Straitjacket's rules are **per-file**: they read one file's text and
decide. `color`, `emoji`, `slop-prose`, `file-size`, `deep-nesting`,
`one-component`, `effect-in-component` never look at a second file, so a monorepo
is no different to them than a single package. They need nothing here.

Three rules are **cross-file** — they compare one file against the rest of the
tree — and those are the ones that misbehave in a monorepo:

- **`duplication`** ran the cpd-finder pass once over every scan path, so it
  reported a clone between *any* two files, including two files in unrelated
  packages.
- **`prop-drilling`** and **`store-passthrough`** consulted a single
  `ComponentIndex` built over **every** React file, keyed by the **bare component
  name**. A `Button` in `packages/web` and a `Button` in `packages/admin`
  collided in that map, and prop-forwarding chains got stitched together across
  packages that never import each other.

The `duplication` case bit first and hardest: the whole point of a package
boundary is that two packages *don't* share a helper, yet Straitjacket flagged
the boilerplate they legitimately hold in common and told you to "factor out a
shared helper" across a boundary you deliberately drew.

## What shipped

A directory that contains a **`.straitjacket.toml`** file is a *project*. The
cross-file analyses partition on these markers: a file is only ever compared
against files in the **same project subtree**, never across a boundary.

- A file belongs to its **nearest ancestor** marker directory. Files above every
  marker form the **root project** (the rest of the repo).
- Nested markers work — an inner package carves its own subtree out of the outer.
- **No markers ⇒ one project ⇒ behaviour identical to before.** Fully backward
  compatible; a non-monorepo pays nothing.

The marker is TOML and its *presence* is what draws the boundary. Fields are
optional (`name` is the only one read today) and reserved for future per-project
settings — so the same file that declares the wall can later tune the package.

Implementation lives in `src/project.rs` (`Projects::discover` / `root_for`),
with the cross-file passes partitioned in `src/duplication.rs`
(`detect_partitioned`) and `src/main.rs` (per-project `ComponentIndex`,
per-project `--prop-chains`).

## Two decisions worth recording

**Why a marker file, not a central glob list.** An earlier sketch put a
`projects:` glob list in the root `.straitjacket.yaml`. A per-directory marker
won out: it's the "declare a project where the project is" model, it needs no
glob syntax, and it composes with nested packages for free. The cost — the
topology isn't visible in one place — is minor next to how obvious a marker in a
package root is. (Discovery ignores hidden-file filtering for the marker but
still honours `.gitignore`, so a vendored tree doesn't declare phantom projects.)

**Why per-project cpd passes, not one global pass then filtered.** The tempting
shortcut is to keep the single `duplication` pass and drop any clone whose two
fragments live in different projects. It's wrong: cpd-finder reports only a
*subset* of clone pairs (not every pairwise match), so a genuine in-project clone
can be reported *only* via a cross-project pairing — and filtering that pair out
silently drops the real finding. Running one cpd pass per project over that
project's own files avoids the trap: cross-project pairs never exist, and every
in-project clone is found on its own terms. The regression test
`duplication_still_flags_within_a_project_across_boundaries` pins this.

## Not done (possible follow-ups)

- **Auto-detection** from workspace manifests (`pnpm-workspace.yaml`,
  `package.json` `workspaces`, Cargo `[workspace] members`), so most monorepos
  need no markers at all.
- **Per-project settings**: let a `.straitjacket.toml` override rule config for
  its subtree (bump `max-lines`, skip a rule locally).
- **Deliberate cross-project checks**: an opt-in for the case where two packages
  copy-pasted a util that *should* be shared.
