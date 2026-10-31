#!/usr/bin/env bash
# Goal: the gate tests only the crates a diff changes and checks their dependents; the slow list, the thread budget and
# the units made from the artifacts of a cargo build do what docs/testing.md says.
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=probe.bash
source scripts/tests/probe.bash
export TMPDIR="$scratch"
# shellcheck source=../lib/check-plan.sh
source scripts/lib/check-plan.sh
# shellcheck source=../lib/check-run.sh
source scripts/lib/check-run.sh
# shellcheck source=../lib/check-units.sh
source scripts/lib/check-units.sh
declare -A secs=()
load_graph

# --- Selection -----------------------------------------------------------------------------------------------------

# gate_case <name> <without Postgres> <with Postgres> <check> <web> -- <changed path...>; clippy follows the tests.
gate_case() {
  local name="$1" other="$2" pg="$3" chk="$4" web="$5" listing clippy
  shift 6
  listing=$(scripts/check.sh gate --list "$head" "$(probe_commit "$@")")
  clippy=$(echo "$other $pg" | tr ' ' '\n' | { grep -v '^-$' || true; } | sort | tr '\n' ' ' | sed 's/ $//')
  [[ -n "$clippy" ]] || clippy=-
  check "gate $name" "$(field "$listing" 'tests without Postgres')|$(field "$listing" 'tests with Postgres')|$(field "$listing" check)|$(field "$listing" web)|$(field "$listing" clippy)" \
    "$other|$pg|$chk|$web|$clippy"
}

gate_case "docs" - - - none -- docs/ROADMAP.md
gate_case "check scripts" - - - none -- scripts/check.sh scripts/lib/check-run.sh scripts/slow-tests.txt
gate_case "engine" mtg-engine - "mtg-cli mtg-protocol mtg-server mtg-testkit" none -- crates/mtg-engine/src/lib.rs
gate_case "cards" mtg-cards - "mtg-cli mtg-engine mtg-protocol mtg-server mtg-testkit" none -- crates/mtg-cards/cards/a.json
gate_case "core" mtg-core - "mtg-cards mtg-cli mtg-db mtg-engine mtg-import mtg-protocol mtg-server mtg-testkit" none \
  -- crates/mtg-core/src/lib.rs
gate_case "protocol" mtg-protocol - mtg-server full -- crates/mtg-protocol/src/lib.rs
gate_case "database" - mtg-db "mtg-cli mtg-server" none -- crates/mtg-db/src/lib.rs
gate_case "leaf with Postgres" - mtg-cli - none -- crates/mtg-cli/src/main.rs
gate_case "sqlx metadata" - mtg-db "mtg-cli mtg-server" none -- .sqlx/query-probe.json
gate_case "two crates" mtg-engine mtg-db "mtg-cli mtg-protocol mtg-server mtg-testkit" none \
  -- crates/mtg-engine/src/lib.rs crates/mtg-db/src/lib.rs
gate_case "web" - - - changed -- web/src/lib/flash.ts
gate_case "root lock" "mtg-cards mtg-core mtg-engine mtg-import mtg-protocol mtg-testkit" "mtg-cli mtg-db mtg-server" - full -- Cargo.lock

# --- Slow list -----------------------------------------------------------------------------------------------------

slow_file="$scratch/slow.txt"
printf '# header\n\nalpha::beta  # why\n  gamma \n' >"$slow_file"
check "slow entries" "$(slow_entries | tr '\n' '|')" "alpha::beta|gamma|"
check "skip arguments" "$(slow_skip_args | tr '\n' ' ')" "--skip alpha::beta --skip gamma "
: >"$slow_file"
check "empty list" "$(slow_skip_args | wc -l)" 0
printf 'listed::\n' >"$slow_file"
cat >"$scratch/run.log" <<'EOF'
running 5 tests
test a::slow_unlisted ... ok <6.5s>
test a::fast ... ok <0.2s>
test listed::slow ... ok <30.0s>
test a::at_the_limit ... ok <5.0s>
test a::broken ... FAILED <0.1s>
test a::plain ... FAILED
EOF
check "over the limit" "$(slow_warnings lbl "$scratch/run.log")" "  lbl: a::slow_unlisted 6.5s"
check "failed names" "$(failed_tests "$scratch/run.log" | tr '\n' ' ')" "a::broken a::plain "

# --- Thread budget -------------------------------------------------------------------------------------------------

# cores, checks running -> share (at least 4)
for row in "24 1 24" "24 2 12" "24 3 8" "24 8 4" "24 0 24" "4 1 4" "96 1 96" "12 1 12"; do
  read -r cores runs share <<<"$row"
  check "share of $cores cores, $runs checks" "$(thread_share "$cores" "$runs")" "$share"
done
# free threads, binaries waiting -> threads of the next binary: a third while three wait, between 2 and 12
for row in "24 18 8" "16 17 5" "11 16 3" "8 15 2" "20 4 6" "24 3 8" "24 2 12" "6 2 3" "24 1 12" "100 1 12" "1 5 2" "5 0 5"; do
  read -r free waiting threads <<<"$row"
  check "threads with $free free and $waiting waiting" "$(unit_threads "$free" "$waiting")" "$threads"
done

lease_dir="$scratch/leases"
mkdir -p "$lease_dir"
touch "$lease_dir/run.dead"
flock "$lease_dir/run.live" sleep 4 &
sleep 0.3
check "leases: one live check" "$(running_checks)" 1
check "leases: a dead one is removed" "$([[ -e "$lease_dir/run.dead" ]] && echo kept || echo removed)" removed
check "leases: this check counts" "$(
  take_lease
  running_checks
)" 2
kill %1 2>/dev/null || true

# --- Units from cargo's artifacts ----------------------------------------------------------------------------------

root=$(pwd)
head -c 2000 /dev/zero >"$scratch/big"
head -c 100 /dev/zero >"$scratch/small"
head -c 300 /dev/zero >"$scratch/bin"
artifact() { # <test|bin> <crate> <target> <kind> <file>
  jq -nc --arg t "$1" --arg c "$root/crates/$2" --arg n "$3" --arg k "$4" --arg e "$scratch/$5" \
    '{reason: "compiler-artifact", executable: $e, profile: {test: ($t == "test")}, manifest_path: ($c + "/Cargo.toml"),
      target: {name: $n, kind: [$k]}, package_id: ("path+file://" + $c + "#0.1.0")}'
}
{
  artifact test mtg-core mtg_core lib small
  artifact test mtg-cli it test big
  artifact bin mtg-cli mtg-cli bin bin
  artifact test mtg-cli mtg_cli lib bin
  echo '{"reason": "build-script-executed"}'
} >"$scratch/build.json"
run_dir="$scratch"
add_test_units "$scratch/build.json" --skip slow::one
check "unit order" "${queue[*]}" "mtg-cli::it mtg-cli::lib mtg-core::lib"
check "unit kinds" "${u_kind['mtg-cli::it']} ${u_kind['mtg-core::lib']}" "pg tests"
check "unit directory" "${u_dir['mtg-core::lib']}" "$root/crates/mtg-core"
check "unit command" "$(tr '\n' ' ' <<<"${u_cmd['mtg-core::lib']}")" "$scratch/small -Z unstable-options --report-time --skip slow::one "
check "unit environment" "$(grep -c '^CARGO_MANIFEST_DIR=\|^CARGO_PKG_NAME=mtg-cli\|^CARGO_BIN_EXE_mtg-cli=' <<<"${u_env['mtg-cli::it']}")" 3
check "crate unit count" "${crate_left[mtg-cli]} ${crate_left[mtg-core]}" "2 1"

finish check_gate
