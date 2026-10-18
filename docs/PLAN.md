# Plan

Design of the web-based Magic: The Gathering platform for friends: goals, architecture, engine, data, testing, deployment and milestones. Progress is tracked in [ROADMAP.md](ROADMAP.md).

## Goals

- A browser-hosted MTG game on a Rust backend, a TypeScript client and Postgres, deployable to a k3s cluster behind the owner's domain.
- Official precon decks and custom deck building; Normal and Commander; any number of players.
- Arena-style guided play: an explicit "nothing to do", highlighted playable instants and an explicit pass.
- A typed effect engine with a manual-resolve fallback for cards it does not implement.
- A goal-driven test suite.

## Frontend

- Stack: Vite, React and strict TypeScript, Tailwind + shadcn/ui, TanStack Router (file-based SPA, no SSR) and Query, zustand (live game state keyed by object id, read through selectors), dnd-kit, Motion, Vitest + Testing Library, Playwright. TanStack Virtual for card lists is planned.
- React over Svelte: shadcn/ui and TanStack are React-first, and selector-based stores recover most of Svelte's fine-grained reactivity. The spike results are in [spike-react.md](spike-react.md).

## Architecture

A Cargo workspace in which each crate has one responsibility and the engine has no IO.

| Crate | Responsibility |
|---|---|
| `mtg-core` | Newtype ids, mana, colors, card characteristics, shared value types |
| `mtg-engine` | Deterministic rules engine |
| `mtg-cards` | Ability and effect DSL, card definitions, oracle-text compiler, coverage status |
| `mtg-import` | Scryfall bulk data and rulings, MTGJSON precons, deck-list parsers and exporters, validation |
| `mtg-db` | Postgres schema and repositories for card data, precon decks and import runs |
| `mtg-protocol` | Client-server messages and per-seat state views; TypeScript types generated with ts-rs so client and server cannot drift |
| `mtg-server` | axum REST and WebSocket, auth, lobbies, game actors, sqlx persistence |
| `mtg-testkit` | Scenario DSL, synthetic cards, invariant checkers, bot players |
| `web/` | React client |

### Engine

Details are in [engine.md](engine.md).

- Decision-driven: the engine runs until a player must decide and publishes a `Decision` with every legal option. The UI highlights exactly those options and guesses nothing, which yields the Arena behavior: an explicit "nothing to do" when nothing is legal, highlighted instants, an active pass.
- Stops and auto-yield are a server-side preference layer over decisions: the server auto-passes only when the player has no legal action or the player's stops say so, and priority can be held.
- Event-sourced with a seeded RNG: state is the fold of the events, which gives deterministic replay, reconnect, persistence and failures reproducible from a seed.
- Characteristics are derived from base values plus continuous effects through the layer system (CR 613) with timestamps and dependencies. All buffs, type changes, counters, auras and "until end of turn" effects take one `ContinuousEffect { layer, selector, modification, duration, timestamp }` path.
- Keywords are defined once in the engine and referenced by cards, so one test covers every card with the keyword.
- Rules breadth: replacement and prevention effects (CR 614, 615), triggered abilities (CR 603, delayed and reflexive included), static, activated and mana abilities, state-based actions to a fixpoint (CR 704), the stack, APNAP order, combat and target re-validation on resolution.
- Formats are a trait (`FormatRules`: deck validation, starting life, mulligan, extra zones and rules). `Normal` is 60 cards, 20 life and the London mulligan. `Commander` is 100 singleton, colour identity, command zone, tax, 40 life, 21 commander damage, partner and companion.
- Any player count: attacks go at any opponent or planeswalker, and elimination and leaving the game are handled.
- Card DSL: declarative data (RON or JSON in the repo and the database) built from `Cost`, `Selector`, `Condition`, `Effect`, `Duration` and `TargetSpec`. Each card has `impl_status: Full | Partial | Manual`.
- Manual fallback: a `Manual` card shows its oracle text and offers generic logged actions (move card, counters, life, create token, tap) that other players can see and object to, so no deck is blocked.
- Oracle-text compiler: a template-based parser compiles common wordings to the DSL and leaves the rest `Manual`. A coverage report ranks unimplemented cards by frequency across imported decks and precons.

### Data and import

- Scryfall bulk data (Oracle Cards, Default Cards, Rulings), never per-card scraping, with the required headers and rate limits; images from the Scryfall CDN. MTGJSON supplies precon deck lists. Sources and limits are in [data-sources.md](data-sources.md).
- The Comprehensive Rules text is vendored so tests can cite rules (`CR 702.7`).
- Deck import parses plain text (set and collector variants, sideboard and commander sections), MTGO `.dek`, Arena export and Moxfield or Archidekt text; exporters round-trip. Name resolution gives fuzzy suggestions and per-line errors.
- The import job is idempotent and resumable (`import_runs`) and runs from the CLI or scheduled in the cluster.
- Schema (sqlx migrations, compile-time-checked queries): `cards`, `printings`, `rulings`, `precon_decks`, `precon_deck_cards`, `import_runs`, `users`, `sessions`, `games`, `seats`, `game_events`; `decks` and `deck_cards` are planned. Search uses `pg_trgm` and `tsvector`, with indexed colour, type and mana-value filters.
- Auth: username and password (argon2) with a session cookie; games are joined through per-seat invite links.

### UX principles

- The engine's legal options are the single source of truth for what a player can do.
- A state banner says "Your priority", "Waiting for X" or "No attackers available".
- Targeting dims illegal objects; the stack and the full game log stay visible; cards zoom on hover.
- The N-player layout focuses one opponent and compacts the others; commander damage and tax show on seats.

## Testing

Each test module opens with a one-line `Goal:`. Tests assert mechanics rather than single cards and prefer table-driven cases.

1. Unit: mana and costs, selectors, layer ordering, dependency resolution, deck validation, parsers.
2. Mechanic conformance: synthetic cards in `mtg-testkit` exercise each keyword and rule once, tagged with CR numbers.
3. Corpus: every implemented card loads, validates and has keywords matching its oracle text; parser golden files; a coverage ratchet (the implemented-card count may not drop).
4. Property and invariant (proptest): card conservation, one zone per object, state-based actions reach a fixpoint, `replay(events)` equals the live state hash, no stuck state.
5. Fuzz games: seeded random-legal-action bots play thousands of N-player games in both formats; a failure prints its seed.
6. Scenario and golden: a scenario DSL (board, script, assertions), seeded from Scryfall rulings for tricky interactions.
7. Import: recorded fixtures with no live network, round trips `parse(export(d)) == d`, malformed input.
8. Server integration: real Postgres (`sqlx::test`), WebSocket clients, bot-vs-bot games, reconnect and restart-resume.
9. Frontend: Vitest and Testing Library for components and stores; Playwright end-to-end on the full stack.
10. CI (GitHub Actions): fmt, clippy `-D warnings`, `cargo test --workspace`, a check that `.sqlx/` is current, and the web typecheck, lint, format, tests and e2e. No change merges without tests that justify it.

## Code standards

The rules are in `CLAUDE.md`. Workspace lints forbid `unsafe` and deny `unwrap` and `expect`; clippy runs pedantic with a curated allowlist. TypeScript is strict, organized by feature folder, checked with oxlint and Prettier, and uses only the generated protocol types.

## Deployment

- A multi-stage Docker image holds the Rust server and the built SPA, served by the server or by nginx.
- Kustomize or Helm manifests; Postgres via CloudNativePG, or a StatefulSet to start; k3s Traefik ingress with cert-manager TLS on the owner's domain, WebSocket-capable.
- Each game lives in memory in a game actor with its event log persisted, so a single replica can restart and resume. Scale-out is out of scope.
- Use is private and non-commercial among friends, which fits Wizards' Fan Content Policy; card art stays attributed and nothing is monetized.

## Milestones

Vertical slices, each with its tests.

- **M0 Foundations**: workspace, CI, `CLAUDE.md`, lint and test tooling, Postgres dev setup, frontend spike.
- **M1 Card data**: schema and migrations, Scryfall and MTGJSON import, card search API, deck parsers and validation, precon listing.
- **M2 Engine core and first playable**: zones, turns, priority, stack, mana, state-based actions, lands and spells, basic combat, testkit; a 2-player vanilla game playable in the browser with highlight and pass UX.
- **M3 Rules depth**: all keywords, layers and continuous effects, triggers, replacement effects, targeting, counters, auras and equipment, tokens.
- **M4 Multiplayer product**: auth, lobbies, N players, reconnect, deck builder UI, precon browser, game log and stack UI.
- **M5 Commander**: format rules, command zone, tax, commander damage, partner; Commander-specific tests.
- **M6 Coverage**: manual fallback mode, oracle-text compiler, coverage dashboard; 100% of a chosen set of precons, then expansion by frequency.
- **M7 Polish**: animations, stop settings, spectators, replays, solo bots, deployment hardening.

Each milestone is verified end to end:

- `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` are green, and the fuzz suite passes with fixed seeds.
- With `docker compose up -d db`, the import job loads cards and Playwright plays a full 2-player and 4-player game (Normal and Commander) with a precon and an imported custom deck.
- A manual check of the Arena-style UX: the no-attackers message, instant highlighting and the explicit pass.
- With the manifests applied to k3s, a WebSocket game works through the domain with TLS.
