# Magic web platform

Browser-hosted Magic: The Gathering for friends. Rust backend + engine, React/TypeScript client, Postgres. Design: `docs/PLAN.md`. Progress: `docs/ROADMAP.md` (continue with the next unchecked item). Agent briefs: `docs/agents.md`. Design notes: `docs/engine.md`, `docs/protocol.md`, `docs/server.md`, `docs/web.md`.

## Layout
- `crates/mtg-core` shared value types · `mtg-engine` rules engine (no IO) · `mtg-cards` card DSL · `mtg-import` Scryfall/MTGJSON/deck parsing · `mtg-db` Postgres schema + repositories · `mtg-protocol` wire types · `mtg-server` axum + sqlx · `mtg-testkit` scenarios/invariants
- `web/` React + Vite + Tailwind + shadcn/ui + TanStack Router client; live game state in a zustand store keyed by object id

## Rules of the house
- The engine never does IO and never imports server, DB or network crates.
- Characteristics are derived from base values plus continuous effects (layers); never mutate them directly.
- Keywords and mechanics are implemented once in the engine; cards only reference them.
- No `unwrap`/`expect` outside tests; no `unsafe`; errors via `thiserror`.
- Docs: one short line on every public item. Inline comments only for a non-obvious why.
- Design notes and the roadmap: short and professional. Present tense and facts only: no history, dates, who asked, hand-off notes or uncertainty lists (open issues become roadmap items). Each file opens with one line of purpose. One line per roadmap item (the outcome); the design lives in `docs/*.md`.
- Tests: each test module opens with a `//! Goal:` line. Test mechanics across many cases (rstest tables, proptest), not single cards.
- Every change ships with tests that justify it.

## Commands
- `scripts/check.sh [fast|rust|web|all]` runs fmt, clippy, tests (fail-fast); `scripts/setup.sh` enables the git hooks once.
- Raw: `cargo fmt --all` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo test --workspace`
- `docker compose up -d db` starts local Postgres; DB tests use `sqlx::test`, and `sqlx::query!` is checked offline against committed `.sqlx/` (refresh steps in `.cargo/config.toml`).
- Wire types: `MTG_UPDATE_BINDINGS=1 cargo test -p mtg-protocol --test bindings` regenerates `web/src/protocol/generated/`; the drift test fails while they are stale.
- `node scripts/ui-shot.mjs <url> <out-prefix> [--widths 1400,390] [--wait <js>]` screenshots a rendered page with headless Firefox (no Chromium here); open the PNG with the Read tool.
- Web (in `web/`): `pnpm typecheck` · `pnpm lint` · `pnpm format` · `pnpm test` · `pnpm e2e`. On NixOS set `CHROMIUM_PATH` to a Nix-built chromium binary for e2e.

## Workflow
- Keep context small: short tool output (`-q`, `tail`, `grep`), never re-read files already seen, delegate big work to agents with short briefs (`docs/agents.md`), suggest `/compact` between roadmap items.
- Delegate in pieces of about 15 minutes; agents commit after every finished module (`wip:` prefix for unfinished checkpoints).
- Tick the roadmap item in the commit that finishes it. After a restart, run `git worktree list` and read each worktree's status before resuming work.
- Review agent output against these rules and run `scripts/check.sh` on the merged result before accepting it.

## Git
Never add Claude as co-author or any attribution line to commits or PR descriptions.
The `commit-msg` hook rejects attribution lines and runs fmt + clippy unless the message starts with `wip:`.
