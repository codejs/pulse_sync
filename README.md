# Pulse Sync

Peer-to-peer file sync. Every device runs the same Rust daemon (`pulsed`); the
React web UI is the control panel for the daemon on that device. There is no
central server holding your files.

> **Status:** M1 — local indexing. Folders can be added, scanned, watched and
> browsed. Device pairing and actual syncing between peers arrive in M2.

## Layout

```
crates/
  pulse-core/    domain types, version vectors, content-defined chunking (no I/O)
  pulse-store/   SQLite metadata: folders, file index, chunks (sqlx + migrations)
  pulse-fs/      scanner, .pulseignore, file watcher, per-folder runners
  pulse-api/     local REST + WebSocket API (axum), token auth
  pulse-sync/    the `pulsed` binary: wiring, secrets, embedded web UI (the released package)
web/             React + TypeScript UI (Vite, TanStack Query/Router, Tailwind, shadcn/ui)
  src/api/generated/   TypeScript types generated from Rust by ts-rs — don't edit
docs/            architecture notes
```

## Prerequisites

- Rust (stable, via rustup)
- Node 22+ and pnpm

That's all. The [just](https://github.com/casey/just) task runner is a dev
dependency (`rust-just`), so `pnpm install` sets it up.

## Development

```sh
pnpm install   # web dependencies + the `just` task runner
pnpm dev       # daemon on 127.0.0.1:8417 + Vite on http://localhost:5173
```

The dev daemon keeps its data in `./.pulse-dev/` (git-ignored). The Vite dev
server proxies `/api` to the daemon and attaches the API token from
`.pulse-dev/api-token`, so the browser never handles it.

| Command | What it does |
|---|---|
| `pnpm dev` | Daemon and web dev server together (Ctrl-C stops both) |
| `pnpm daemon` / `pnpm web` | Just one of them; `pnpm daemon --listen 127.0.0.1:9000` passes flags |
| `pnpm test` | Rust and web tests (also regenerates TypeScript types) |
| `pnpm lint` | rustfmt, clippy, Biome, tsc: what CI checks |
| `pnpm fmt` | Apply formatters |
| `pnpm gen-types` | Regenerate `web/src/api/generated/` from Rust |
| `pnpm build` | Release binary with the web UI embedded |
| `pnpm reset-dev` | Delete the dev data directory |
| `pnpm archive` | Zip the source (everything not git-ignored, including uncommitted changes) into `archives/`; `-o file.zip` to choose the path |

These all run recipes from the `justfile`; `pnpm just --list` shows them, and
if you have `just` installed globally, `just dev` etc. work directly.

Change a Rust type that's exported to TypeScript (anything with
`#[derive(TS)]`)? Run `pnpm gen-types` and commit `web/src/api/generated/`.
CI fails if they drift.

## Installing

From a [GitHub release](https://github.com/codejs/pulse_sync/releases):

```sh
# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/codejs/pulse_sync/releases/latest/download/pulse-sync-installer.sh | sh

# Homebrew
brew install codejs/tap/pulse-sync
```

```powershell
# Windows (or download the .msi from the release page)
powershell -ExecutionPolicy Bypass -c "irm https://github.com/codejs/pulse_sync/releases/latest/download/pulse-sync-installer.ps1 | iex"
```

Then run `pulsed` and open http://127.0.0.1:8417.

Options: `--data-dir` / `PULSE_DATA_DIR` (default: the OS data directory),
`--listen` / `PULSE_LISTEN` (loopback only, default `127.0.0.1:8417`).

To build it yourself: `pnpm build`, then run `./target/release/pulsed`.
Releases are described in [docs/releasing.md](docs/releasing.md).

### Folders

- Each shared folder gets a `.pulse/` directory. It's the safety marker: if it
  disappears (say, an unmounted drive), the folder goes into an error state
  instead of recording every file as deleted.
- A `.pulseignore` file (gitignore syntax) in any directory excludes paths.
- Removing a folder only forgets its index; files on disk are never touched.

See [docs/architecture.md](docs/architecture.md) for the design and roadmap.
