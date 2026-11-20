#!/usr/bin/env bash
# shellcheck disable=SC2016 # the cases hand code to `eval` in single quotes
# Goal: the unit scheduler of `scripts/check.sh` runs binaries side by side within the thread budget, one Postgres unit
# at a time on the shared server or side by side on servers of their own, stops at the first failure with the failing
# names last, and kills a unit over the run cap.
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

lease_dir="$scratch/leases"
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

# A shell snippet that logs "<name> s", waits until <together> units have started (no unit ends before that many ran at
# once, whatever the machine's speed), pauses 0.2 s so that a unit that should have waited overlaps, and logs "<name> e".
work() {
  printf 'echo %s s >>%s; echo . >>%s; n=0; while [ "$(wc -l <%s)" -lt %s ] && [ $n -lt 1200 ]; do sleep 0.05; n=$((n + 1)); done; sleep 0.2; echo %s e >>%s' \
    "$1" "$scratch/events" "$scratch/arrived" "$scratch/arrived" "$2" "$1" "$scratch/events"
}
# reset: empty the events of the last case.
reset() { : >"$scratch/events" && : >"$scratch/arrived"; }
# peak: the most units that were running at once, from the events.
peak() { awk '$2 == "s" { n++; if (n > m) m = n } $2 == "e" { n-- } END { print m + 0 }' "$scratch/events"; }

reset
sched 'for i in 1 2 3 4 5 6; do unit u$i tests "$(work u$i 4)"; done'
check "budget: all ok" "$rc" 0
check "budget: share 8 of 2 threads runs four at once" "$(peak)" 4
check "budget: output" "$(grep -c '^\[tests\] u. ok (' <<<"$out")" 6

reset
MTG_TEST_THREADS=4 sched 'for i in 1 2 3 4; do unit u$i tests "$(work u$i 2)"; done'
check "budget: share 4 runs two at once" "$(peak)" 2

# Without a fixed thread count the first binary takes a third of the threads and the later ones less.
thr() { echo "echo \$RUST_TEST_THREADS >$scratch/thr.$1; sleep 0.3"; }
MTG_TEST_THREADS=24 sched 'unset RUST_TEST_THREADS; for i in 1 2 3 4 5 6 7; do unit u$i tests "$(thr u$i)"; done'
check "budget: threads taper" "$(cat "$scratch"/thr.u{1,2,3,4,5,6,7} | tr '\n' ' ')" "8 5 3 2 2 2 2 "

# With the seconds of their last runs known, each binary takes its share of the waiting work.
MTG_TEST_THREADS=24 sched 'unset RUST_TEST_THREADS; for i in 1 2 3; do unit u$i tests "$(thr u$i)"; done
  u_expect=([u1]=600 [u2]=300 [u3]=100)'
check "budget: threads follow the last seconds" "$(cat "$scratch"/thr.u{1,2,3} | tr '\n' ' ')" "12 9 3 "

reset
sched 'for i in 1 2 3; do unit p$i pg "$(work p$i 1)"; done'
check "postgres units run one at a time" "$(peak)" 1
check "postgres units all run" "$(grep -c ' e$' "$scratch/events")" 3

# With a server each, the Postgres units run side by side past a held lock, each on its own URL, and stop their servers.
pg_start() { echo "srv$BASHPID postgres://fake/$BASHPID"; }
pg_stop() { echo "$1" >>"$scratch/stopped"; }
reset
: >"$scratch/stopped"
# The lock stays held until the units are done, and each unit records whether it started before the lock was let go: a
# run that waited for the lock would start later (the holder gives up after a minute), so no timing is involved.
hold_lock "$pg_lock"
pg_mode=own sched 'for i in 1 2 3; do
  unit p$i pg "if [ -e '"$scratch"'/gone ]; then echo waited; else echo early; fi >'"$scratch"'/lock.p$i; $(work p$i 3); echo \$DATABASE_URL >'"$scratch"'/url.p$i"
done'
release_lock
check "own servers: side by side" "$(peak)" 3
check "own servers: no lock wait" "$(cat "$scratch"/lock.p? | sort | uniq -c | tr -s ' ' | tr '\n' ' ')" " 3 early "
check "own servers: one URL each" "$(sort -u "$scratch"/url.p? | grep -c '^postgres://fake/')" 3
check "own servers: all stopped" "$(sort -u "$scratch/stopped" | wc -l)" 3
pg_start() { return 1; }
pg_mode=own sched 'unit p1 pg "touch '"$scratch"'/ran.p1"'
check "own servers: a server that does not start fails the unit" "$rc $(grep -c 'could not start a Postgres server' <<<"$out") $([[ -e "$scratch/ran.p1" ]] && echo ran || echo not)" "1 1 not"

# Another check holds the Postgres lock until the other unit is done: that one runs meanwhile and the Postgres unit
# follows once the lock is free.
reset
hold_lock "$pg_lock"
sched "unit p1 pg \"\$(work p1 1); if [ -e $scratch/release ]; then echo after; else echo early; fi >$scratch/after.p1\"
       unit t1 tests \"\$(work t1 1); touch $scratch/release\""
release_lock
check "postgres lock: others first" "$(tr '\n' ' ' <"$scratch/events")" "t1 s t1 e p1 s p1 e "
check "postgres lock: waited" "$(cat "$scratch/after.p1")" after

# A check that only waits for the Postgres lock does not count as running, and counts again after.
take_lease
check "lease: own check counts" "$(running_checks)" 1
exec 8>"$lease_dir/run.other"; flock -n 8
check "lease: two checks" "$(running_checks)" 2
drop_lease
check "lease: waiting check not counted" "$(running_checks)" 1
take_lease
check "lease: counted again after" "$(running_checks)" 2
exec 8>&-

# The Postgres unit sizes its threads from the share after the wait: with no other check it gets what is free (at most 12).
hold_lock "$pg_lock"
{ sleep 0.5 && touch "$scratch/release"; } &
MTG_TEST_THREADS=24 sched 'unset RUST_TEST_THREADS; unit p1 pg "echo \$RUST_TEST_THREADS >'"$scratch"'/thr.p1"'
release_lock
check "postgres lock: threads after the wait" "$(cat "$scratch/thr.p1")" 12

# The first failure stops the rest, and the failing names come last.
# A run that waited for the long unit would take its 120 s: the bound is far from any machine's slowness.
sched 'unit long tests "echo \$\$ >'"$scratch"'/long.pid; exec sleep 120"
       unit bad tests "sleep 0.4; echo \"test m::broken ... FAILED\"; echo failures:; echo boom; exit 101"'
check "fail-fast: exit code" "$rc" 1
check "fail-fast: quick" "$((elapsed < 60 ? 1 : 0))" 1
check "fail-fast: the others are gone" "$(kill -0 "$(<"$scratch/long.pid")" 2>/dev/null && echo alive || echo gone)" gone
check "fail-fast: failing names last" "$(tail -n 2 <<<"$out" | tr '\n' '|')" "failed tests:|  bad: m::broken|"

# A unit that failed while the caller compiled stops the run at the next look.
sched 'unit bad tests "exit 3"; start_ready
       until [[ -s "${u_log[bad]}.rc" ]]; do sleep 0.05; done
       reap_done; unit never tests "touch '"$scratch"'/never"'
check "early failure: exit code" "$rc $([[ -e "$scratch/never" ]] && echo ran || echo stopped)" "1 stopped"

# The web checks run the typecheck, lint, format and tests in order, or only the vitest files a diff touches.
mkdir -p "$scratch/fake"
printf '#!/bin/sh\necho "pnpm $*" >>%s/pnpm.log\n' "$scratch" >"$scratch/fake/pnpm"
chmod +x "$scratch/fake/pnpm"
PATH="$scratch/fake:$PATH" sched 'add_web_unit "'"$scratch"'" full base1'
check "web full" "$(tr '\n' '|' <"$scratch/pnpm.log")" "pnpm --silent typecheck|pnpm --silent lint|pnpm --silent format|pnpm --silent test|"
: >"$scratch/pnpm.log"
PATH="$scratch/fake:$PATH" sched 'add_web_unit "'"$scratch"'" changed base1'
check "web changed" "$(tail -n 1 "$scratch/pnpm.log")" "pnpm exec vitest run --changed base1 --passWithNoTests"

# A unit over the run cap is killed and named as hung.
sched 'test_timeout=0.02; unit stuck tests "echo test m::stuck has been running for over 60 seconds; sleep 30"'
check "cap: exit code" "$rc" 1
check "cap: names the hung test" "$(grep -c 'exceeded .* a hung test' <<<"$out")" 1

finish check_sched
