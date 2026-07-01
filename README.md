# nukenpm

An interactive terminal UI to find and **nuke** heavy directories like
`node_modules` and reclaim disk space — inspired by
[npkill](https://www.npmjs.com/package/npkill), written in Rust with
[ratatui](https://github.com/ratatui/ratatui).

## Features

- 🔍 Recursively scans for `node_modules` (or any directory name you choose)
- ⚡ Background scanning — the UI stays responsive while a huge tree is walked
- 📦 Shows each directory's size and last-modified age
- 🗑️ Delete directories interactively; deletions run off the UI thread
- 📊 Live totals: how much is reclaimable and how much you've freed
- 🔀 Sort by size, path, or age

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
```

### Keybindings

| Key            | Action              |
| -------------- | ------------------- |
| `↑` / `k`      | Move up             |
| `↓` / `j`      | Move down           |
| `space` / `del` / `enter` | Delete selected directory |
| `s`            | Cycle sort mode (size → path → age) |
| `q` / `esc` / `ctrl-c` | Quit        |

## How it works

- A background thread walks the tree, and when it hits a target directory it
  measures the size and reports it — without descending into it (so nested
  `node_modules` are never double-counted).
- Deletions are spawned onto their own threads and stream their result back, so
  the interface never blocks while a large tree is removed.
- Symlinks are never followed, avoiding cycles and off-tree files.

## Development

```bash
cargo test    # unit tests for the scanner and size helpers
cargo run     # run against the current directory
```
