# notes

Research backing straitjacket's direction.

- **`detectability-tiers.md`** — the catalogue from Wikipedia's *Signs of AI writing*, regrouped by **how confidently each sign can be detected statically** (regex / wordlist / structure, no ML model). This is the lens that matters for straitjacket: what can a deterministic scanner flag with a low false-positive rate?
- **`source-signs-of-ai-writing.txt`** — verbatim plain-text snapshot of [Wikipedia:Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing), fetched 2026-06-30, kept for provenance. Wikipedia text is CC BY-SA 4.0.

The source is Wikipedia-specific (wikitext, AfC, WP: shortcuts). The tiers doc pulls out what generalizes to code repos, docs, comments, and commit messages.
