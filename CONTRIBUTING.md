# Contributing to Lunaria Annua

## Branch-and-PR policy

**Every change to `main` must go through a pull request — including changes made
by maintainers.** Direct pushes to `main` are not allowed.

1. Create a topic branch from `main`:
   `feat/...`, `fix/...`, `docs/...`, `chore/...`, or `refactor/...`.
2. Commit on that branch. Keep each commit focused and its message in
   `type: summary` form (for example `feat: add HandleKey encoding`).
3. Push the branch and open a pull request against `main`.
4. Make sure formatting, Clippy, and tests pass (see below). At least one
   approving review is required; a maintainer may self-merge after checks pass.
5. Merge with squash or merge commits. Delete the topic branch afterwards.

Emergency fixes follow the same flow. If `main` must be restored, use a revert
PR rather than a direct push or a history rewrite.

## Development setup

Rust **stable** (edition 2024, MSRV 1.85) is pinned by `rust-toolchain.toml`;
`rustup` installs the required components on first use.

```sh
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

## Coding guidelines

- Follow `rustfmt.toml`; CI rejects unformatted code.
- Clippy warnings are treated as errors.
- Keep `lunaria-handles` dependency-free.
- `unsafe` is allowed where the architecture requires it, but every `unsafe`
  block must carry a `// SAFETY:` comment explaining its invariants.
- Follow the architecture and memory rules in
  [`SPECIFICATION.md`](./SPECIFICATION.md). In particular, never cache raw
  register pointers across VM calls (use `base_offset`), and respect the
  Frame Arena POD isolation rule.
- Add tests for new behavior; bug fixes should include a regression test.

## License

By contributing, you agree that your contributions are licensed under the
GNU General Public License v3.0 or later (`GPL-3.0-or-later`), as described in
[`LICENSE`](./LICENSE).
