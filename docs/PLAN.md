# Plan

Goals and architecture of the web-based Magic: The Gathering platform for friends. Progress is tracked in [ROADMAP.md](ROADMAP.md).

## Goals

- A browser-hosted MTG game on a Rust backend, a TypeScript client and Postgres, deployed to a k3s cluster behind the owner's domain.
- Official precon decks and custom deck building; Normal and Commander; any number of players.
- Arena-style guided play: an explicit "nothing to do", highlighted playable instants and an explicit pass.
- A typed effect engine with a by-hand fallback for card text it does not run.
- Every card Verified automatically; manual play is the fallback.

## Architecture

A Cargo workspace in which each crate has one responsibility and the engine has no IO.

| Crate | Responsibility |
|---|---|
| `mtg-core` | Newtype ids, mana, colors, card characteristics, shared value types |
| `mtg-engine` | Deterministic rules engine |
| `mtg-cards` | Ability and effect DSL, card definitions, oracle-text compiler, support states |
| `mtg-import` | Scryfall bulk data and rulings, MTGJSON precons, deck-list parsers and exporters |
| `mtg-db` | Postgres schema and repositories |
| `mtg-cli` | Fetch and ingest card data, compile, coverage, Forge translator |
| `mtg-protocol` | Client-server messages and per-seat state views; TypeScript types generated with ts-rs |
| `mtg-server` | axum REST and WebSocket, auth, lobbies, game actors, sqlx persistence |
| `mtg-testkit` | Scenario DSL, synthetic cards, invariant checkers, bot players |
| `web/` | React client: Vite, Tailwind + shadcn/ui, TanStack Router, zustand, dnd-kit, Motion |

## Principles

- The engine is decision-driven: it runs until a player must decide and publishes a `Decision` with every legal option. The UI highlights exactly those options and guesses nothing. See [engine.md](engine.md).
- State is the fold of an event log under a seeded RNG, which gives deterministic replay, reconnect, persistence and failures reproducible from a seed.
- Characteristics are derived from base values plus continuous effects (layers, CR 613); keywords are implemented once in the engine and referenced by cards.
- Formats are a trait (`FormatRules`); Normal and Commander share the engine.
- Cards are JSON rules files in the DSL; support is computed from the file and the engine's capabilities. See [cards-dsl.md](cards-dsl.md).
- Card data comes from Scryfall bulk files (never per-card scraping) and MTGJSON precons; see [data-sources.md](data-sources.md). The Comprehensive Rules text is vendored so tests cite rules (`CR 702.7`).
- Each game lives in memory in a game actor with its event log persisted, so a single replica restarts and resumes. Scale-out is out of scope.
- Use is private and non-commercial among friends, in line with Wizards' Fan Content Policy; card art stays attributed.
- Tests are goal-driven; see [testing.md](testing.md). CI runs when a release is published, not on pushes.
