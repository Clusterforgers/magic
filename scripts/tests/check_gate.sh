#!/usr/bin/env bash
# Goal: the gate tests only the crates a diff changes and checks their dependents; the slow list, the thread budget and
# the units made from the artifacts of a cargo build do what docs/testing.md says.
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=probe.bash
source scripts/tests/probe.bash
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
gate_case "engine" mtg-engine - "mtg-campaign mtg-cli mtg-protocol mtg-server mtg-testkit" none -- crates/mtg-engine/src/lib.rs
gate_case "cards" mtg-cards - "mtg-campaign mtg-cli mtg-engine mtg-protocol mtg-server mtg-testkit" none -- crates/mtg-cards/cards/a.json
gate_case "core" mtg-core - "mtg-campaign mtg-cards mtg-cli mtg-db mtg-engine mtg-import mtg-protocol mtg-server mtg-testkit" none \
  -- crates/mtg-core/src/lib.rs
gate_case "protocol" mtg-protocol - mtg-server full -- crates/mtg-protocol/src/lib.rs
gate_case "database" - mtg-db "mtg-cli mtg-server" none -- crates/mtg-db/src/lib.rs
gate_case "leaf with Postgres" - mtg-cli - none -- crates/mtg-cli/src/main.rs
gate_case "sqlx metadata" - mtg-db "mtg-cli mtg-server" none -- .sqlx/query-probe.json
gate_case "two crates" mtg-engine mtg-db "mtg-campaign mtg-cli mtg-protocol mtg-server mtg-testkit" none \
  -- crates/mtg-engine/src/lib.rs crates/mtg-db/src/lib.rs
gate_case "web" - - - changed -- web/src/lib/flash.ts
gate_case "protocol fixture" mtg-protocol - - changed -- web/src/protocol/fixtures/probe.json
gate_case "root lock" "mtg-campaign mtg-cards mtg-core mtg-engine mtg-import mtg-protocol mtg-testkit" "mtg-cli mtg-db mtg-server" - full -- Cargo.lock

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
check "over a sliced binary's limit" "$(slow_warnings lbl "$scratch/run.log" 6.5)" ""
check "failed names" "$(failed_tests "$scratch/run.log" | tr '\n' ' ')" "a::broken a::plain "

# --- Seed slices ---------------------------------------------------------------------------------------------------

# slice_of <mode> [commit]: the slice seed_slice picks, or "error".
slice_of() { if seed_slice "$@" 2>/dev/null; then echo "$REPLY"; else echo error; fi; }
check "slice: affected runs all" "$(MTG_SEED_SLICE='' slice_of affected)" all
check "slice: all runs all" "$(MTG_SEED_SLICE='' slice_of all)" all
check "slice: the gate takes one of the commit" "$(MTG_SEED_SLICE='' slice_of gate "$head")" \
  "$((16#${head:0:7} % gate_slices + 1))/$gate_slices"
check "slice: the gate is the same on one commit" "$(MTG_SEED_SLICE='' slice_of gate "$head")" \
  "$(MTG_SEED_SLICE='' slice_of gate "$head")"
spread=$(for rev in $(git rev-list --max-count=40 "$head"); do MTG_SEED_SLICE='' slice_of gate "$rev"; done | sort -u | wc -l)
check "slice: forty commits use every slice" "$spread" "$gate_slices"
for row in "2/4 gate 2/4" "all gate all" "1/1 affected 1/1" "007/10 gate 7/10" "0/4 gate error" "5/4 gate error" "x gate error" \
  "3 gate error"; do
  read -r spec mode want <<<"$row"
  check "slice: MTG_SEED_SLICE=$spec in $mode" "$(MTG_SEED_SLICE="$spec" slice_of "$mode")" "$want"
done
check "slice: the gate lists its slice" \
  "$(MTG_SEED_SLICE=3/4 scripts/check.sh gate --list "$head" "$(probe_commit docs/ROADMAP.md)" | grep '^seed slice')" \
  "seed slice: 3/4 (suites of scripts/seed-slices.txt)"
check "slice: a bad value stops the check" \
  "$(MTG_SEED_SLICE=9/4 scripts/check.sh gate --list "$head" "$head" >/dev/null 2>&1 && echo ran || echo "stopped $?")" "stopped 2"

slice_file="$scratch/slices.txt"
printf '# header\nfam::fuzz\nfam::game  house_   # a prefix\n\n' >"$slice_file"
check "slice entries" "$(slice_entries | tr '\n' '|')" "fam::fuzz|fam::game  house_|"
check "sliced units" "$(for u in fam::fuzz fam::game fam::lib fam; do sliced_unit "$u" && echo "$u"; done | tr '\n' ' ')" "fam::fuzz fam::game "
names=$(for i in $(seq 1 200); do echo "a_fuzz::case_$i"; echo "house_x::seed_$i"; echo "plain::t_$i"; done)
union="" total=0
for k in 1 2 3 4; do
  part=$(slice_tests fam::game "$k/4" "" <<<"$names")
  total=$((total + $(grep -c '^house_' <<<"$part")))
  union+=$(grep '^house_' <<<"$part")$'\n'
  check "slice $k/4 keeps every test off the list" "$(grep -c '^plain::\|^a_fuzz::' <<<"$part")" 400
  check "slice $k/4 holds a share of the listed tests" "$(($(grep -c '^house_' <<<"$part") > 25))" 1
done
check "the slices of a suite are disjoint" "$total" 200
check "the slices of a suite cover it" "$(sed '/^$/d' <<<"$union" | sort -u | wc -l)" 200
check "a unit without a prefix is sliced whole" "$(($(slice_tests fam::fuzz 1/4 "" <<<"$names" | grep -c '^plain::') < 100))" 1
printf 'fam::fuzz !plain::\n' >>"$slice_file"
check "a ! prefix keeps its tests whole" "$(slice_tests fam::fuzz 1/4 "" <<<"$names" | grep -c '^plain::')" 200
check "a ! prefix keeps the rest sliced" "$(($(slice_tests fam::fuzz 1/4 "" <<<"$names" | grep -c '^a_fuzz::') < 100))" 1
check "a slice applies the slow list" "$(slice_tests fam::other 1/3 $'plain::\nseed_1' <<<"$names" | grep -c 'plain\|seed_1')" 0
check "a slice is the same every run" "$(slice_tests fam::fuzz 2/3 "" <<<"$names" | md5sum)" \
  "$(slice_tests fam::fuzz 2/3 "" <<<"$names" | md5sum)"

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

# free threads, seconds of the binary, seconds waiting -> threads: its share of the work, rounded up, between 2 and 12
for row in "8 700 1000 6" "24 700 1000 12" "8 100 1000 2" "12 300 300 12" "6 1 1 6" "4 0 0 2"; do
  read -r free mine waiting threads <<<"$row"
  check "threads for $mine of $waiting seconds with $free free" "$(expected_threads "$free" "$mine" "$waiting")" "$threads"
done

lease_dir="$scratch/leases"
mkdir -p "$lease_dir"
touch "$lease_dir/run.dead"
hold_lock "$lease_dir/run.live"
check "leases: one live check" "$(running_checks)" 1
check "leases: a dead one is removed" "$([[ -e "$lease_dir/run.dead" ]] && echo kept || echo removed)" removed
check "leases: this check counts" "$(
  take_lease
  running_checks
)" 2
release_lock

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
check "rerun command" "${unit_rerun['mtg-cli::it']}|${unit_rerun['mtg-core::lib']}" \
  "cargo test -p mtg-cli --test it --|cargo test -p mtg-core --lib --"

# The seconds of the last run order the units before their size; the newest run of a unit counts.
unit_secs_file="$scratch/unit-secs"
declare -A unit_secs=([mtg-core::lib]=900 [mtg-cli::lib]=30 [web]=999)
record_unit_secs
unit_secs=([mtg-core::lib]=400)
record_unit_secs
check "the web is not a recorded unit" "$(grep -c '^web ' "$unit_secs_file")" 0
unit_secs=()
queue=()
add_test_units "$scratch/build.json" >/dev/null
check "unit order by last seconds" "${queue[*]}" "mtg-core::lib mtg-cli::lib mtg-cli::it"
check "expected seconds" "${u_expect[mtg-core::lib]}|${u_expect[mtg-cli::lib]}|${u_expect[mtg-cli::it]:-}" "400|30|"
rm -f "$unit_secs_file"

# Outside gate the listed heavy binaries run alone; the gate runs them side by side.
alone_units="mtg-cli::it"
queue=() u_alone=()
add_test_units "$scratch/build.json" >/dev/null
check "alone: not in gate" "${u_alone[*]:-none}" none
queue=() run_alone=1
add_test_units "$scratch/build.json" >/dev/null
check "alone: the listed binary" "${!u_alone[*]}" mtg-cli::it
run_alone=0 u_alone=()

# A sliced unit runs the exact names of its slice; the slow list is applied to the names.
cat >"$scratch/fuzz" <<'EOF'
#!/usr/bin/env bash
[[ "$*" == "--list --format terse" ]] || exit 1
for i in $(seq 1 40); do echo "a_fuzz::seed_$i: test"; done
echo "slow::one: test"
EOF
chmod +x "$scratch/fuzz"
artifact test mtg-testkit fuzz test fuzz >"$scratch/build.json"
printf 'mtg-testkit::fuzz\n' >"$slice_file"
queue=()
seed_slice=2/4
add_test_units "$scratch/build.json" --skip slow::one >"$scratch/note"
seed_slice=all
mapfile -t words <<<"${u_cmd['mtg-testkit::fuzz']}"
check "sliced unit: the note" "$(<"$scratch/note")" "mtg-testkit::fuzz: seed slice 2/4 runs $((${#words[@]} - 5)) of 41 tests"
check "sliced unit: exact names" "${words[*]:0:5}" "$scratch/fuzz -Z unstable-options --report-time --exact"
check "sliced unit: a share of the seeds" "$(((${#words[@]} - 5) > 3 && (${#words[@]} - 5) < 20))" 1
check "sliced unit: the slow list" "$(printf '%s\n' "${words[@]}" | grep -c 'slow::one')" 0
check "sliced unit: marked" "${unit_sliced['mtg-testkit::fuzz']:-no}" 1

finish check_gate
