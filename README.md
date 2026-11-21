# Lunaria Annua

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
