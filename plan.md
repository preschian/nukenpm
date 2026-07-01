# Plan: Homebrew Distribution (pre-built binary)

**Goal:** Users without a Rust toolchain can `brew install` and immediately run
the `nukenpm` binary. This means Homebrew must **not** compile from source — the
formula must download an already-built binary.

Target install command:

```bash
brew install preschian/tap/nukenpm
```

---

## Why pre-built binary (not build-from-source)

| Approach | Needs Rust? | Install time | Maintainer effort |
|---|---|---|---|
| `cargo install` in formula | Yes (builds) | Slow (compiles) | Low |
| **Pre-built binary** (chosen) | **No** | **Instant** | Needs release CI |

Since the requirement is "users without Rust", we must ship a pre-built binary.

---

## Architecture

1. **Main repo** (`preschian/nukenpm`) — build cross-platform binaries in GitHub
   Actions on `v*` tags, then attach the tarballs to a GitHub Release.
2. **Tap repo** (`preschian/homebrew-tap`) — holds `Formula/nukenpm.rb` pointing
   at the per-platform binary tarballs + SHA256. Can be updated automatically by
   CI, or manually at first.

The formula uses the binary directly (`bin.install`), not `cargo`. No
`depends_on "rust"`.

---

## Target platforms (build matrix)

| OS | Arch | Rust target triple |
|---|---|---|
| macOS Apple Silicon | arm64 | `aarch64-apple-darwin` |
| macOS Intel | x86_64 | `x86_64-apple-darwin` |
| Linux | x86_64 | `x86_64-unknown-linux-gnu` |
| Linux | arm64 | `aarch64-unknown-linux-gnu` (optional) |

Homebrew runs on macOS + Linux, so the first three targets are the minimum.

---

## Implementation steps

### 1. Fill in `cli/Cargo.toml` metadata
Add fields to keep it tidy and usable by tooling:
```toml
description = "Interactive TUI to find and clean node_modules directories"
license = "MIT"
repository = "https://github.com/preschian/nukenpm"
homepage = "https://github.com/preschian/nukenpm"
```
Make sure `nukenpm --version` works (clap derive → enable via
`#[command(version)]`), since the formula's `test` block uses it.

### 2. Release workflow in the main repo (`.github/workflows/release.yml`)
Trigger: `push` on `v*` tags.
- Build matrix per target triple (working-directory `cli`).
  - For Linux arm64 cross-compile, use `cross` or an arm runner.
- Package each binary into a tarball: `nukenpm-<version>-<target>.tar.gz`
  (contains: the `nukenpm` binary + LICENSE + README).
- Compute the SHA256 of each tarball.
- Create a GitHub Release (e.g. `softprops/action-gh-release`) and upload all
  tarballs + checksums.

### 3. Tap repo `preschian/homebrew-tap`
Create a new repo (name **must** start with `homebrew-`). Add
`Formula/nukenpm.rb`:

```ruby
class Nukenpm < Formula
  desc "Interactive TUI to find and clean node_modules directories"
  homepage "https://github.com/preschian/nukenpm"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/preschian/nukenpm/releases/download/v0.1.0/nukenpm-0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "<sha_macos_arm64>"
    end
    on_intel do
      url "https://github.com/preschian/nukenpm/releases/download/v0.1.0/nukenpm-0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "<sha_macos_x86_64>"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/preschian/nukenpm/releases/download/v0.1.0/nukenpm-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "<sha_linux_x86_64>"
    end
  end

  def install
    bin.install "nukenpm"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/nukenpm --version")
  end
end
```

### 4. Automate formula updates (optional, phase 2)
After a release, a workflow bumps `version` + `sha256` in the tap repo
automatically (e.g. opening a PR against `homebrew-tap`). Manual is fine to
start.

### 5. First release
```bash
git tag v0.1.0
git push origin v0.1.0   # triggers the release workflow
```
Grab the SHA256 from the generated checksums → fill them into the formula →
commit to the tap.

### 6. Verify (on a machine without Rust)
```bash
brew install preschian/tap/nukenpm
nukenpm --version
```

---

## Checklist

- [ ] `cli/Cargo.toml` metadata + `--version` works
- [ ] `.github/workflows/release.yml` (build matrix + release + checksum)
- [ ] `preschian/homebrew-tap` repo created
- [ ] `Formula/nukenpm.rb` (binary, per-platform, no `depends_on rust`)
- [ ] Tag `v0.1.0` → first release
- [ ] Fill SHA256 into the formula
- [ ] Test `brew install` on a machine without Rust

---

## Notes

- The Cargo project now lives under `cli/`, so every build step uses
  `working-directory: cli` or `--manifest-path cli/Cargo.toml`.
- Homebrew-core (vs. a tap) requires notability (stars/forks/age) — not
  realistic early on. A personal tap is enough and still satisfies the
  "without Rust" requirement.
- macOS binaries are not code-signed/notarized; if Gatekeeper complains about
  non-brew downloads later, consider signing (out of scope for brew).
