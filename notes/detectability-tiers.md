<!-- straitjacket-allow-file:slop-prose  this file catalogues the very artifacts it would otherwise flag -->

# Signs of AI writing, grouped by how confidently we can detect them statically

Source: [Wikipedia:Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) (CC BY-SA 4.0), snapshot in `source-signs-of-ai-writing.txt`.

The lens here is **straitjacket's**: a deterministic scanner (regex / wordlist / structural counts, **no ML model**) running over text in a repo — docs, comments, commit messages, PR descriptions. So each sign is rated by two things:

- **Detectability** — can a static rule catch it at all?
- **Confidence** — if the rule fires, how likely is it actually a tell vs. a false positive?

The single most important meta-point from the source, and the thing that should shape any straitjacket "prose" module:

> Almost no single sign is proof. Humans write "not X, but Y" and use em dashes. LLMs are trained on human text, so the distributions overlap. **The signal is co-occurrence and density**, not any one hit. The source repeatedly says humans (and AI detectors) are bad at this from style alone. → Score and surface for review; do **not** hard-fail CI on a lone match.

Tiers run from "machine fingerprint, basically certain" down to "needs a model or a human."

---

## Tier 0 — Deterministic artifacts (near-certain, exact match, ~0 false positives)

These are leftover machine output: copy-paste residue, internal citation tokens, placeholder text. Unambiguous. **This is true secret-scanner territory and the ideal first prose target for straitjacket** — exact strings/regex, no scoring needed, fires rarely but damningly.

| Sign | Static pattern |
|------|----------------|
| ChatGPT citation residue | `contentReference`, `oaicite`, `oai_citation`, `:contentReference[oaicite:N]{index=N}` |
| Search/image link tokens (PUA-wrapped) | `turn0search0`, `turn0image1`, `turn0news0`, `turn0file0` → `/turn\d+(search|image|news|file)\d+/` |
| Other provider citation cards | `grok_card`, `grok_render_citation_card_json`, `【NN†L…】` (DeepSeek), `[cite: 1]` (Gemini), `[attached_file:1]`, `[web:1]` (Perplexity) |
| Attribution JSON | `({"attribution":{"attributableIndex":"X-Y"}})` |
| Newer doc markup | `:::writing{variant="document" id="NNNNN"}` |
| Tracking params on cited URLs | `utm_source=chatgpt.com`, `utm_source=openai`, `utm_source=copilot.com`, `referrer=grok.com` |
| Knowledge-cutoff / refusal boilerplate | "As an AI language model", "As of my last knowledge update", "I don't have specific information about" |
| Unfilled placeholders | `INSERT_SOURCE_URL`, `PASTE_..._HERE`, `[Your Name]`, `[Describe the specific section ...]`, `[link to the revised article]` |
| Placeholder dates | `2025-XX-XX`, `|access-date=...XX...` → `/20\d\d-(XX|xx)-(XX|xx)/` |
| "Want me to convert this?" meta-offers | "Would you like me to … turn this into … `wikitext`?" |

**Confidence: very high.** A human almost never types `oaicite` or `utm_source=chatgpt.com` by hand. Caveat (from source): the URL params prove a ChatGPT *tool* touched it, not necessarily that prose was AI-written.

---

## Tier 1 — Structural / formatting signals (high detectability, medium-high confidence, count-based)

Deterministic to detect; confidence depends on density and context. Mostly regex + a counter.

| Sign | Static pattern | FP risk |
|------|----------------|---------|
| **Emoji as decoration** (in headings/bullets/log lines) | codepoint scan — **straitjacket already does this** | low in code |
| **Curly quotes / apostrophes** | `[‘’“”]` | medium — smart-quote editors, macOS, Chicago style, typeset sources |
| **Spaced em dashes** | ` — ` (em dash with surrounding spaces), density per 1k words | medium — the *spacing* is the tell; bare `—` alone is weak and "notorious" enough that GPT-5.1 suppresses it |
| **Inline-header vertical lists** | `^\s*[-*•–#]?\s*\*\*[^*]+:?\*\*:?\s` — bolded lead-in per bullet | medium — readmes/slides do this too |
| **Overuse of boldface** | `**…**` count per paragraph over threshold | medium |
| **Markdown where it doesn't belong** | `##` headings, ```` ``` ```` fences, `[text](url)` in a non-Markdown context | context-dependent; in a Markdown repo this is just normal |
| **Title Case Headings** | heading where most words are Capitalized | higher — many style guides use title case |
| **Thematic break before each heading** | `---`/`***` immediately preceding a heading | low-medium |
| **Skipped heading levels** | `###` with no preceding `##` | low (but Wikipedia-flavored) |
| **Tiny needless tables** | a 2-row table that could be prose | hard to judge structurally |

**For straitjacket:** emoji (done), curly quotes, spaced-em-dash density, and bolded-bullet lead-ins are all clean regex/counter rules. Best as a **score** ("4 formatting tells in this file"), not individual hard fails.

---

## Tier 2 — Lexical signals (high detectability, confidence ONLY via density + co-occurrence)

Pure wordlist / phrase matching (aho-corasick). One hit is noise; *many, together, in post-2022 text* is, per the source, "one of the strongest tells." Must be density-scored.

- **"AI vocabulary" wordlist** — the source gives era-bucketed lists:
  - 2023–mid-2024 (GPT-4): *delve, tapestry, testament, boasts, bolstered, crucial, intricate/intricacies, interplay, landscape, meticulous, pivotal, underscore, valuable, vibrant, garner, enduring*
  - mid-2024–mid-2025 (GPT-4o): *align with, enhance, fostering, highlighting, showcasing, underscore, vibrant, pivotal*
  - mid-2025+ (GPT-5): *emphasizing, enhance, highlighting, showcasing*
  - Grok-idiosyncratic: *causal, empirical, correlate, underscore*
- **Stock phrases** — "maintains an active social media presence", "rich cultural heritage", "stands as a testament", "plays a vital/pivotal role", "nestled in", "leaving a lasting impact", "rich tapestry of".
- **Comment-context words** — "concrete evidence", "concrete examples" (used when *denying* AI use).
- **Copula-avoidance verbs** — "serves as", "stands as", "features", "boasts", "offers" replacing is/has (overlaps Tier 3).

**Confidence: low per hit, high in aggregate.** The source is explicit: take it "as literally as possible" (a word being overused doesn't implicate synonyms), respect context (literal "underscore" the character), and weight by era. Implementation = matched terms + a normalized density score, surfaced when it crosses a threshold.

---

## Tier 3 — Syntactic templates (medium detectability, real false-positive risk)

Templates with variable slots. Regex can approximate them within a sentence; this is where **"not X, but Y"** lives. Genuinely used by humans, so confidence is moderate even on a match — lean on co-occurrence with other tiers.

- **Negative parallelism** (the family you asked about):
  - *Not only X but (also) Y* — `\bnot only\b .* \bbut\b`
  - *Not just X, it's Y* / *isn't just X — it's Y* — `\b(not|isn't|isn’t) just\b`
  - *Not X, but Y* / *It's not X, it's Y* — `\bit'?s not\b .* ,? \bit'?s\b`
  - *no X, no Y, just Z*
  - *X rather than Y* (Grok-leaning) — reversed form
- **Rule of three / tricolon** — "A, B, and C" of adjectives or short phrases. Structurally findable but lists are everywhere → only meaningful as density.
- **"Challenges / Future prospects" outline conclusion** — `Despite its .*, .* faces challenges` then a vaguely upbeat close.
- **Superficial-analysis participle tails** — sentence-final `, (highlighting|reflecting|underscoring|emphasizing|showcasing) .*` ("…, highlighting its significance").
- **Canned comment templates** (near Tier-0 for the comment domain — fixed enough to match high-confidence): "I understand the importance of adhering to Wikipedia's…", "I am open to any suggestions or feedback", "Could an uninvolved editor advise…", "Let's keep the discussion focused on the content".

**Confidence: medium.** The "not X but Y" regexes are easy; the hard part is the gap between X and Y crossing clause/sentence boundaries (multi-sentence forms need a sentence splitter — still static, just a tokenizer). The canned-comment phrases are the high-confidence subset.

---

## Tier 4 — Statistical / corpus signals (detectable, but need a baseline, not a single-file regex)

Computable without a model, but they require **comparison** — a baseline corpus or an author's own history — so they don't fit a stateless line scanner.

- **Lexical diversity / elegant variation** — repetition-penalty artifact; needs type-token / synonym-spread stats.
- **Avoidance of is/are** — only meaningful as a *ratio drop* vs. a human baseline (the source cites a >10% decline studies controlled for lead sentences).
- **Sentence-length uniformity / low burstiness** — distributional.
- **Pronounced shift in writing style** — cross-document; compare a contributor's new text to their pre-Nov-2022 text. (For a repo: compare a PR author's diff to their git history.)

**Confidence: medium, but only with infrastructure.** Out of scope for a simple scanner; plausible for a stats pass or a git-history-aware mode.

---

## Tier 5 — Semantic / judgment (NOT statically detectable — needs a model, world knowledge, or network)

What the text *means*. No regex reaches these; they require understanding or external verification.

- **Undue emphasis on significance / legacy / broader trends** — "a revolutionary titan of industry" for a niche subject.
- **Superficial analysis, promotional/travel-guide tone** — semantic register.
- **Vague attributions / overgeneralized opinions** — weasel wording ("researchers note", "widely interpreted").
- **Knowledge-cutoff *speculation*** — fabricated "not widely documented … likely …" claims (vs. the fixed *boilerplate* form, which is Tier 0).
- **Hallucinated / wrong citations** — partly mechanizable but not from text alone:
  - ISBN/DOI **checksum validity** → actually pure-static (Tier 1-ish), cheap to add.
  - **DOI resolves to an unrelated paper**, broken external links, page-less book cites, cited page doesn't support the claim → need **network + semantic** checks.
- **Hallucinated categories / templates / policies / shortcuts** — need an authority list to diff against (Wikipedia-specific).

**Confidence: this is the LLM-detector / human-reviewer job**, explicitly the part the source warns is unreliable from style alone.

---

## Two negative lists (just as important)

### Do NOT flag on these alone — "Ineffective indicators" (source §)
The source's explicit false-positive traps. A prose module should **avoid** rules keyed only on:
- Perfect grammar.
- Formal / "academic" / "fancy" prose (the correlation is with *specific* words, not formality in general).
- "Bland" or "robotic" feel.
- Transition words in isolation (*Additionally, Consequently, Notably*) — weak; precedented in human essay writing.
- Letter-like formatting alone (salutations/sign-offs).
- Mixed casual+formal registers.
- Unsourced content; correct *or* bizarre wikitext on its own.

### Signs of HUMAN writing — usable as confidence *reducers*
Counter-evidence that should lower a score:
- Text predating **2022-11-30** (ChatGPT launch) → AI ruled out. For a repo: commit/authored date.
- Simple `is`/`has` phrases, hedging qualifiers (*very, perhaps, tends to*), wordy human constructions (*in order to, as a result of, the fact that*), blunt superlatives (*the only, the first*).
- An author who can explain their own edit/choice.

---

## What this means for straitjacket

Directly actionable now, in order of confidence, scoped to **prose surfaces** (`.md`, comments, commit messages, PR text) — never code:

1. **Tier 0 artifacts** → a high-confidence, exact-match rule group. Fires rarely, near-zero FP. Could even default-on. (`oaicite`, `utm_source=chatgpt.com`, "As an AI language model", placeholder text, …)
2. **Tier 1 formatting** → emoji (done) + curly quotes + spaced-em-dash density + bolded-bullet lead-ins, as a **per-file score**.
3. **Tier 2 lexical** → era-bucketed AI-vocabulary wordlist via aho-corasick, **density-scored**, weighted by file/commit date.
4. **Tier 3 templates** → "not X, but Y" family + canned-comment phrases. Report-only.

Everything Tier 4–5 and the Wikipedia-specific machinery (wikitext, AfC, WP: shortcuts, categories, DOI-resolves-elsewhere) is out of scope for a static scanner.

**Design rule, straight from the source:** emit a *score with the contributing hits*, threshold it, default to report-only / warning. A single "not X but Y" or one em dash means nothing — three tells from three different tiers in one post-2022 paragraph is the actual signal.
