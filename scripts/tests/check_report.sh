#!/usr/bin/env bash
# Goal: the closing report of `scripts/check.sh` returns 0 and prints its listings whole under pipefail, however long
# the slow-test warnings and the unit list are, and a failed unit still lists its failed tests.
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=probe.bash
source scripts/tests/probe.bash
# shellcheck source=../lib/check-plan.sh
source scripts/lib/check-plan.sh
# shellcheck source=../lib/check-run.sh
source scripts/lib/check-run.sh

# fill <units> <tests per unit>: units whose logs each hold that many unlisted tests over the slow limit.
fill() {
  local u t log
  u_kind=() u_log=() unit_secs=()
  for ((u = 1; u <= $1; u++)); do
    log="$scratch/unit$u.log"
    for ((t = 1; t <= $2; t++)); do echo "test probe_mod_$u::a_long_test_name_number_$t ... ok <9.${t}s>"; done >"$log"
    u_kind[unit$u]=test u_log[unit$u]="$log" unit_secs[unit$u]=$u
  done
}

# strict <command...>: run a command in a subshell under errexit as check.sh does (a `||` would switch errexit off
# inside it); sets out and rc.
strict() {
  set +e
  (
    set -e
    "$@"
  ) >"$scratch/out" 2>&1
  rc=$?
  set -e
  out=$(<"$scratch/out")
}

# report <units> <tests per unit>: run report_units; sets out and rc.
report() {
  fill "$1" "$2"
  strict report_units
}

for size in "1 1" "6 3" "40 200"; do
  read -r units tests <<<"$size"
  report "$units" "$tests"
  check "exit code ($size)" "$rc" 0
  check "slowest units line ($size)" "$(grep -c '^slowest units: ' <<<"$out")" 1
  want=$((units * tests < 20 ? units * tests : 20))
  check "warning lines ($size)" "$(grep -c '^  unit' <<<"$out")" "$want"
done
report 40 200
check "slowest first" "$(sed -n 's/^slowest units: \([^ ]*\) .*/\1/p' <<<"$out")" unit40

check "first reads to the end" "$( (seq 1 200000 | first 3 | tr '\n' ' ') && echo ok)" "1 2 3 ok"

# A failing unit whose log names many failed tests still prints the list (the check then ends with 1, not 141).
fail_log="$scratch/fail.log"
{
  for ((t = 1; t <= 5000; t++)); do echo "test probe::failing_test_with_a_long_name_$t ... FAILED"; done
  echo "failures:"
  for ((t = 1; t <= 5000; t++)); do echo "    probe::failing_test_with_a_long_name_$t"; done
  echo "test result: FAILED"
} >"$fail_log"
u_kind=([bad]=test) u_log=([bad]="$fail_log") unit_crate=()
kill_units() { :; }
strict unit_failed bad 101 3
check "failed unit exit code" "$rc" 1
check "failed tests listed" "$(grep -c '^  bad: ' <<<"$out")" 50

finish check_report
