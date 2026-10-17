# Magic web platform

Browser-hosted Magic: The Gathering for friends. Rust backend + engine, React/TypeScript client, Postgres. Full plan: `docs/PLAN.md`.

## Layout
- `crates/mtg-core` shared value types · `mtg-engine` rules engine (no IO) · `mtg-cards` card DSL · `mtg-import` Scryfall/MTGJSON/deck parsing · `mtg-protocol` wire types · `mtg-server` axum + sqlx · `mtg-testkit` scenarios/invariants
- `web/` React + Vite + Tailwind + shadcn/ui + TanStack Router client; live game state in a zustand store keyed by object id

## Rules of the house
- The engine never does IO and never imports server, DB or network crates.
- Characteristics are derived from base values plus continuous effects (layers); never mutate them directly.
- Keywords and mechanics are implemented once in the engine; cards only reference them.
- No `unwrap`/`expect` outside tests; no `unsafe`; errors via `thiserror`.
- Docs: one short line on every public item. Inline comments only for a non-obvious why.
- Tests: each test module opens with a `//! Goal:` line. Test mechanics across many cases (rstest tables, proptest), not single cards.
- Every change ships with tests that justify it.

## Commands
- `cargo fmt --all` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo test --workspace`
- `docker compose up -d db` starts local Postgres.
- Web (in `web/`): `pnpm typecheck` · `pnpm lint` · `pnpm format` · `pnpm test` · `pnpm e2e`. On NixOS set `CHROMIUM_PATH` to a Nix-built chromium binary for e2e.

## Git
Never add Claude as co-author or any attribution line to commits or PR descriptions.
