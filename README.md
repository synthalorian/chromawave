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

Extract dominant colors from a PPM P3 image using median-cut quantization:

```sh
chromawave palette examples/gradient.ppm --count 4
# # chromawave palette v1
# # source: examples/gradient.ppm (16 pixels)
# #0000ff
# #4b00b4
# #af0050
# #f1000d
```

Mix two hex colors by ratio (0.0 = all color1, 1.0 = all color2):

```sh
chromawave mix '#ff0066' '#00ccff' 0.35
# #a6479c
```

Mix with a blend mode (multiply, screen, overlay):

```sh
chromawave mix '#ff0066' '#00ccff' 0.5 --mode multiply
# #800066
```

Blend two colors directly (no ratio):

```sh
chromawave blend '#ff0066' '#00ccff' --mode screen
# #ffccff
```

Check WCAG 2.1 contrast ratio:

```sh
chromawave contrast '#f8f8f2' '#080812'
# 18.69 AAA
```

Generate a palette from a base color:

```sh
chromawave generate '#ff0066' --scheme triadic
# # chromawave generate v1
# # base: #ff0066 scheme: triadic
# #ff0066
# #66ff00
# #0066ff
```

Available schemes: `complementary`, `analogous`, `triadic`, `split-complementary`.

## Image format

v0 supports PPM P3 (ASCII) only. This is a deliberate choice: P3 is human-readable, trivially parseable without external crates, and sufficient for testing palette extraction. PNG/JPEG support is on the roadmap behind a feature flag.

To convert images to PPM P3:

```sh
# Using ImageMagick (if installed):
convert photo.png -compress none photo.ppm

# Using FFmpeg (if installed):
ffmpeg -i photo.jpg -pix_fmt rgb24 photo.ppm
```

## Architecture

`src/main.rs` contains the complete v0 implementation: parsing, validation, pure core functions, CLI dispatch, and unit tests. The next extraction boundary is a `core` module once the format stabilizes; until then, keeping the tape on one reel makes audits cheap.

## Roadmap

- [x] P3 PPM palette extraction (median-cut quantizer)
- [x] Hex mixing and contrast (WCAG 2.1)
- [x] Blend modes (normal, multiply, screen, overlay)
- [x] Palette generation (complementary, analogous, triadic, split-complementary)
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
Made by [synth](https://github.com/synthalorian) with blackclaw ⚫🦞
