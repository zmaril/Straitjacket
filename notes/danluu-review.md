# Dan Luu review — candidate rules for straitjacket & housekeeping

Research pass (2026-07-04): mined [Dan Luu's blog](https://danluu.com) for ideas
that could become deterministic checks in **straitjacket** (code/prose smells) or
**housekeeping** (repo health). Cheap agents read the reliability, testing/process,
writing, and complexity clusters; this doc is the filtered result.

## The filter: assume Biome + a formatter already run

Straitjacket is not a general linter and shouldn't grow into one. The working
assumption here is that a repo already runs **Biome** (or ESLint) and a formatter.
That kills every candidate Biome already covers, and it changes the pitch for the
rest: straitjacket earns its slot by being **the one binary you run across every
repo regardless of stack** (per the README) and by covering things Biome doesn't —
**prose**, **taste/design tokens**, and **cross-language** structural smells.

Everything below is held to straitjacket's real bar: deterministic, generic, and a
**low false-positive rate** so red stays trustworthy.

## The second filter: "does a language-specific linter already own this?"

Biome-overlap is a special case of a broader test that turned out to be the sharp
one. A correctness lint that a mature per-language tool — or the test runner
itself — already does **semantically** is not straitjacket's job; a cross-language
regex there is *strictly worse* (blind to AST/semantics, FP-prone) and earns
nothing. straitjacket should stay out of that space and lean into what **no**
language linter owns: cross-cutting **taste** — design tokens, prose, and
complexity **dials**. That's the tool's actual identity (the README's
dials-for-the-undesigned framing), and `file-size` is the proof of species.

This filter kills the two test-hygiene rules below and leaves `deep-nesting` as the
only surviving code rule — because it's a taste dial, not a correctness lint.

## The Dan Luu throughline

Three essays map cleanly onto these tools:

- **`corp-eng-blogs`** — "vague boasting," "generic high-level fluff," specificity
  over abstraction. Same aesthetic slop-prose already encodes; just more vocabulary.
- **`broken-builds` / `everything-is-broken` / `wat`** — builds must fail loudly;
  normalized deviance; silently-masked failures. Grounds a CI + test cluster.
- **`essential-complexity`** — long/deeply-nested functions are "nearly entirely
  accidental complexity." The natural sibling of the existing `file-size` rule.

---

## straitjacket — ship

### `deep-nesting` — nesting depth over budget  ★ (the one surviving code rule)
Whole-file rule, like `file-size`. Flag any
line whose nesting depth exceeds a budget (default ~6). Deeply-nested code forces a
reader to hold too many contexts at once.
- **Dan Luu**: `essential-complexity` (accidental complexity), `simple-architectures`.
- **Check**: two ways, and the second is the good one —
  - brace/bracket-stack depth (accurate, needs a tiny tokenizer for strings/comments);
  - **leading-whitespace depth** — *reliable precisely because a formatter is
    enforced*, so indentation is canonical. Language-agnostic, no tokenizer, one
    pass. This is the intended default given the "fmt is on" assumption.
- **Biome overlap**: **none** — Biome has no max-nesting-depth rule (ESLint's
  `max-depth` exists but is JS-only). Genuine gap, and language-agnostic.
- **FP**: medium (parsers / state machines / table-driven code legitimately nest)
  → ship as a **tunable warn**, `--max-nesting`, disable with `0`, mirroring
  `file-size`.

### slop-prose: "vague boasting" vocabulary
Add a weighted tell-class from `corp-eng-blogs`: *industry-leading, cutting-edge,
best-in-class, game-changing, revolutionary, seamless, robust, powerful,
state-of-the-art, blazing-fast, effortless, unlock, supercharge*. The exact
marketing fluff Dan Luu contrasts against good technical writing.
- **Biome overlap**: none — Biome doesn't touch prose.
- **FP**: low, *because* it rides the existing density model — a single "robust"
  can't trip it; co-occurrence in a 400-char window can. **Best effort-to-value
  item here**: it's a wordlist + weights, no new mechanism.

---

## straitjacket — dropped (with reason)

- **`focused-test`** (`.only`/`fdescribe`/`FIt`) — *closed, [#43]*. Fails the
  language-linter filter: every ecosystem owns this natively and semantically —
  Biome `noFocusedTests` / eslint-plugin-jest (JS/TS), `ginkgo --fail-on-focused`
  (Go, at the runner), rubocop-rspec `RSpec/Focus` (Ruby). A cross-language regex
  is strictly worse (blind to semantics, `fit(` vs `model.fit(` ambiguity).
- **`disabled-test`** (`.skip`/`xit`/`#[ignore]`) — *closed, [#44]*. Same filter:
  Biome `noSkippedTests`, rubocop-rspec, etc. own it; and it was the more
  subjective of the two.
- **`empty-catch`** — Biome `noEmptyBlockStatements` already flags empty
  `catch {}`. Doing it in straitjacket would be redundant and regex-based error
  detection is FP-prone anyway.
- **`barrel-export`** (`export * from`) — Biome `noBarrelFile` + `noReExportAll`
  cover JS/TS; the non-JS variants (`from x import *`, `pub use x::*`) are too
  marginal to justify a rule.
- **`long-function`** — too subjective (per reviewer); where's the line? Also can't
  be done generically without a real parser for non-brace languages. `deep-nesting`
  captures most of the same intent more objectively.
- **broad-catch / unchecked-fsync / ignored-return / error-check-without-throw** —
  all need semantics or type info, all medium-to-high FP. Semgrep's job, not
  straitjacket's; they'd erode "trust the red."
- **TODO-density, commented-out-code** — legitimate-use-heavy; at most opt-in
  warns, not default material.

---

## housekeeping — ship

### B1. `ci-continue-on-error` — no silently-masked failures
In GitHub Actions, **`continue-on-error: true`** on a step or job means the
step/job may fail while the workflow **still reports success** — a green check.
Put it on a test or lint step and CI goes green even as tests fail: the build lies.
That's exactly the failure mode `broken-builds` is about — a build must fail
loudly when it's broken.
- **Check**: grep `.github/workflows/*.yml` for `continue-on-error:\s*true` inside
  a step/job whose name or `run:` involves `test`/`lint`/`build`; allow it if an
  adjacent comment explains the exception.
- **Dan Luu**: `broken-builds`, `postmortem-lessons` (silent failures enable
  disasters). **FP**: low-med. **Applies**: public & private.

### B2. `codeowners` — CODEOWNERS exists and is non-empty
Routes review to people who know the sensitive areas; human process matters as much
as the tooling.
- **Check**: `.github/CODEOWNERS` (or root/`docs/`) exists with ≥1 non-comment rule.
- **Dan Luu**: `broken-builds`, `postmortem-lessons`. **FP**: low. **Applies**:
  both; **warn** on solo/small repos.

### B3. `ci-scheduled-run` — CI runs on a schedule, not just push/PR
Push-only CI never exercises the repo between commits, so bitrot, expiring
credentials, and dependency drift go unseen until someone happens to push. A
`schedule:` trigger catches them.
- **Check**: at least one workflow with a test/build job also has a `schedule:`
  trigger.
- **Dan Luu**: `broken-builds`, `why-benchmark` (measure continuously). **FP**:
  low. **Applies**: both; higher value on public/library repos.

### B4. README quality additions
From `corp-eng-blogs` ("specificity over abstraction"), extend the existing README
pass with deterministic warns:
- opening paragraph carries a **predicate** ("X **is a** … / **does** … /
  **provides** …"), not just "Welcome to X";
- at least one **runnable** fenced block (code/CLI invocation), not only prose;
- no placeholder headings (`TODO` / `TBD` / `[…]`).
- **FP**: low-med → warns.

### B5. `reproducible-toolchain` — no floating `latest`/`*` in CI  ★
`node-version: latest`, `python-version: '*'`, unpinned `setup-*`, or a missing
`.nvmrc` / `rust-toolchain.toml` / `.python-version` means the build isn't
reproducible: it bitrots silently and "works on my machine" drifts from CI. Same
"builds must be trustworthy" spine as B1, and the best of these additions.
- **Check**: grep workflow YAML for `-version:\s*(latest|\*)` and unpinned
  `actions/setup-*`; check the tree for a pinned toolchain file.
- **Dan Luu**: `everything-is-broken` (reproducibility). **FP**: low. **Applies**:
  both.

### B6. `ci-job-timeout` — `timeout-minutes` set on CI jobs
No timeout means a hung job (network wedge, deadlocked test) burns the full runner
ceiling — hours of a lying "in progress." Bound the feedback loop.
- **Check**: each job with a test/build step has `timeout-minutes:`.
- **Dan Luu**: `broken-builds` + his tail-latency thinking. **FP**: low.
  **Applies**: both.

### B7. `test-retry-masking` — the mirror of B1
`pytest-rerunfailures`, `jest --retries`, `nextest` retry configs auto-rerun tests
until one run passes, then report green — the exact `wat` "@flaky library reports a
pass if any run passes" anti-pattern. It launders flakiness into a false green.
- **Check**: grep CI + test config for rerun/retry plugins and flags.
- **Dan Luu**: `wat` (normalized deviance). **FP**: med (retries have legit uses)
  → **warn**; except where intended. **Applies**: both.

---

## Recommendation

After the language-linter filter, the code-rule side collapses to one item, and
housekeeping carries the rest. Ship in this order:

1. **`deep-nesting`** ([#45]) — the one surviving code rule; whitespace-based,
   `--max-nesting`, warn. Same species as `file-size`.
2. **slop-prose boasting vocab** ([#46]) — wordlist edit, no new mechanism.
3. housekeeping **B1** (`continue-on-error`, [#29]) + **B2** (`CODEOWNERS`, [#30])
   + **B5** (`reproducible-toolchain`, [#33]) — the trustworthy-builds spine.

Then **B3** (scheduled CI, [#31]), **B6** (job timeout, [#34]), **B7**
(retry-masking, [#35]), and **B4** (README predicate, [#32]). Housekeeping supports
per-check exceptions, so lean toward including a check and letting repos opt out
rather than omitting it.

**Closed after review:** `focused-test` ([#43]) and `disabled-test` ([#44]) — both
owned better by per-language linters/runners (see the filter above).
