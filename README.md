# nukenpm

Reclaim disk space by finding and **nuking** heavy folders like `node_modules`.

`nukenpm` is a fast, interactive terminal app: point it at a folder, it hunts
down all the space-hogging directories underneath, shows you how big each one is
and how long since you last touched it, and lets you wipe the ones you don't need
— all from a keyboard-driven list.

## Install

With [Homebrew](https://brew.sh) (macOS & Linux):

```bash
brew install preschian/tap/nukenpm
```

That's it — no other tools required. To update later: `brew upgrade nukenpm`.

> Prefer to build it yourself? See [CONTRIBUTING.md](CONTRIBUTING.md).

## What it does

- 🔍 Scans a folder (and everything inside it) for `node_modules`
- 📦 Shows each one's size, file count, and how old it is
- 🕰️ Highlights stale folders you haven't touched in 6+ months
- ✅ Pick several at once and reclaim them in a single sweep
- 🛡️ Asks for confirmation — with the total size — before deleting anything
- ⚡ Stays snappy the whole time, even on huge project folders
- 📊 Tells you exactly how much space you freed when you're done

## Usage

```bash
# scan the current folder
nukenpm

# scan a specific folder
nukenpm ~/code

# hunt for a different folder name instead of node_modules
nukenpm ~/code --target target      # Rust build folders
nukenpm ~/projects -t .venv          # Python virtual environments

# skip the confirmation prompt
nukenpm ~/projects --yes
```

### Keys

| Key                    | Action                                   |
| ---------------------- | ---------------------------------------- |
| `↑` / `k`, `↓` / `j`   | Move the cursor                          |
| `space`                | Select / deselect the highlighted row    |
| `a`                    | Select all / clear the selection         |
| `enter` / `del`        | Delete the selection (or the cursor row) |
| `s`                    | Change sort order (size → age → name)    |
| `q` / `esc`            | Show the session summary                 |
| `ctrl-c`               | Quit immediately                         |

In the confirmation dialog, `enter` / `y` confirms and `esc` / `n` cancels.
On the summary screen, `r` scans again and `q` / `esc` quits.

## Credits

Inspired by [npkill](https://www.npmjs.com/package/npkill). Built in Rust with
[ratatui](https://github.com/ratatui/ratatui).

## License

[MIT](LICENSE)
