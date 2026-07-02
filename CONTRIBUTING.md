# Contributing

Notes for developing and releasing `nukenpm`.

## Repository layout

This is a monorepo. Each app lives in its own top-level directory:

| Path   | What                                            |
| ------ | ----------------------------------------------- |
| `cli/` | The Rust command-line app (the only one so far) |

Future `web/` and `macos/` apps will sit alongside `cli/`.

Because the Cargo project lives under `cli/`, run Cargo commands from there
(`cd cli`) or pass `--manifest-path cli/Cargo.toml`.

## Building from source

Requires a [Rust toolchain](https://rustup.rs).

```bash
cd cli
cargo build --release
# binary at cli/target/release/nukenpm
```

## Development

```bash
cd cli
cargo run      # run against the current directory
cargo test     # unit tests for the scanner and size helpers
```

CI (`.github/workflows/build.yml`) builds and tests on every push/PR that
touches `cli/`.

## How it works

- A background thread walks the tree. When it hits a target directory it measures
  the size and reports it — without descending into it, so nested `node_modules`
  are never double-counted.
- Deletions are spawned onto their own threads and stream their result back, so
  the interface never blocks while a large tree is removed.
- Symlinks are never followed, avoiding cycles and off-tree files.
- When every discovered directory has been reclaimed, the session summary appears
  automatically; press `r` to scan again.

## Releasing

Distribution is via a Homebrew tap using **pre-built binaries**, so end users
don't need Rust. Cutting a release is two steps:

1. Bump `version` in `cli/Cargo.toml`, then refresh the lockfile:
   ```bash
   cargo update -p nukenpm --manifest-path cli/Cargo.toml
   ```
   Commit both.
2. Tag and push:
   ```bash
   git tag vX.Y.Z && git push origin vX.Y.Z
   ```

`.github/workflows/release.yml` then, on the `v*` tag:

- builds the binary for macOS (arm64 + x86_64) and Linux (x86_64),
- packages each as `nukenpm-<version>-<target>.tar.gz` (binary + README + LICENSE),
- computes each tarball's SHA256, and publishes them to the GitHub Release
  (using the pre-installed `gh` CLI — no third-party actions), then
- the `bump-formula` job renders `.github/nukenpm-formula.rb.tmpl` with the new
  version + SHA256s and pushes `Formula/nukenpm.rb` to
  [`preschian/homebrew-tap`](https://github.com/preschian/homebrew-tap).

The **canonical formula lives in `.github/nukenpm-formula.rb.tmpl`** — edit it
there, not in the tap, since the tap copy is regenerated on every release.

### One-time setup for the auto-bump

The `bump-formula` job pushes to a *different* repo, which the default
`GITHUB_TOKEN` can't do. Create a **fine-grained PAT** scoped to
`preschian/homebrew-tap` only, with `Contents: Read and write`, and store it as
an Actions secret named `HOMEBREW_TAP_TOKEN` in `preschian/nukenpm`:

```bash
gh secret set HOMEBREW_TAP_TOKEN -R preschian/nukenpm
```

### Gotchas

- **The source repo must be public.** Homebrew downloads release assets
  anonymously; on a private repo the asset URLs return `404` even though the API
  lists them, so `brew install` fails. If the source ever needs to stay private,
  host the tarballs in a separate public repo instead.
- The release tarball wraps the binary in a top-level
  `nukenpm-<version>-<target>/` directory; Homebrew descends into that single dir
  automatically, so `bin.install "nukenpm"` works as-is.
- Installing from a fresh third-party tap prints a "Tap-Trust" warning — expected,
  and it doesn't block the install.
- macOS binaries are not code-signed/notarized. Homebrew installs fine; if
  Gatekeeper ever complains about non-brew downloads, consider signing.

## Conventions

- Commits and PR titles follow [Conventional Commits](https://www.conventionalcommits.org).

## Ideas / TODO

- Add an `aarch64-unknown-linux-gnu` (Linux ARM) target to the release matrix
  (needs cross-compilation via `cross` or an ARM runner).
