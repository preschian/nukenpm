# nukenpm

An interactive terminal UI to find and **nuke** heavy directories like
`node_modules` and reclaim disk space — inspired by
[npkill](https://www.npmjs.com/package/npkill), written in Rust with
[ratatui](https://github.com/ratatui/ratatui).

## Features

- 🔍 Recursively scans for `node_modules` (or any directory name you choose)
- ⚡ Background scanning — the UI stays responsive while a huge tree is walked
- 📦 Shows each directory's size, file count and last-modified age
- ✅ Multi-select several directories, then reclaim them in one pass
- 🛡️ A confirmation dialog (with size + file totals) before anything is deleted
- 🗑️ Deletions run off the UI thread, so the interface never blocks
- 📊 Live "reclaimable" readout, plus a session summary of what you freed
- 🔀 Sort by size, modified time, or path
- 🕰️ Stale directories (untouched for 6+ months) are gently highlighted

## Install / Build

```bash
cargo build --release
# binary at ./target/release/nukenpm
```

## Usage

```bash
# scan the current directory for node_modules
nukenpm

# scan a specific directory
nukenpm ~/code

# hunt for a different directory name
nukenpm ~/code --target target      # Rust build dirs
nukenpm ~/projects -t .venv          # Python virtualenvs

# delete without the confirmation dialog
nukenpm ~/projects --yes
```

### Keybindings

| Key                    | Action                                  |
| ---------------------- | --------------------------------------- |
| `↑` / `k`, `↓` / `j`   | Move the cursor                         |
| `space`                | Select / deselect the highlighted row   |
| `a`                    | Select all / clear the selection        |
| `enter` / `del`        | Delete the selection (or cursor row)    |
| `s`                    | Cycle sort mode (size → modified → path) |
| `q` / `esc`            | Show the session summary                |
| `ctrl-c`               | Quit immediately                        |

In the confirmation dialog, `enter` / `y` confirms and `esc` / `n` cancels.
On the summary screen, `r` scans again and `q` / `esc` quits. Pass `--yes` to
skip confirmation entirely.

## How it works

- A background thread walks the tree, and when it hits a target directory it
  measures the size and reports it — without descending into it (so nested
  `node_modules` are never double-counted).
- Deletions are spawned onto their own threads and stream their result back, so
  the interface never blocks while a large tree is removed.
- Symlinks are never followed, avoiding cycles and off-tree files.
- When every discovered directory has been reclaimed, the session summary
  appears automatically; press `r` to scan again.

## Development

```bash
cargo test    # unit tests for the scanner and size helpers
cargo run     # run against the current directory
```
