# straitjacket

<p align="center">
  <img src="assets/strait-waistcoat.jpg" alt="Engraving of a patient restrained in a strait-waistcoat" width="320">
  <br>
  <em><sub>Insane patient in a strait-waistcoat. Wellcome Collection (L0011301), <a href="https://creativecommons.org/licenses/by/4.0">CC BY 4.0</a>, via <a href="https://commons.wikimedia.org/wiki/File:Insane_patient_in_a_strait-waistcoat._Wellcome_L0011301.jpg">Wikimedia Commons</a>.</sub></em>
</p>

Straitjacket is a fast, deterministic scanner that flags the weird code and text LLMs like to produce. It sweeps your files against a set of rules — with snobby yet configurable defaults — and flags anything it finds. It's a single static Rust binary with no runtime dependencies, so it drops into almost any environment or repo's CI, regardless of language or stack.

## What it catches

The built-in rules are intentionally **generic** — no framework or single-language assumptions — so the same binary works across all your repos:

| rule | flags |
|------|-------|
| `emoji` | emoji glyphs in code, comments, strings, and Markdown (a reliable LLM tell). Color emoji, VS16-presented glyphs, and flag sequences — but **not** text symbols like `©` `™` `✓`, arrows, dashes, or the geometric star. |
| `color` | hardcoded color literals — hex (`#1e1e1e`) and CSS color functions (`rgb()`, `rgba()`, `hsl()`, `hwb()`, `lab()`, `lch()`, `oklab()`, `oklch()`, `color()`). Use a theme token / CSS variable instead. |
| `inline-svg` | hand-rolled inline `<svg>` in component code — extract it into a named, reusable icon. |
| `inline-font` | inline `font-family` stacks — define the font once and reference a CSS variable. |
| `motion` | ad-hoc `transition` / `animation` / `@keyframes` — centralize motion so it can be tuned or disabled. |
| `file-size` | files longer than the line budget (default **1500**) — sprawling single files are a common LLM tell. Tune with `--max-lines`, disable with `--max-lines 0` or `--skip file-size`. |
| `slop-prose` | LLM prose tells in `.md`/`.markdown`/`.mdx`/`.html`: machine artifacts hard-fail, and a high *density* of style tells warns or fails. Disable with `--skip slop-prose`. See [below](#detecting-ai-written-prose-slop-prose). |
| `duplication` | copy/pasted code across the tree — any clone of ≥ 50 tokens fails; **a structure may appear only once**. Compiled in (no external tool). Tune with `--dup-min-tokens`, disable with `--skip duplication`. See [below](#duplication). |
| `one-component` | more than one React component in a `.tsx`/`.jsx` file. AST-based (OXC). Disable with `--skip one-component`. |
| `effect-in-component` | `useEffect` in a `.tsx`/`.jsx` file that declares a component — effects belong in named custom hooks in their own files. Pure hook files may use it freely. AST-based (OXC). Disable with `--skip effect-in-component`. |
| `prop-drilling` | a **pure conduit**: a component forwards one of its own props unchanged into a *local* child component and **never uses it otherwise** — dead-weight drilling. Uses OXC **semantic** analysis: a prop the component also reads (`{value.x}`, a computation, a DOM binding) is fine; only forwarded-but-unused is flagged. Also excluded — modified props, library components (a Mantine `<Button>` must receive props), **function-typed** slots (callbacks, by type), and `.map` params. Lift the value into a store or context. Disable with `--skip prop-drilling`. |
| `store-passthrough` | a value from a `use*Store()` hook forwarded unchanged into a child component — the child should read the store directly. Same semantic engine; only bare `{value}` on a component counts. Disable with `--skip store-passthrough`. |

**Prop-drilling *depth*.** `prop-drilling` flags a single forwarding hop but can't tell a harmless 1-level pass from a deep drill. `straitjacket --prop-chains` stitches the per-file forwarding edges into a cross-file graph (resolving child components by name) and prints each drill **chain** longest-first with its depth — how many components a prop is handed through:

```
depth 2: ArchiveRow.task  →  TaskLinks.task  →  ReviewLink.task
   from src/web/panes/archive/ArchiveRow.tsx:50
```

Depth ≥ 2 is where "hand-me-down through the tree" starts; most flagged forwards are depth 1 (a leaf) and fine.

The last four rules are **React-specific** and only ever fire on `.tsx`/`.jsx`, so they're inert in non-React repos. Everything is on by default — straitjacket runs at its max, and you ratchet down with `--skip`. Each rule only looks at the file types where it makes sense (e.g. `color` ignores `.json`, `inline-svg` only scans component sources).

Run `straitjacket --list-rules` to see them with descriptions.

## Install

```sh
# prebuilt binary (Linux x86_64, macOS arm64/x86_64):
curl -fsSL https://raw.githubusercontent.com/zmaril/straitjacket/main/install.sh | sh
```

Installs to `/usr/local/bin` if writable, else `~/.local/bin`. Override with `STRAITJACKET_INSTALL_DIR` or pin a release with `STRAITJACKET_VERSION`.

On **Windows**, grab `straitjacket-x86_64-pc-windows-msvc.zip` from [Releases](https://github.com/zmaril/straitjacket/releases). Or, on any platform, from source:

```sh
cargo install --git https://github.com/zmaril/straitjacket
```

## Usage

```sh
straitjacket                      # scan the current directory (honors .gitignore)
straitjacket src tests            # scan specific paths
straitjacket --format json        # machine-readable output
straitjacket --only emoji,color
straitjacket --skip motion,slop-prose  # ratchet rules off
straitjacket --max-lines 800       # tighter file-size budget (0 disables the rule)
straitjacket --prose-window 600    # widen the slop-prose density window
straitjacket --dup-min-tokens 80   # only flag larger duplicated blocks
straitjacket --include-json        # also scan .json (skipped by default)
straitjacket --prop-chains         # report cross-file prop-drilling depth and exit
straitjacket --no-ignore          # don't respect .gitignore / hidden-file rules
straitjacket --no-fail            # report but always exit 0
```

`.json` is skipped by default — it's almost always generated or config data, not code or prose meant to be read by humans. Pass `--include-json` to scan it too.

Output is `path:line:col  [rule]  matched` (warnings are tagged `(warn)`). The process exits **1** when there's any **error**-level finding (so CI fails), **0** when clean or only warnings.

## Detecting AI-written prose (`slop-prose`)

`slop-prose` is a separate analyzer for the linguistic tells of LLM-written text, scoped to prose (`.md`/`.markdown`/`.mdx`/`.html`) — never code. It's **on by default** (straitjacket runs at its max); disable it with `--skip slop-prose`. Because, unlike an emoji in source, "reads like an LLM" is a *probabilistic* claim, its density gate **warns** before it **fails** — the design reflects that.

It works two ways:

- **Machine artifacts → hard fail.** Copy-paste residue that a human essentially never types: `oaicite` / `contentReference`, `turn0search0`, `utm_source=chatgpt.com`, `As an AI language model`, unfilled placeholders (`PASTE_URL_HERE`, `[Your Name]`), `2025-XX-XX` dates. A single hit is an **error**, regardless of document length.
- **Style density → warn, then fail.** Everything else (AI-vocabulary words like *delve/tapestry/pivotal*, stock phrases like *"stands as a testament"*, negative parallelisms like *"not just X, it's Y"*, spaced em dashes, curly quotes) carries a **weight**. No single one means much — the signal is *co-occurrence*. straitjacket slides a fixed **`--prose-window`** (default 400 chars) across the text and scores the densest span. Elevated density → **warning**; high density → **error**.

Dividing by a fixed window is deliberate: it keeps short text lenient, so a lone "not X, but Y" in a commit-style line can't spike the ratio. The score and the contributing phrases are shown, e.g.:

```
CHANGELOG.md:14:4  [slop-prose]  density 0.100 (score 40/400)
   AI-prose density 0.100 over a 400-char window — reads like LLM slop: "stands as a testament", "rich cultural heritage", "showcasing", "vibrant", …
```

**English only, for now.** The wordlist, stock phrases, and templates are all English — `slop-prose` doesn't know what LLM slop sounds like in any other language. If you want a specific language and are willing to help verify what actually reads as sloppy in it, [file an issue](https://github.com/zmaril/straitjacket/issues) requesting that language. (The machine-artifact hard-fails above are language-agnostic and work regardless.)

**Caveats, by design.** LLMs are trained on human text, so these distributions overlap — humans write "not X, but Y" and use em dashes too. Treat `slop-prose` as a *nudge for review*, not proof; the thresholds are v1 calibration guesses (`FAIL_DENSITY`/`WARN_DENSITY` in `src/slop_prose.rs`). Grounding, methodology, and the full tell taxonomy — including what *isn't* statically detectable — are in [`notes/detectability-tiers.md`](notes/detectability-tiers.md), derived from Wikipedia's *Signs of AI writing*.

Exempt a doc that legitimately quotes these things (like those notes) with `straitjacket-allow-file:slop-prose`, or a single line with `straitjacket-allow`.

## Suppressing a false positive

There are two scopes of escape hatch. Both just look for the marker text — the comment syntax (`//`, `#`, `/* */`, `<!-- -->`) doesn't matter.

**One line** — add a same-line comment:

```ts
const brandColor = "#ff6600"; // straitjacket-allow: fixed brand color, not themeable
```

- `straitjacket-allow` suppresses **every** rule on that line.
- `straitjacket-allow:<rule>` suppresses only that rule, e.g. `straitjacket-allow:color`.

**A whole file** — put the marker on any one line of it (top of file is conventional). This is the right tool for a theme/palette file full of legitimate hexes:

```css
/* straitjacket-allow-file:color  design tokens — colors live here */
:root { --bg: #1e1e1e; --fg: #abb2bf; }
```

- `straitjacket-allow-file` exempts **every** rule for the file.
- `straitjacket-allow-file:<rule>` exempts only that rule for the file — so the palette above still gets checked for emoji, oversized length, etc.

### Ignoring big files

`file-size` is a whole-file rule, so use the file-scoped marker (a per-line `straitjacket-allow` won't silence it):

- **Exempt one file:** `straitjacket-allow-file:file-size` on any line of it:
  ```ts
  // straitjacket-allow-file:file-size  generated, intentionally large
  ```
- **Stop scanning generated files entirely:** add them to `.gitignore` or `.ignore` — straitjacket honors both. `.ignore` is handy for files you commit but don't want any tooling to lint, and it exempts the file from *all* rules, which is usually what you want for generated output.
- **Globally:** `--max-lines N` to raise the budget, or `--max-lines 0` / `--skip file-size` to turn it off.

## CI

Use the bundled GitHub Action in any repo:

```yaml
# .github/workflows/straitjacket.yml
name: straitjacket
on: [push, pull_request]
jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: zmaril/straitjacket@v0.1.0
```

The action downloads the prebuilt binary and runs it over the repo — every rule, including [duplication](#duplication), in one self-contained pass. It fails the job on any error-level finding.

Configure it with typed fields (each maps to a CLI flag) rather than a raw argument string:

```yaml
      - uses: zmaril/straitjacket@v0.1.0
        with:
          paths: "src tests"     # default "."
          skip: "motion,slop-prose"
          only: ""               # run only these rules
          max-lines: "800"       # file-size budget (0 disables)
          prose-window: "600"    # slop-prose density window
          dup-min-tokens: "80"   # duplication clone size
          format: "text"         # or "json"
          no-ignore: "false"
          no-fail: "false"
          working-directory: "."
          version: "latest"      # or a pinned tag
```

Every field is optional; blanks fall back to straitjacket's own defaults.


## Duplication

Copy-paste is one of the loudest LLM tells: models clone-and-tweak instead of factoring out a shared helper. The `duplication` rule is **compiled into straitjacket** — it uses [`cpd-finder`](https://crates.io/crates/cpd-finder), the Rust engine behind [jscpd](https://github.com/kucherenko/jscpd) 5, as a library. Nothing to install, no Node, no separate binary; straitjacket walks and tokenizes the tree (Rust, TS/JS, Python, and more) in-process.

The policy matches straitjacket's max-by-default stance: **a structure may appear only once.** Any clone of ≥ 50 tokens is an error:

```
src/util.rs:42:1  [duplication]  9 lines, 71 tokens
   duplicated code — this block also appears at src/helpers.rs:88. LLMs clone-and-tweak; factor out a shared helper.
```

Raise the floor with `--dup-min-tokens N`, or turn it off with `--skip duplication`.

## Background & philosophy

Straitjacket started life as the per-repo `lint-*` Bun scripts in [powdermonkey](https://github.com/zmaril/powdermonkey) (PR #41), written because I got annoyed with the way Claude kept messing with the design of the interface, as well as with the kinds of code and text it would output. I'd written versions of these linters across various projects over the last few years, and I kept finding new smells as I generated more code and text over time. Eventually I decided to bundle them all into one tool, so I wouldn't have to keep rewriting them haphazardly all over the place — and so other people could use it and tell me what other annoying things LLMs tend to do.

During the initial development of Straitjacket, I had a strong realization: what bothers me most about the way LLMs change the design of an application maps neatly onto common UI settings. Claude randomly inserts elements and changes their colors — that's the province of a theme switcher. Claude decides it needs ten font families and a hundred sizes and weights — that's the purview of a font family and size picker. Every element on a page wiggles in its own individual way; well, well, well, that's a motion-control toggle. So, in a way, in lieu of guidance — of an enforced design system — why shouldn't Claude get freaky with it? We never said it couldn't.

So, alongside restricting the design tokens above to blessed files, I'd recommend giving users a way to control these settings too. To me, the two go hand in hand. Likewise, when reviewing code, I found it was very easy for Claude to squirrel thousands of lines away into a single file. I'd review all the lines, they'd look fine, but these monsters would sneak up on me before I knew it. Refactoring them always made the codebase better, and I've found that 1500 lines is about where they start breaking down logically enough for me to notice.

As for slop text, it just smells. There's no way around it, and I don't like it. Straitjacket does its best to scan for the most common signs. It's not wrong to use the word *delve*, but it does get suspicious when you use it often, alongside other signs. Not trying to get too fancy with it.

Straitjacket has become an exercise in me encoding as much of my personal tastes as I can into deterministic checkers I can run across LLM output, hopefully saving me the trouble of having to go "Yuck!" myself.

## Found a new smell?

LLMs invent new tells constantly, and everyone's "Yuck!" is a little different. If you've spotted a pattern straitjacket should catch — or a false positive it shouldn't! — [**file an issue**](https://github.com/zmaril/straitjacket/issues). Concrete examples help most. Two things especially wanted:

- **New rules** — a deterministic smell that generalizes across repos.
- **`slop-prose` in another language** — if you read it and can verify what actually sounds sloppy, say so in the issue (see [above](#detecting-ai-written-prose-slop-prose)).

## License

Code is MIT.

The banner image (`assets/strait-waistcoat.jpg`) — *Insane patient in a strait-waistcoat*, [Wellcome Collection](https://wellcomecollection.org/works/ckwscya3) (L0011301) — is licensed [CC BY 4.0](https://creativecommons.org/licenses/by/4.0) and is **not** covered by the MIT license; reuse it under its own terms.
