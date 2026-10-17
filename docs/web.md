# Web core notes (M2f)

The client layer without UI, in `web/src/net` and `web/src/play`. Wire contract: `protocol.md`; transport: `server.md`.

## Modules
- `net/api.ts`: `api.createGame/getGame/listDecks` over `fetch`; bodies are checked at runtime and a failure is an `ApiError {code, status, message}`. Codes: those of `crates/mtg-server/src/api.rs` (`invalid_request`, `unsupported_format`, `unknown_deck`, `forbidden_origin`, `not_found`, `method_not_allowed`, `payload_too_large`, `unsupported_media_type`, `internal`, `server_full`, `unavailable`) plus local `unknown`, `network`, `bad_response`.
- `net/parse.ts`: `parseFrame(data)` is the only door from the wire (JSON, known `type`, required fields, 1 MiB cap). Garbage is a `FrameError`, never a throw.
- `net/socket.ts`: `GameSocket` sends `Join` first and once per connection, retries, and reports `idle | connecting | open | reconnecting | replaced | stopped`. `createSocket`, `timers`, `random`, `origin` are injectable. `net/pacer.ts` holds sends to 30 at once, then 15 a second (the server allows 40 and 20 and closes a socket over it with 1008), so a bot or auto-pass screen is slowed, not dropped.
- `play/view.ts`: pure `applyBatch`, `fromFirstBatch`, `fromSnapshot` (ports of `StateView::apply`); unchanged objects, players and zones keep their identity.
- `play/session.ts`, `play/store.ts`: `createSession({game, token, name})` gives `{store, connect, disconnect, ready, answer, concede, chat, resync, dismissNotice}`. Make one per game page (not per render). In React: `useStore(session.store, selector)`, `connect()` in an effect, `disconnect()` in its cleanup.

## State flow
Socket event -> session (the client rule) -> `store.setState`. A Snapshot sets the base and `seq`. A batch with any other `from_seq` (or any batch but the one at 0 while there is no view) sends one Resync; nothing is applied and decisions are ignored until the next Snapshot. `Decision` replaces `pending`; `answer()` sends once per id and clears it. A Snapshot or a lost connection forgets answered ids, so the server's repeat of a decision can be answered again. `GameOver` sets `result` and makes a later close final.

## Reconnect
| Close or message | Result |
| 1001, 1006, 1013 and any code not listed below | retry after 250 ms doubling to 15 s, less up to 50 % jitter; a Welcome resets it |
| 4000 | `replaced`, no retry; `connect()` takes the seat back |
| 1000, 1002, 1007, 1008, 1009; `Error` UnsupportedVersion or UnknownGame | `stopped`, reason in `notice` |
| any close after `GameOver` | `stopped`, no notice |

StaleDecision and IllegalAnswer give a notice and a Resync; BadMessage and Internal only a notice. A wrong token is a refused upgrade, which a browser reports as 1006, so the client retries forever (`attempt` counts).

## Tests
`scripts/check.sh web`, or `pnpm test` in `web/`. Fakes in `web/src/test-utils`: `FakeNetwork`, `FakeTimers`, `startSession()`, the Rust-generated `transcripts`. `vite` proxies `/api` and `/ws` to 127.0.0.1:8080 and keeps `Host`, which the server's Origin check compares with the page's `Origin`.
