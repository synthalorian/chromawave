# chromawave

Palette extraction, mixing, and contrast checks from the CLI.

## Why this exists

The current project fleet already covers agent frameworks, music software, games, privacy, sync, mobile, and archival tooling. `chromawave` fills a narrower gap: a small local-first utility that can be audited in one sitting and composed with OpenShark, OpenShield, shell scripts, or other agents.

## v0 scope

- No network access.
- No external Rust dependencies.
- Deterministic output where the filesystem allows it.
- Plain text formats that can be reviewed in Git.
- Real unit tests, not placeholder stubs.

## Commands

```sh
chromawave palette cover.ppm --count 8
```
```sh
chromawave mix '#ff0066' '#00ccff' 0.35
```
```sh
chromawave contrast '#f8f8f2' '#080812'
```

## Architecture

`src/main.rs` contains the complete v0 implementation: parsing, validation, pure core functions, CLI dispatch, and unit tests. The next extraction boundary is a `core` module once the format stabilizes; until then, keeping the tape on one reel makes audits cheap.

## Roadmap

- [ ] P3 PPM palette extraction
- [ ] Hex mixing and contrast
- [ ] PNG decoder behind no-network feature
- [ ] Tailwind/CSS token exporters

## Development

```sh
cargo fmt --check
cargo test
cargo run -- --help
```

## Safety

Local commits only. Never push or create remotes without explicit instruction. Do not weaken validation to make a failing test pass.

---
Made by [synth](https://github.com/synthalorian) with synthclaw 🎹🦞
