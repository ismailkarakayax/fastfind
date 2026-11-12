# fastfind

**English** | [Türkçe](README.tr.md)

Blazing fast, parallel **file name** search for the command line, written in Rust.
Type part of a name and get the full path of every matching file and folder.

```console
> fastfind report C:\Users
C:\Users\me\Documents\report_2025.docx
C:\Users\me\Desktop\old\Report.pdf
2 results · 184,302 entries scanned · 0.41s
```

## Why?

Windows Explorer search is fast only for indexed locations. Outside the index
(other drives, project folders, external disks) it crawls the disk slowly, and
`dir /s` or `Get-ChildItem -Recurse` walk the tree on a single thread.

fastfind walks the directory tree on **all CPU cores at once** and only looks
at names, never file contents.

## Benchmark

Searching for `report` in a user folder with **710,835 entries** (hidden files
included for every tool, warm cache, 5 runs each, measured with
[hyperfine](https://github.com/sharkdp/hyperfine)):

| Command | Mean time | |
|---|---:|---|
| **`fastfind`** | **1.43 s** ± 0.18 | |
| `dir /s /b /a` | 13.24 s ± 0.15 | 9.3× slower |
| `Get-ChildItem -Recurse -Force` | 25.95 s ± 0.32 | 18.2× slower |

<sub>AMD Ryzen 5 9600X (6 cores / 12 threads), Kingston 1 TB SSD, Windows 10.</sub>

Reproduce on your own machine with [`scripts/bench.ps1`](scripts/bench.ps1).

## Installation

You need [Rust](https://rustup.rs).

```bash
git clone https://github.com/<user>/fastfind
cd fastfind
cargo install --path .
```

## Usage

```text
fastfind <PATTERN> [PATH...]
```

| Example | What it does |
|---|---|
| `fastfind invoice` | Names containing "invoice" (any case) under the current folder |
| `fastfind invoice D:\ E:\` | Search several drives at once |
| `fastfind -g "*.pdf" C:\Users` | Glob: every PDF |
| `fastfind -r "^IMG_\d{4}"` | Regular expression |
| `fastfind report -e pdf -e docx` | Only these extensions |
| `fastfind node_modules -t d` | Only directories (`-t f` for files only) |
| `fastfind report -d 2` | At most 2 levels deep |
| `fastfind secret -H` | Include hidden files and folders |
| `fastfind config --gitignore` | Respect `.gitignore` rules |
| `fastfind log -n 10` | Stop after 10 results |
| `fastfind log -q \| more` | No summary line, handy for piping |

Matching is done on the **file name only**, case-insensitively unless `-s` is
given. Results are always printed as absolute paths. The summary line goes to
stderr, so stdout stays clean for pipes.

Exit codes: `0` = found something, `1` = no matches, `2` = error.

Run `fastfind --help` for all options.

## How is it fast?

- **Parallel traversal:** uses the [`ignore`](https://crates.io/crates/ignore)
  crate's work-stealing walker (the engine behind ripgrep and fd), so every core
  reads directories at the same time.
- **Names only:** no file is ever opened; the name comes straight from the
  directory listing.
- **SIMD literal search:** substring searches are compiled by the `regex` crate
  into a vectorized literal matcher.
- **No shared hot counters:** each thread counts locally and merges once at the
  end, so threads don't fight over a cache line.
- **Single buffered writer:** workers send hits over a channel to one thread
  that writes through a 64 KB buffer, instead of locking stdout per line.
- **Release profile:** fat LTO, one codegen unit, `panic = "abort"`.

## Roadmap

- [ ] Optional index/cache for instant repeat searches
- [ ] Reading the NTFS Master File Table directly (like *Everything*)
- [ ] Fuzzy matching
- [ ] Prebuilt binaries on GitHub Releases

## License

[MIT](LICENSE)
