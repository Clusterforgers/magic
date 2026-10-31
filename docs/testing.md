# Testing speed

How the checks run, where the time of a check and a release goes, and the design that tests only what a change can break.

## Commands

| Command | Who | Runs |
| --- | --- | --- |
| `scripts/check.sh gate` | agents, every commit | fmt, clippy of the changed crates, `cargo check` of their dependents, the script tests, the tests of the changed crates without the slow list, the web checks when `web/` changed |
| `scripts/check.sh affected` | orchestrator, once per merged batch | fmt, clippy, the script tests, every test of the changed crates and their dependents (slow list included), the web checks the diff needs |
| `scripts/check.sh all` | orchestrator, before a tag | everything, the whole web check included |
| `scripts/check.sh fast` | commit hook | fmt, clippy, the script tests |

- A diff is taken against the merge-base with `origin/main` (`gate BASE` and `affected BASE` name another); `--list` prints the selection and exits. A change to the check scripts alone selects no crate in the gate and every crate in `affected`.
- The slow list `scripts/slow-tests.txt` names the tests over 5 s (substrings of the test path, one per line, a case table by its function). The gate passes them to the test binaries as `--skip`; `affected` and `all` run them. A test over 5 s that ran and is not on the list is printed with its seconds at the end of the run.
- The test build is one `cargo test --no-run` of the selected crates. Each test binary then runs as its own process from its crate folder with the environment cargo gives a test (`CARGO_MANIFEST_DIR`, `CARGO_PKG_*`, `CARGO_BIN_EXE_*`) and with `--report-time`, so the feature set of the build is the one tested. A crate with doc tests also runs `cargo test --doc`.
- Test binaries and the web checks run side by side within a thread budget: the cores (`MTG_TEST_THREADS`, default `nproc`) divided by the checks running, at least 4. The biggest binary starts first with a third of the free threads, the next with a third of the rest, down to 2 (`RUST_TEST_THREADS` fixes the count); a binary starts when its threads are free. The web checks start first and run while the Rust phases compile.
- The binaries of the Postgres crates (`mtg-db`, `mtg-server`, `mtg-cli`) run one at a time under a machine-wide file lock, because `sqlx::test` drops the test databases of other runs.
- The first failing binary kills the rest. The run ends with the failure section of that binary and the names of the failing tests; a binary over `MTG_TEST_TIMEOUT` minutes (40) is killed and the tests that ran over 60 s are named.
- Every phase result is a stamp in `~/.cache/mtg-check/stamps`, keyed by content: the sources of the crate and of its workspace dependencies, `Cargo.lock`, the toolchain, `.cargo`, `clippy.toml`, the Postgres crates' `.sqlx`, and for the tests of `mtg-protocol` the fixtures and bindings in `web/src/protocol`; the web key hashes `web/`, the lock files and the node version. Docs and unrelated crates do not change a key, so a merge reruns only the crates whose key changed, worktrees with equal content share stamps, and a checked commit needs no new check for a release. The working tree is hashed with a temporary index (tracked and untracked files, not ignored ones); `--force` reruns. A gate pass leaves a `gate-test` stamp that `affected` does not accept.

## Measurements

24 cores, `CARGO_BUILD_JOBS=4`, load average 3 to 19 (other agents running).

| Step | Seconds |
| --- | --- |
| Cold test build of the workspace (new worktree) | 234 to 328 |
| Warm rebuild after a card file or an engine file changes | 20 to 39 |
| Warm rebuild of the `mtg-testkit` `it` binary after one test file changes | 136 |
| fmt / clippy (warm) / script tests | 3 / 59 / 2 to 20 |
| Tests without Postgres, one `cargo test` run (binaries one after another) | 1,211 to 1,242 |
| Tests with Postgres (under the global lock) | 520 to 738 |
| Web (typecheck, lint, format, vitest) | 106 to 121 |
| Release images (server and web, one after the other, after the check) | about 600 |

Per binary (`cargo test` binaries, 6 threads, load 10 to 17): `mtg-testkit::it` 925 s for 17,517 tests, `mtg-protocol::it` 337 s for 1,465, `mtg-cards::it` 96 s for 770, every other binary under 11 s. Test-seconds per binary with one process per test (nextest): `mtg-testkit::it` 18,517, `mtg-protocol::it` 1,650, `mtg-cards::it` 504, all others under 160.

- Each test process loads the built-in library once (`OnceLock`, about 1.1 s), so one process per test (nextest) costs 19,000 test-seconds of loading; libtest in one process pays it once.
- The long tail decides the wall clock: 329 tests over 5 s take 6,000 s of test time, 489 fuzz tests take 2,337 s. Slowest: `hidden_info` (protocol, 1,050 s, one case 256 s), `dk1e_deck` (631 s), `house_commander` (468 s), `free_loops`, `replacement_budget`, `manifest_view`, `foretell_view`, `filter_lands`.
- Every card file lives in `mtg-cards`, and every crate except `mtg-core`, `mtg-import` and `mtg-db` depends on it, so `affected` for one card or engine change selects nearly the whole workspace; the gate tests the changed crate only. A protocol change selects `mtg-protocol`, `mtg-server` and the whole web check.

## Design

- Three levels. The gate (agents, every commit), the batch run (orchestrator, once per merged batch: every test of `affected`, slow list included) and the tag run (before a release: everything with the long fuzz counts).
- A change only to card files (`crates/mtg-cards/cards/`) tests those cards, the test modules and fixture decks that name them and a fixed sample; the batch run tests the whole corpus.
- The release images build while the batch run of the release commit runs, both at once; `scripts/release.sh` finds them in the build cache and only pushes and rolls out.
- A new worktree starts from a copy of the warm `target` of its base commit instead of a cold build.
- The `mtg-testkit` `it` binary splits into binaries by family, so one test edit rebuilds only its family and the families run side by side.
- Each Postgres test run gets its own database prefix, so the global lock goes.
- The built-in library loads from a snapshot made at build time instead of 1.1 s of parsing per process.
