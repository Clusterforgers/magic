# Agent briefs

Template for delegating a task to an agent, and the rules the orchestrator follows. The rules an agent follows live in `CLAUDE.md`.

## Brief template

Use it for every delegated task and keep the brief itself short.

```
Task: <one roadmap item, one focused piece, about 15 minutes; at about 80 model turns commit and report what is left>
Read first: CLAUDE.md, docs/ROADMAP.md, <only the files you need>
Scope: only <crate or folder>; no edits elsewhere except append-only workspace deps.
Deliver: <list>
Tests: each test module starts with `//! Goal:`; table-driven/property tests over single cases.
Docs you write follow the docs rule in CLAUDE.md.
Checks before every non-wip commit: scripts/check.sh fast; before the final report: `git merge main`, then scripts/check.sh affected. If main moved while it ran and the merge is clean, re-run only scripts/check.sh fast and `cargo test -p <crate>` of the crates you touched; run `affected` again only if the merge had conflicts in code. Once the check is green do not merge main again: it moves with every merge and push, and the orchestrator merges it into your branch and resolves the roadmap conflicts.
Context: CARGO_BUILD_JOBS=4 (six cargo jobs share 24 cores); read line ranges (`grep -n`, `sed -n a,bp`) and each file once; run a check that takes more than a few minutes in the background and end the turn instead of polling; never wait in an `until` loop on `pgrep -f` (the pattern matches the loop's own command line, so it never ends and the agent never reports), wait on the command's own exit or on an end marker in its log.
Commits: commit after every finished module or green run. Prefix unfinished checkpoints with `wip:`. Never add AI attribution.
Last step: after the final green `scripts/check.sh affected`, run `rm -rf target` in your worktree.
Report (max 15 lines): files added, public API in one line each, test counts, decisions, uncertainties.
```

## Orchestrator rules

- Isolate each agent in a git worktree; merge only after a green check of the merged result: `scripts/check.sh all` for a batch that touches `web/` or the protocol, `scripts/check.sh affected` for the rest.
- `scripts/check.sh affected [BASE]` diffs against BASE (default: the merge-base with `origin/main`) and always runs fmt, clippy and the script tests; it tests the changed crates and their dependents (`cargo metadata`), the Postgres crates only when one of them, `.sqlx` or a crate they depend on changed, the whole web check for the protocol, the bindings or web config, and only the vitest files related to the diff (`vitest --changed`) for other web changes. A root file (`Cargo.toml`, `Cargo.lock`, the toolchain, `.cargo`, `check.sh`) selects everything; `--list` prints the selection. A phase or crate that passed on a clean tree leaves a stamp (tree hash, toolchain, phase) in `<git common dir>/mtg-check-stamps`, so the same tree skips it in every worktree until `--force`.
- Test runner: `cargo test`; cargo-nextest 0.9.146 is not faster on `mtg-cards` and `mtg-engine` (15,508 tests, warm build, 6 threads, load 25 to 45): `cargo test` 62 s and 276 s, nextest 186 s and 159 s wall in two alternating rounds.
- Size a piece to at most about 80 model turns. Every turn re-reads the whole context, so the cost of a session grows with the square of its length, and an idle gap of more than five minutes writes the whole context to the cache again.
- Never chain checks through pipes without `set -o pipefail`; a lint failure must stop the commit.
- Validate parsers and importers against real data before calling them done.
- Keep the root Postgres running (`docker compose up -d db`) while agents work: their `check.sh` reuses it instead of starting a compose project of its own. The script compiles the tests and runs the crates without Postgres outside the file lock and holds the lock only for the `mtg-db`, `mtg-server` and `mtg-cli` tests, because `sqlx::test` names its test databases after the test path and drops those of other runs; a database per worktree does not isolate runs. It ends with the seconds of each phase and the lock wait. Remove a stray `agent-*` project with `docker compose -p <name> down -v`.
- `scripts/check.sh` runs at most `MTG_TEST_SLOTS` (4) test runs at once machine-wide (`flock` on `/tmp/mtg-test-slot.N`; the wait is the `slot wait` line) with `RUST_TEST_THREADS` (6) threads each, so 4 x 6 fills 24 cores, and kills a run after `MTG_TEST_TIMEOUT` (40) minutes, naming the tests that ran over 60 seconds; compiles use `CARGO_BUILD_JOBS` (4). On an idle machine: `CARGO_BUILD_JOBS=24 MTG_TEST_SLOTS=1 RUST_TEST_THREADS=24 scripts/check.sh`. The seeded campaigns of `mtg-testkit` play a small default number of games; the long run is `MTG_FUZZ_GAMES=500 cargo test -p mtg-testkit --test it` (`docs/engine.md`, Verification).
- Never `cd` into an agent worktree from the orchestrator shell: the harness then makes it the primary directory and forbids returning to the root. Use `git -C <path>` and absolute paths, and merge with `--no-stat` to keep output short.
- After a restart, run `git worktree list` and read each worktree's `git status` before resuming or redoing anything.
- When the load stays high or a check does not finish, run `scripts/agent-status.sh`: it shows the CPU by worktree, test binaries running for over 15 minutes with the threads that burn CPU (a hung game), processes whose worktree is gone, wait loops that match themselves and the scratch databases. Tell the agent of a hung test to kill the run and find the seed.
- Run as many agents at once as are certain not to affect each other: different crates, modules, roadmap items, e2e ports and scratch databases, never the same file or decision. Structure items that reshape shared code run alone.
- Disk: every worktree builds its own `target/` (tens of GB each). After merging an agent, remove its worktree and branch (`git worktree remove`, then `git branch -d`); an agent deletes its own `target/` in its last step. Keep one agent worktree per running agent.
- Push `main` to origin after every merge and before launching agents: an `isolation: worktree` agent branches from `origin/main`, and agents run elsewhere see a claimed piece (`[~]` in the roadmap) only once it is pushed.
- Run about six agents at once at most (24 cores): each runs one cargo job at a time and ends with `git merge main` and `scripts/check.sh affected` (after a clean merge of a `main` that moved meanwhile, only the fast check and the touched crates' tests); the orchestrator fast-forwards `main` to its tip when `main` is an ancestor and otherwise sends the agent back to merge.
- Dependencies stay current: after a round, with no agent running, `cargo update --dry-run` lists what is behind; the (dep) piece updates it alone because `Cargo.lock` changes rebuild every worktree.
- Coverage pieces measure the Verified counts in a scratch database (`ingest`, `compile`, `coverage` with `--database-url`), never in `magic`; the numbers go to the M6 target line.
- Corpus fixtures (`crates/mtg-cards/tests/fixtures/corpus/*.json`) are the trimmed Scryfall record of the card from the cache (`~/.cache/mtg-cli/scryfall`, `zcat ... | jq`): the id, oracle id, set, collector number and release date are real, never written from memory.
- A change to a wire type regenerates the bindings and can break the web: `check.sh affected` then runs the whole web check (`check.sh rust` skips the web). Counts kept in code (`TEMPLATES: [Template; N]`, the effect sample count in `resolve/prim/mod.rs`, the keyword name lists) are recounted at every merge of branches that each added one.
- A conflict in a list (keyword names, test modules, `reproduced.txt`) keeps both sides; `reproduced.txt` is re-sorted with `LC_ALL=C sort -u` and `cargo fmt` follows. The check runs again on the merged tree after any conflict and after `main` moved.
- A new enum variant with a payload as large as the largest one grows the enum and can break a size test (`Event` is limited to 144 bytes): box the payload.
- A check that fails with `E0514` (a crate compiled by another rustc) ran across a toolchain update: run it again. After an update, run `docker compose up -d db` and `scripts/agent-status.sh` before launching agents.
- Edit roadmap lines by their id, never by a line number held in a shell variable that can be empty.

### Parallel agents

- Land a foundation task first, with seams so each later agent replaces only its own file (and its own docs bullet, with blank lines around it) and not a shared registry.
- Give each agent a distinct dev-server port.
- Merge the agents one at a time and run the checks after each. Message the agents still running to merge `main` into their branch before they report, so they resolve their own conflicts.
