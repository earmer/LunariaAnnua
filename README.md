# Lunaria Annua

[![CI](https://github.com/earmer/LunariaAnnua/actions/workflows/ci.yml/badge.svg)](https://github.com/earmer/LunariaAnnua/actions/workflows/ci.yml)

Programming language and bytecode VM for high-refresh graphics, dual-host
(Rust / Go) zero-copy interop, and sub-millisecond on-device hot compilation.
The architecture whitepaper is [`SPECIFICATION.md`](./SPECIFICATION.md).

## Toolchain

Rust **stable** (edition 2024, MSRV 1.85). `rust-toolchain.toml` pins the
required channel and components; `rustup` installs them on first use.

```sh
rustup show          # verify the active toolchain
cargo build          # build the workspace
cargo test           # run all tests
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

## Layout

```
crates/
  lunaria-handles/   SlotMap RID / weak ?T handles + downward FrameArena (Step 1)
```

Further crates (`lunaria-vm`, `lunaria-syntax`, `lunaria-compiler`,
`lunaria-ffi`, `lunaria-stdlib`, `lunaria-arc`) are added in roadmap order;
`members = ["crates/*"]` picks them up automatically.

## Contributing

All changes, including those made by maintainers, must be proposed on a topic
branch and merged through a pull request to `main`. See
[`CONTRIBUTING.md`](./CONTRIBUTING.md).

## License

Lunaria Annua is free software licensed under the GNU General Public License,
version 3 or later (`GPL-3.0-or-later`). See [`LICENSE`](./LICENSE).

