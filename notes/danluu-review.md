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

### 1. `focused-test` — `.only` / `fdescribe` / `fit`
`describe.only`, `it.only`, `test.only`, `fdescribe`, `fit` **silently skip the
rest of the suite** — a green CI that's lying. Classic accidental commit.
- **Dan Luu**: `wat` (normalized deviance / silent failure).
- **Check**: `\b(describe|it|test|context)\.only\s*\(` · `\bf(describe|it)\s*\(`
- **Biome overlap**: `noFocusedTests` covers **JS/TS**. Straitjacket's value is
  everything else — pytest `@pytest.mark.only`-style focus, RSpec `fit`/`focus:`,
  Go build-tag focus — and running one check across all of them.
- **FP**: very low. **Severity**: error. **Effort**: trivial (pattern rule).

### 2. `disabled-test` — skipped / ignored tests
`.skip`, `xit`/`xdescribe`, `@pytest.mark.skip`/`xfail`, `@Ignore`, `t.Skip(`,
`#[ignore]`. A disabled test is a disabled reliability check; LLMs "fix" a red test
by skipping it.
- **Dan Luu**: `wat`, `broken-builds`.
- **Check**: `\.skip\s*\(|\b(xit|xdescribe)\b|pytest\.mark\.(skip|xfail)|@Ignore\b|#\[ignore\]`
- **Biome overlap**: `noSkippedTests` covers **JS/TS**. Same cross-language pitch
  as #1.
- **FP**: low. **Severity**: warn (some skips are legitimate and annotated).

### 3. `deep-nesting` — nesting depth over budget  ★
The one the reviewer flagged as best. Whole-file rule, like `file-size`. Flag any
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

### 4. slop-prose: "vague boasting" vocabulary
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

Ship the cleanest, lowest-FP wins first:

1. **slop-prose boasting vocab** (#4) — wordlist edit, no new mechanism.
2. **`focused-test`** (#1) — trivial, very low FP, high value.
3. **`deep-nesting`** (#3) — the standout; whitespace-based, `--max-nesting`, warn.
4. housekeeping **B1** (`continue-on-error`) + **B2** (`CODEOWNERS`) +
   **B5** (`reproducible-toolchain`) — the trustworthy-builds spine.

Then **`disabled-test`** (#2), **B3** (scheduled CI), **B6** (job timeout),
**B7** (retry-masking), and **B4** (README predicate). Housekeeping supports
per-check exceptions, so lean toward including a check and letting repos opt out
rather than omitting it. Every item traces to a specific Dan Luu argument and
clears the Biome filter.
