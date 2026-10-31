#!/usr/bin/env bash
# shellcheck disable=SC2034,SC2154 # the libraries share globals with check.sh
# Run side of scripts/check.sh: the thread budget shared by the checks that run at once, and the scheduler that runs
# test binaries (units) side by side, stops at the first failure and keeps the output short.
# Sourced by check.sh after check-plan.sh. A caller may define `on_unit_ok <unit>`.

lease_dir="${MTG_CHECK_LEASES:-${XDG_RUNTIME_DIR:-/tmp}/mtg-check-leases}"
pg_lock="${TMPDIR:-/tmp}/mtg-pg-tests.lock"
test_timeout="${MTG_TEST_TIMEOUT:-40}"
web_threads=4

# --- Thread budget -----------------------------------------------------------------------------------------------

# take_lease: count this check as running until it exits (a lock on fd 7, closed for the children).
take_lease() {
  command -v flock >/dev/null || return 0
  mkdir -p "$lease_dir"
  exec 7>"$lease_dir/run.$$"
  flock -n 7 || true
}

# running_checks: how many checks hold a lease, this one included; leases of dead checks are removed.
running_checks() {
  local f n=0
  if command -v flock >/dev/null; then
    for f in "$lease_dir"/run.*; do
      [[ -e "$f" ]] || continue
      if flock -n "$f" true 2>/dev/null; then rm -f "$f"; else n=$((n + 1)); fi
    done
  fi
  echo $((n > 0 ? n : 1))
}

# current_share: the threads this check may use now (MTG_TEST_THREADS replaces the core count).
current_share() {
  thread_share "${MTG_TEST_THREADS:-$(nproc 2>/dev/null || echo 4)}" "$(running_checks)"
}

# --- Units -------------------------------------------------------------------------------------------------------

# A unit is one process: a test binary or the web checks. Its words and environment are one entry per line.
declare -A u_kind=() u_dir=() u_cmd=() u_env=() u_need=() u_lock=() u_log=() u_start=() unit_secs=() running=()
queue=()
used=0 lock_held=0 tick_pid="" run_dir="" pg_fd_open=0

init_units() {
  run_dir=$(mktemp -d "${TMPDIR:-/tmp}/mtg-check-run.XXXXXX")
  if command -v flock >/dev/null; then
    exec 9>"$pg_lock"
    pg_fd_open=1
  fi
}

# add_unit <name> <kind: tests|pg|web> <dir> <threads> <env lines> <command words, one per line>
add_unit() {
  local name="$1"
  u_kind[$name]="$2" u_dir[$name]="$3" u_need[$name]="$4" u_env[$name]="$5" u_cmd[$name]="$6"
  u_lock[$name]=0
  [[ "$2" != pg ]] || u_lock[$name]=1
  u_log[$name]="$run_dir/${name//[^A-Za-z0-9_.-]/_}.log"
  queue+=("$name")
}

# launch_unit <name> <threads>: start the unit in its own process group under the run cap; the exit code lands in
# "<log>.rc". Fds 7 and 9 are closed for the children, so a test left behind keeps neither lease nor lock.
launch_unit() {
  local name="$1" threads="$2" pid log="${u_log[$1]}"
  local -a words envs limit=()
  mapfile -t words < <(printf '%s' "${u_cmd[$name]}" | sed '/^$/d')
  mapfile -t envs < <(printf '%s' "${u_env[$name]}" | sed '/^$/d')
  # `--foreground` keeps the test binary in the unit's process group, so one kill reaches it.
  command -v timeout >/dev/null && limit=(timeout --foreground --signal=KILL "${test_timeout}m")
  set -m
  (
    cd "${u_dir[$name]}" || exit 1
    rc=0
    "${limit[@]}" env "RUST_TEST_THREADS=$threads" "${envs[@]}" "${words[@]}" || rc=$?
    echo "$rc" >"$log.rc"
    exit "$rc"
  ) >"$log" 2>&1 7>&- 9>&- &
  pid=$!
  set +m
  running[$pid]="$name"
  u_start[$name]=$SECONDS
  used=$((used + u_need[$name]))
}

# start_ready: start the queued units that fit the thread share (a unit that needs the Postgres lock waits for it).
start_ready() {
  local share free left name need rest=() blocked=0
  share=$(current_share)
  left=${#queue[@]}
  for name in "${queue[@]}"; do
    free=$((share - used))
    need="${u_need[$name]}"
    [[ "$need" != auto ]] || need="${RUST_TEST_THREADS:-$(unit_threads "$free" "$left")}"
    left=$((left - 1))
    if ((${#running[@]} > 0 && need > free)); then
      rest+=("$name")
      continue
    fi
    if ((u_lock[$name])); then
      if ((lock_held)) || { ((pg_fd_open)) && ! flock -n 9; }; then
        rest+=("$name")
        blocked=1
        continue
      fi
      lock_held=1
    fi
    u_need[$name]="$need"
    launch_unit "$name" "$need"
  done
  queue=("${rest[@]}")
  if ((blocked && ${#running[@]} > 0)) && [[ -z "$tick_pid" ]]; then
    sleep 1 &
    tick_pid=$!
  fi
}

# finished_unit <pid> <exit code>: record a finished unit; a failure stops the run.
finished_unit() {
  local pid="$1" rc="$2" name secs
  name="${running[$pid]}"
  unset "running[$pid]"
  secs=$((SECONDS - u_start[$name]))
  used=$((used - u_need[$name]))
  if ((u_lock[$name])); then
    ((!pg_fd_open)) || flock -u 9
    lock_held=0
  fi
  unit_secs[$name]="$secs"
  if ((rc != 0)); then
    unit_failed "$name" "$rc" "$secs"
  fi
  echo "[${u_kind[$name]}] $name ok (${secs}s)"
  if declare -F on_unit_ok >/dev/null; then on_unit_ok "$name"; fi
}

# reap_done: take in the units that finished while the caller ran something else (stops at the first failure).
reap_done() {
  local pid log
  for pid in "${!running[@]}"; do
    log="${u_log[${running[$pid]}]}"
    [[ -s "$log.rc" ]] || continue
    wait "$pid" 2>/dev/null || true
    finished_unit "$pid" "$(<"$log.rc")"
  done
}

# kill_units: stop every running unit with its children and wait for them.
kill_units() {
  local pid
  for pid in "${!running[@]}"; do kill -KILL -- "-$pid" 2>/dev/null || true; done
  [[ -z "$tick_pid" ]] || kill "$tick_pid" 2>/dev/null || true
  for pid in "${!running[@]}"; do
    wait "$pid" 2>/dev/null || true
    unset "running[$pid]"
  done
  tick_pid=""
}

# failed_tests <log>: the names of the tests that failed in a libtest log.
failed_tests() {
  sed -n 's/^test \(.*\) \.\.\. FAILED\( <[0-9.]*s>\)\{0,1\}$/\1/p' "$1"
}

# unit_failed <name> <exit code> <seconds>: report the failure, stop the others, end the check.
unit_failed() {
  local name="$1" rc="$2" secs="$3" log="${u_log[$1]}" limit_secs kind="${u_kind[$1]}"
  limit_secs=$(awk -v m="$test_timeout" 'BEGIN { printf "%d", m * 60 }')
  echo "[$kind] $name FAILED (${secs}s)"
  if ((rc == 124 || rc == 137)) && ((secs >= limit_secs - 2)); then
    echo "[$kind] $name exceeded $test_timeout minutes: a hung test; running for over 60 seconds when it was killed:"
    sed -n 's/.*test \([^ ]*\) has been running for over 60 seconds.*/  \1/p' "$log" | sort -u
  elif grep -q '^failures:$' "$log"; then
    sed -n '/^failures:$/,$p' "$log" | head -n 80
  else
    tail -n 40 "$log"
  fi
  kill_units
  if [[ -n "$(failed_tests "$log")" ]]; then
    echo "failed tests:"
    failed_tests "$log" | head -n 50 | sed "s/^/  $name: /"
  fi
  exit 1
}

# finish_one: wait for any unit (or the lock tick) to end.
finish_one() {
  local pid="" rc=0
  wait -n -p pid || rc=$?
  [[ -n "$pid" ]] || return 0
  if [[ "$pid" == "$tick_pid" ]]; then
    tick_pid=""
  elif [[ -v running[$pid] ]]; then
    finished_unit "$pid" "$rc"
  fi
}

# run_units: run the queue and the units already started until all have ended.
run_units() {
  local wait_start
  while ((${#queue[@]} + ${#running[@]} > 0)); do
    reap_done
    start_ready
    if ((${#running[@]} == 0)); then
      # Only units that need the Postgres lock are left, and another check holds it.
      wait_start=$SECONDS
      flock 9
      lock_held=0
      secs["lock wait"]=$((${secs["lock wait"]:-0} + SECONDS - wait_start))
      continue
    fi
    finish_one
  done
}

# slow_warnings <label> <log>: tests of a libtest log that took over the limit and match no slow-list entry.
slow_warnings() {
  local entries
  entries=$(slow_entries)
  awk -v label="$1" -v limit="$slow_limit" -v entries="$entries" '
    BEGIN { n = split(entries, e, "\n") }
    /^test .* \.\.\. ok <[0-9.]+s>$/ {
      t = $NF; gsub(/[<>s]/, "", t)
      if (t + 0 <= limit) next
      for (i = 1; i <= n; i++) if (e[i] != "" && index($2, e[i])) next
      printf "  %s: %s %.1fs\n", label, $2, t
    }' "$2"
}

# report_units: the slowest units and the tests over the limit that the slow list does not name.
report_units() {
  local name warn=""
  ((${#unit_secs[@]} > 0)) || return 0
  for name in "${!unit_secs[@]}"; do
    [[ "${u_kind[$name]}" == web ]] || warn+=$(slow_warnings "$name" "${u_log[$name]}")$'\n'
  done
  warn=$(printf '%s' "$warn" | sed '/^$/d' | awk '{ print $NF, $0 }' | sort -rn | head -n 20 | cut -d' ' -f2-)
  if [[ -n "$warn" ]]; then
    echo "warning: tests over ${slow_limit}s that $slow_file does not name (add them or make them faster):"
    echo "$warn"
  fi
  for name in "${!unit_secs[@]}"; do echo "${unit_secs[$name]} $name"; done | sort -rn | head -n 4 |
    awk '{ printf "%s%s %ss", (NR > 1 ? ", " : "slowest units: "), $2, $1 } END { print "" }'
}
