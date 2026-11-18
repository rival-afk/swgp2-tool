# Changelog

## 2.0.0 - 2026-11-18

**Breaking:** the flag-based CLI (`--slot`, `--save`, `dump`, `set`, …) was
removed. Everything it did is now available with zero arguments:

- `swgp2` — new simple GUI: slot picker, inline editing of common fields,
  advanced set-by-path, raw-JSON dump/apply/preview, backup & restore.
- `swgp2-tui` — new terminal UI with the same functionality.

Internally the crate is split into a library (`crypto`, `nrbf`, `save`, `app`)
shared by both front-ends. Write path unchanged and always safe:
edit → backup → write → re-parse & verify (auto-restore on mismatch).
`cargo test --release` runs a round-trip test on a temp copy of your save.
