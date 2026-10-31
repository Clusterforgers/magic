#!/usr/bin/env bash
# Goal: `scripts/check.sh affected --list` picks the phases a diff can break, and a content stamp skips a crate.
# Each case commits a probe change on top of HEAD into a throwaway object store and lists the diff of the two commits.
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=probe.bash
source scripts/tests/probe.bash

# expect <name> <listing> <without Postgres> <with Postgres> <web>
expect() {
  check "$1" "$(field "$2" "tests without Postgres")|$(field "$2" "tests with Postgres")|$(field "$2" "web")" "$3|$4|$5"
}

# case_diff <name> <without Postgres> <with Postgres> <web> -- <changed path...>
case_diff() {
  local name="$1" other="$2" pg="$3" web="$4" commit
  shift 5
  commit=$(probe_commit "$@")
  expect "$name" "$(scripts/check.sh affected --list "$head" "$commit")" "$other" "$pg" "$web"
}

all_other="mtg-cards mtg-core mtg-engine mtg-import mtg-protocol mtg-testkit"
all_pg="mtg-cli mtg-db mtg-server"
case_diff "only docs" - - none -- docs/ROADMAP.md CLAUDE.md
case_diff "other scripts" - - none -- scripts/agent-status.sh
case_diff "mtg-cards" "mtg-cards mtg-engine mtg-protocol mtg-testkit" "mtg-cli mtg-server" none -- crates/mtg-cards/src/lib.rs
case_diff "mtg-core" "$all_other" "$all_pg" none -- crates/mtg-core/src/lib.rs
case_diff "only web" - - changed -- web/src/lib/flash.ts
case_diff "web config" - - full -- web/src/lib/flash.ts web/package.json
case_diff "bindings" - - full -- web/src/protocol/generated/Rule.ts
case_diff "mtg-protocol" "mtg-protocol" "mtg-server" full -- crates/mtg-protocol/src/lib.rs
case_diff "Cargo.lock" "$all_other" "$all_pg" full -- Cargo.lock
case_diff "check.sh" "$all_other" "$all_pg" full -- scripts/check.sh
case_diff "check lib" "$all_other" "$all_pg" full -- scripts/lib/check-plan.sh
case_diff "slow list" - - none -- scripts/slow-tests.txt
case_diff "mtg-db" - "$all_pg" none -- crates/mtg-db/src/lib.rs
case_diff "migration" - "$all_pg" none -- crates/mtg-db/migrations/9999_probe.sql
case_diff ".sqlx" - "$all_pg" none -- .sqlx/query-probe.json
case_diff "web and mtg-cli" - "mtg-cli" changed -- web/src/lib/flash.ts crates/mtg-cli/src/main.rs

# A stamp for the content of the working tree skips those crates: the diff runs from a probe base back to the tree.
# shellcheck source=../lib/check-plan.sh
source scripts/lib/check-plan.sh
load_graph
snap=$(snapshot_tree)
probe_tree="$snap"
: >"$MTG_CHECK_STAMPS"
for crate in mtg-db mtg-server; do
  stamp_line "$snap" test "$crate"
  echo "$REPLY" >>"$MTG_CHECK_STAMPS"
done
expect "stamped" "$(scripts/check.sh affected --list "$(probe_commit crates/mtg-db/src/lib.rs)")" - mtg-cli none
expect "forced" "$(scripts/check.sh affected --list --force "$(probe_commit crates/mtg-db/src/lib.rs)")" - "$all_pg" none

finish check_select
