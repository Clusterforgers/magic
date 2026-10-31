#!/usr/bin/env bash
# shellcheck disable=SC2016 # the scheduler cases hand code to `eval` in single quotes
# Goal: the gate tests only the crates a diff changes and checks their dependents; the slow list, the thread budget and
# the unit scheduler (side by side within the budget, fail-fast, short output) do what docs/testing.md says.
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
test a::broken ... FAILED
EOF
check "over the limit" "$(slow_warnings lbl "$scratch/run.log")" "  lbl: a::slow_unlisted 6.5s"
check "failed names" "$(failed_tests "$scratch/run.log")" "a::broken"

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

# --- Scheduler -----------------------------------------------------------------------------------------------------

lease_dir="$scratch/leases2"
pg_lock="$scratch/pg.lock"
export MTG_TEST_THREADS=8 RUST_TEST_THREADS=2

# sched <script>: run the units a script adds in a subshell; sets out, rc and elapsed seconds.
sched() {
  local start=$SECONDS
  rc=0
  out=$(
    queue=() running=()
    run_dir=""
    init_units
    eval "$1"
    run_units
  ) 2>&1 || rc=$?
  elapsed=$((SECONDS - start))
}

# unit <name> <kind> <shell>: add a unit that runs a shell snippet.
unit() { add_unit "$1" "$2" "$scratch" auto "" "$(printf '%s\n' sh -c "$3")"; }

# A shell snippet that logs "<name> s" and "<name> e" around its work: the log shows what ran at once.
work() { echo "echo $1 s >>$scratch/events; sleep $2; echo $1 e >>$scratch/events"; }
# peak: the most units that were running at once, from the events.
peak() { awk '$2 == "s" { n++; if (n > m) m = n } $2 == "e" { n-- } END { print m + 0 }' "$scratch/events"; }

: >"$scratch/events"
sched 'for i in 1 2 3 4 5 6; do unit u$i tests "$(work u$i 0.6)"; done'
check "budget: all ok" "$rc" 0
check "budget: share 8 of 2 threads runs four at once" "$(peak)" 4
check "budget: output" "$(grep -c '^\[tests\] u. ok (' <<<"$out")" 6

: >"$scratch/events"
MTG_TEST_THREADS=4 sched 'for i in 1 2 3 4; do unit u$i tests "$(work u$i 0.5)"; done'
check "budget: share 4 runs two at once" "$(peak)" 2

# Without a fixed thread count the first binary takes a third of the threads and the later ones less.
thr() { echo "echo \$RUST_TEST_THREADS >$scratch/thr.$1; sleep 0.4"; }
MTG_TEST_THREADS=24 sched 'unset RUST_TEST_THREADS; for i in 1 2 3 4 5 6 7; do unit u$i tests "$(thr u$i)"; done'
check "budget: threads taper" "$(cat "$scratch"/thr.u{1,2,3,4,5,6,7} | tr '\n' ' ')" "8 5 3 2 2 2 2 "

: >"$scratch/events"
sched 'for i in 1 2 3; do unit p$i pg "$(work p$i 0.4)"; done'
check "postgres units run one at a time" "$(peak)" 1
check "postgres units all run" "$(grep -c ' e$' "$scratch/events")" 3

# Another check holds the Postgres lock: the other units run meanwhile and the Postgres unit follows.
: >"$scratch/events"
flock "$pg_lock" sleep 2 &
sleep 0.3
sched "unit p1 pg \"\$(work p1 0.2)\"; unit t1 tests \"\$(work t1 0.2)\""
wait %2 2>/dev/null || true
check "postgres lock: others first" "$(tr '\n' ' ' <"$scratch/events")" "t1 s t1 e p1 s p1 e "
check "postgres lock: waited" "$((elapsed >= 2 ? 1 : 0))" 1

# The first failure stops the rest, and the failing names come last.
: >"$scratch/events"
sched 'unit long tests "echo \$\$ >'"$scratch"'/long.pid; exec sleep 60"
       unit bad tests "sleep 0.5; echo \"test m::broken ... FAILED\"; echo failures:; echo boom; exit 101"'
check "fail-fast: exit code" "$rc" 1
check "fail-fast: quick" "$((elapsed < 10 ? 1 : 0))" 1
check "fail-fast: the others are gone" "$(kill -0 "$(<"$scratch/long.pid")" 2>/dev/null && echo alive || echo gone)" gone
check "fail-fast: failing names last" "$(tail -n 2 <<<"$out" | tr '\n' '|')" "failed tests:|  bad: m::broken|"

# A unit that failed while the caller compiled stops the run at the next look.
sched 'unit bad tests "exit 3"; start_ready; sleep 0.6; reap_done; unit never tests "touch '"$scratch"'/never"'
check "early failure: exit code" "$rc $([[ -e "$scratch/never" ]] && echo ran || echo stopped)" "1 stopped"

# The web checks run the typecheck, lint, format and tests in order, or only the vitest files a diff touches.
mkdir -p "$scratch/fake"
printf '#!/bin/sh\necho "pnpm $*" >>%s/pnpm.log\n' "$scratch" >"$scratch/fake/pnpm"
chmod +x "$scratch/fake/pnpm"
PATH="$scratch/fake:$PATH" sched 'add_web_unit "'"$scratch"'" full base1; run_units'
check "web full" "$(tr '\n' '|' <"$scratch/pnpm.log")" "pnpm --silent typecheck|pnpm --silent lint|pnpm --silent format|pnpm --silent test|"
: >"$scratch/pnpm.log"
PATH="$scratch/fake:$PATH" sched 'add_web_unit "'"$scratch"'" changed base1; run_units'
check "web changed" "$(tail -n 1 "$scratch/pnpm.log")" "pnpm exec vitest run --changed base1 --passWithNoTests"

# A unit over the run cap is killed and named as hung.
sched 'test_timeout=0.02; unit stuck tests "echo test m::stuck has been running for over 60 seconds; sleep 30"'
check "cap: exit code" "$rc" 1
check "cap: names the hung test" "$(grep -c 'exceeded .* a hung test' <<<"$out")" 1

finish check_gate
