# .githooks

Committed git hooks that run this repo's full CI gate locally, before a commit
lands — so failures surface here instead of after a push.

## Activate

Hooks are not enabled automatically on clone. Run once per checkout:

```sh
git config core.hooksPath .githooks
```

## What runs

`pre-commit` mirrors CI:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo run --release -- .` — straitjacket scanning its own tree (dogfood)

`commit-msg` enforces Conventional Commits on the subject line, matching the
`conventional` PR-title check.

## Bypass

`git commit --no-verify` skips both hooks for a single commit.
