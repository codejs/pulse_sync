# Architecture

## Process model

```
Browser (React UI) ──HTTP/WS, 127.0.0.1 only──▶ pulsed ◀──QUIC (M2)──▶ pulsed on other devices
```

One daemon per device. It owns the file index and (from M2) talks to peers.
The UI is a client of the local daemon only.

## Crates

| Crate | Responsibility | Depends on |
|---|---|---|
| `pulse-core` | Ids, models, version vectors, FastCDC chunking + BLAKE3. Pure, no I/O. | — |
| `pulse-store` | SQLite schema and queries (sqlx, embedded migrations). | core |
| `pulse-fs` | Scanner, `.pulseignore`, watcher, `FolderManager` runners. | core, store |
| `pulse-api` | axum REST + WebSocket, auth, DTOs exported to TS. | core, store, fs |
| `pulse-sync` | `pulsed` binary: config, secrets, wiring, embedded UI. | all |
| `pulse-net` *(M2)* | iroh endpoint, pairing, index exchange, chunk transfer. | core, store |

## Identity and local security

- **Device id**: Ed25519 public key (`<data-dir>/device.key`, mode 0600),
  shown as base32. It's the same key type iroh uses for node ids, so M2 can
  reuse it directly. Version vectors key on its first 8 bytes (`short()`).
- **API token**: 32 random bytes (`<data-dir>/api-token`, 0600). Accepted as a
  `Bearer` header, as the `pulse_token` cookie (set as `HttpOnly; SameSite=Strict`
  when the embedded UI loads), or as `?token=` on `/api/events` only.
- **Host check**: every request must carry a `localhost`/`127.0.0.1`/`[::1]`
  Host header, which blocks DNS-rebinding attacks from web pages.
- The daemon refuses non-loopback `--listen` addresses.

## Folder model

A folder has a random id (`abcde-fgh23`) that peers will agree on. The local
path is per-device. Per-device settings: mode (send-receive / send-only /
receive-only), paused, rescan interval.

Guards when adding a folder: absolute path, exists, not `/`, doesn't overlap
another folder, doesn't overlap the daemon's data dir (otherwise the daemon
would index its own database writes).

## Index

`files` holds one row per path ever seen in a folder:

- `size`, `mtime_ns`, `hash` (whole-file BLAKE3)
- `version`: a version vector (`{deviceShort: counter}`), bumped on each local
  content change, so M2 can tell "newer" apart from "concurrent edit → conflict"
- `sequence`: a per-folder monotonically increasing change number, so peers can
  ask "what changed since N"
- `deleted`: tombstones, so deletions propagate

`chunks` holds each file's content-defined chunks (FastCDC, 64 KiB min, 256 KiB
avg, 1 MiB max) with their BLAKE3 hashes, indexed by hash so M2 can reuse local
data instead of downloading it.

A changed mtime with identical contents updates the mtime only: no new version
and no sync traffic.

## Scanning

`FolderManager` runs one task per active folder:

1. Initial full scan.
2. Rescan on (a) debounced watcher events (1.5 s), (b) user request, or
   (c) the periodic interval (default 1 h), which covers missed events.
   Triggers arriving during a scan are coalesced into one follow-up scan.

A scan walks the tree on a blocking thread (`ignore` crate, honouring
`.pulseignore`, skipping `.pulse/`, `.pulse-tmp-*` and symlinks). It hashes only
files whose size or mtime changed, and streams changes to the async side, which
commits them in batches of 256. A file that changes while it's being hashed is
skipped until the next event. Paths the walk couldn't read are never tombstoned.

The UI gets `folderStatusChanged`, `indexUpdated` and `foldersChanged` events
over `/api/events` and updates its TanStack Query cache from them.

## Roadmap

- **M2 — two peers on a LAN**: `pulse-net` with iroh; pairing by ticket/QR;
  folder share offers/accepts; index exchange by sequence; chunk fetching;
  atomic writes via `.pulse-tmp-*` + rename.
- **M3 — beyond the LAN**: hole punching + relay, reconnect/resume.
- **M4 — UI**: devices page, transfer progress, pairing QR.
- **M5 — robustness**: conflict copies, versioning/trash in `.pulse/`,
  bandwidth limits, case-insensitive filesystem handling, symlink policy.
