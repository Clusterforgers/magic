# Plan: Web-based Magic: The Gathering platform (working name: "magic")

## Context
Greenfield project in `/home/chris/private/Clusterforgers/magic` (empty, not a git repo). Goal: a browser-hosted MTG game for friends: official/precon decks, custom deck building (TableCommander-style), unlimited players, Normal + Commander modes, Arena-style guided play (clear "nothing to do", highlighted playable instants, explicit choice to pass), a robust effects engine, and a strong, goal-driven test suite. Rust backend, TypeScript web client, Postgres, deployable to the user's k3s cluster behind their domain.

Decisions already made with the user:
- Card effects: **typed effect engine in Rust + manual-resolve fallback** for cards not yet implemented.
- Database: **Postgres**.
- Frontend: **TypeScript SPA**; framework choice settled below.

## Frontend decision (why not Svelte)
shadcn/ui and TanStack are React-first (shadcn-svelte and TanStack Svelte adapters exist but are second-class; TanStack Router is React/Solid). Given the user's experience and ecosystem fit, **React is the right pick**; Svelte's fine-grained reactivity advantage is manageable in React with selector-based stores.

Stack: Vite + React + TypeScript (strict), Tailwind + shadcn/ui, TanStack Router (file-based, SPA, **no TanStack Start**; no SSR needed), TanStack Query (REST: cards, decks, lobbies), Zustand or TanStack Store (live game state, keyed by object id with selectors), dnd-kit (drag/target), Motion (layout animations), TanStack Virtual (card lists), Vitest + Testing Library + Playwright.

How we make sure it's correct: a **time-boxed spike (Milestone 0)** with pass criteria, before committing:
- 4 players, ~150 permanents, server patches at 10/s, stays ≥60 fps (profiled).
- Hand→battlefield and attack/block animations via layout animations.
- Drag-to-target and click-to-target with legal-target highlighting.
- Card text/hover preview latency <50 ms.
If React misses these, revisit (Svelte 5 fallback); otherwise lock it in.

## Architecture
Cargo workspace; each crate has one responsibility; engine has **no IO**.

| Crate | Responsibility |
|---|---|
| `mtg-core` | Newtype IDs, mana/colors, zones, card characteristics, shared value types |
| `mtg-engine` | Deterministic rules engine (see below) |
| `mtg-cards` | Ability/effect DSL, card definitions, oracle-text→DSL compiler, coverage status |
| `mtg-import` | Scryfall bulk + rulings, MTGJSON precons, deck-list parsers/exporters, validation |
| `mtg-protocol` | Client↔server messages; TS types generated (ts-rs/specta) so client/server can't drift |
| `mtg-server` | axum REST + WebSocket, auth, lobbies, game actors, sqlx persistence |
| `mtg-testkit` | Scenario DSL, synthetic cards, invariant checkers, bot players |
| `web/` | React client |

### Engine design (the core risk)
- **Decision-driven state machine**: `engine.step()` runs until a player must decide, returning `Decision { player, kind, legal_options }` (priority, targets, attackers, blockers, costs, trigger order, modes, X). The UI highlights exactly `legal_options`; nothing is client-guessed. This directly delivers the Arena behaviour: no legal attackers/blockers → explicit, auto-advancing "nothing to do"; castable instants/abilities highlighted; player must actively pass.
- **Stops/auto-yield** are a server-side preference layer over decisions (auto-pass only when zero legal actions or the player's stops say so; "hold priority" supported).
- **Event-sourced + seeded RNG**: state = fold(events). Gives deterministic replay, reconnect, persistence, undo-for-friends, and trivial test reproduction (every failure is a replayable seed).
- **Characteristics are derived, never mutated**: base + continuous effects computed through the **layer system (CR 613)** with timestamps and dependencies. All buffs/debuffs/type changes/counters/auras/"until end of turn" go through one `ContinuousEffect { layer, selector, modification, duration, timestamp }` path.
- Replacement/prevention effects (CR 614/615), triggered abilities (603, incl. delayed/reflexive), static/activated/mana abilities, state-based actions to fixpoint (704), stack, APNAP ordering, combat (first strike, double strike, trample, deathtouch, menace, etc.), targeting re-validation on resolution.
- **Keywords defined once** in the engine and referenced by cards, so one test covers every card with that keyword.
- **Formats as a trait** (`FormatRules`): deck validation, starting life, mulligan, extra zones/rules. `Normal` (60 cards, 20 life, London mulligan) and `Commander` (100 singleton, colour identity, command zone, tax, 40 life, 21 commander damage, partner/companion later). Any player count; attack any opponent/planeswalker; elimination and leaving-the-game handled.
- **Card DSL**: declarative data (RON/JSON stored in repo + DB), composed of `Cost`, `Selector`, `Condition`, `Effect`, `Duration`, `TargetSpec` primitives. Each card carries `impl_status: Full | Partial | Manual`.
- **Manual fallback**: `Manual` cards show oracle text and expose generic logged actions (move card, counters, life, create token, tap, etc.) with other players able to see/object. Never blocks a deck from being played.
- **Oracle-text compiler** (later milestone): template/pattern-based parser compiling common wordings to the DSL; unparsed → `Manual`. Coverage report ranks unimplemented cards by frequency across imported decks/precons so effort goes where it matters.

### Data & import pipeline
- **Scryfall bulk data** (Oracle Cards + Default Cards + Rulings; never per-card scraping; respect User-Agent/Accept headers and rate limits). Images via Scryfall CDN, optional local cache later.
- **MTGJSON** for official precon deck lists (Scryfall has no deck lists).
- **Comprehensive Rules** text vendored for test-case references (`CR 702.7` etc. in test docs).
- Deck import parsers: plain text (`4 Lightning Bolt`, with set/collector variants, sideboard/commander sections), MTGO `.dek`, Arena export, Moxfield/Archidekt export text; exporters for round-trip. Name resolution with fuzzy suggestions and per-line error reporting.
- Idempotent, resumable import job (`import_runs` table), runnable via CLI and scheduled in-cluster.
- Postgres schema (sqlx migrations, compile-time-checked queries): `cards` (oracle_id, normalized columns + `jsonb`), `printings`, `rulings`, `precon_decks`, `users`, `decks`, `deck_cards`, `games`, `game_events`, `game_players`, `import_runs`. Search via `pg_trgm` + `tsvector`; colour/type/cmc filters indexed.
- Auth: username + password (argon2), session cookie; invite-link lobbies. OIDC later if wanted.

### UX principles
- One source of truth for "what can I do": the engine's legal options.
- Clear state banner: "Your priority", "Waiting for X", "No attackers available".
- Targeting mode dims illegal objects; stack panel always visible; full game log; card hover zoom.
- Fast N-player layout (focus opponent + compact others), commander damage/tax shown on seats.

## Testing strategy (every test states its Goal)
Convention: each test module opens with a one-line `Goal:` doc; prefer table-driven/parametrized tests (rstest) over single-card tests. Tests assert **mechanics**, not individual cards.
1. **Unit**: mana/costs, selectors, layer ordering, dependency resolution, deck validation, parsers.
2. **Mechanic conformance**: synthetic cards built in `mtg-testkit` exercise each keyword/rule once (e.g. first strike covers all first-strikers), tagged with CR section numbers.
3. **Corpus tests**: every implemented card loads, validates, and its keywords match its oracle text; oracle-parser golden files; **coverage ratchet** (implemented-card count may not drop).
4. **Property/invariant tests** (proptest): card conservation across zones, object in exactly one zone, SBA fixpoint reached, replay(events)==live state hash, no stuck state (a legal action always exists or game ended).
5. **Fuzz games**: random-legal-action bots play thousands of seeded N-player games in both formats; failures print the seed.
6. **Scenario/golden tests**: scenario DSL (setup board → script → assert), seeded from Scryfall rulings for complicated interactions.
7. **Import tests**: recorded fixtures (no live network), round-trip `parse(export(d))==d`, malformed-input cases.
8. **Server integration**: real Postgres (testcontainers / `sqlx::test`), WebSocket clients, bot-vs-bot full games, reconnect and restart-resume.
9. **Frontend**: Vitest + Testing Library for components/stores; **Playwright E2E** against full stack (build deck, create lobby, play turns with highlighting/pass flow).
10. **CI** (GitHub Actions or self-hosted): `cargo fmt --check`, `clippy -D warnings`, nextest, `cargo-deny`, TS typecheck/lint, coverage reporting. Rule: no change merges without tests justifying it.

## Code standards (written into project `CLAUDE.md`)
- `#![forbid(unsafe_code)]`, clippy pedantic (curated allowlist), `thiserror` in libraries, no `unwrap`/`expect` outside tests, newtype IDs, small modules with single responsibility, no cross-layer imports (engine never touches DB/network).
- Docs on every public item: one short line. Inline comments only for non-obvious *why*. No long design essays in code.
- TS: strict mode, no `any`, feature-folder structure, ESLint + Prettier, generated protocol types only.
- No Claude attribution in commits/PRs (user's global preference).

## Deployment (k3s)
Multi-stage Docker image (Rust server + built SPA served by it or via nginx), Kustomize/Helm manifests, Postgres via CloudNativePG (or a simple StatefulSet to start), k3s Traefik ingress + cert-manager TLS on the user's domain (WebSocket-capable). Games live in-memory per game actor with the event log persisted, so a single replica can restart and resume; scale-out deferred.
Notes: private non-commercial friends use fits Wizards' Fan Content Policy; keep card art attributed and avoid any monetization.

## Milestones (vertical slices, each with its tests)
- **M0 Foundations**: workspace, CI, `CLAUDE.md`, lint/test tooling, Postgres dev setup (docker compose), **frontend spike** with pass criteria above.
- **M1 Card data**: schema + migrations, Scryfall/MTGJSON import, card search API, deck parsers/validation, precon listing.
- **M2 Engine core + first playable**: zones, turns/phases, priority, stack, mana, SBA, lands/spells, basic combat, testkit; 2-player vanilla game playable in browser with highlight/pass UX.
- **M3 Rules depth**: all keywords, layers/continuous effects, triggers, replacement effects, targeting, counters, auras/equipment, tokens.
- **M4 Multiplayer product**: auth, lobbies, N-player, reconnect, deck builder UI, precon browser, game log/stack UI.
- **M5 Commander**: format rules, command zone, tax, commander damage, partner; Commander-specific tests.
- **M6 Coverage**: manual fallback mode, oracle-text compiler, coverage dashboard; target: 100% of a chosen set of precons, then expand by frequency.
- **M7 Polish**: animations, stop settings, spectators, replays, bots for solo play, deployment hardening.

## Verification (end-to-end, per milestone)
- `cargo nextest run --workspace` and `cargo clippy --workspace -- -D warnings` green; fuzz-game suite passes with fixed seeds.
- `docker compose up` brings up Postgres + server + web; import job loads cards; Playwright E2E plays a full 2- and 4-player game (Normal and Commander) with a precon and an imported custom deck.
- Manual check of Arena-style UX: no-attackers message, instant highlighting, explicit pass.
- Deploy manifests applied to k3s; WebSocket game works through the domain with TLS.

## First steps after approval
Scaffold the workspace + `CLAUDE.md` + CI + docker-compose Postgres, then run the M0 frontend spike and report results before locking the stack.
