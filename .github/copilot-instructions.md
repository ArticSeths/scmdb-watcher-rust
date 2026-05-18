# SCMDB Watcher — Development Rules

## Project Overview

Tauri 2 desktop app (Rust backend + vanilla JS frontend) that tails Star Citizen's `Game.log`, parses mission/blueprint events, and exposes them via an SSE server for consumption by scmdb.net.

## Wire Protocol

- The SSE server on `127.0.0.1` emits JSON events consumed by **scmdb.net**.
- All serialized field names MUST be **camelCase** (`debugName`, `startTs`, `productName`, `missionGuid`, `missionTrigger`, `sourceLogs`, `duplicatesMerged`, etc.).
- The enum tag (`"type"` field) uses **snake_case** values (`mission_start`, `mission_complete`, `blueprint_received`, `session_reset`, `state_snapshot`) — this matches the Python reference watcher.
- Use `#[serde(rename_all = "camelCase")]` on serializable structs, and per-field `#[serde(rename = "camelCase")]` on inline enum variant fields (serde doesn't support `rename_all` on tagged enum variant fields).

## Architecture Conventions

- **Shared regex patterns** live in `src-tauri/src/watcher/patterns.rs`. Never duplicate regexes in `parser.rs` or `importer/mod.rs` — import from `patterns`.
- The SSE server's internal state struct is `SseState` (not `AppState`) to avoid collision with the Tauri `AppState` in `commands.rs`.
- `WatcherState`, `WatcherStateInner`, and `EventBus` implement `Default` (delegating to `::new()`).

## Performance

- The log tailer (`tailer.rs`) uses `AsyncSeekExt::seek()` to resume reading. **Never** read-and-discard bytes to skip forward — use `seek(SeekFrom::Start(file_pos))`.

## Code Quality

- `cargo clippy -- -D warnings` must pass with zero warnings.
- `cargo fmt --check` must pass (run `cargo fmt` before committing).
- Prefer `if let` over single-arm `match` (clippy: `single_match`).
- Avoid `Ok(expr?)` — return the inner expression directly (clippy: `needless_question_mark`).

## Frontend (`src/app.js`)

- Field access uses **camelCase only** (e.g., `event.debugName`, `event.startTs`, `result.sourceLogs`). No snake_case fallbacks.
- Config serialization uses snake_case (Rust struct field names) because it's local config, not wire protocol.

## CORS & Binding

- SSE server binds to `127.0.0.1` only (never `0.0.0.0`).
- Default allowed origins: `https://scmdb.net`, `https://www.scmdb.net`.
- Additional origins configurable via `custom_origins` in config file.

## Compatibility Target

- Wire protocol must be compatible with the Python reference watcher (https://github.com/KrovaxCode/SCMDB_LOG_WATCHER).
- Regex patterns must remain byte-identical to the Python version.
