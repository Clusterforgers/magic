#!/usr/bin/env bash
# Usage: scripts/agent-status.sh   The orchestrator's view of a loaded machine: CPU per worktree over 6 s, test binaries
# running for over 15 minutes with the threads that burn CPU (a hung game), processes whose worktree is gone, wait loops
# that match themselves, and scratch databases.
set -uo pipefail
ck=$(getconf CLK_TCK)
name_of() { readlink "/proc/$1/cwd" 2>/dev/null | sed -e 's#.*worktrees/agent-##' -e 's# (deleted)#!#' -e 's#^/.*#main#' | cut -c1-12; }

echo "load: $(cut -d' ' -f1-3 /proc/loadavg)"
declare -A before
while read -r pid cpu; do before[$pid]=$cpu; done < <(ps -eo pid=,cputimes=)
sleep 6
echo "== CPU by worktree over 6 s (percent of one core, 24 cores = 2400)"
while read -r pid cpu; do
  [[ -n ${before[$pid]:-} ]] || continue
  delta=$(((cpu - before[$pid]) * 100 / 6))
  ((delta >= 15)) && echo "$(name_of "$pid") $delta $pid"
done < <(ps -eo pid=,cputimes=) | awk '{c[$1]+=$2} END {for (k in c) printf "  %-13s %5d%%\n", k, c[k]}' | sort -k2 -nr | head -10

echo "== test binaries running over 15 minutes (threads with over 300 s of CPU are likely stuck)"
ps -eww -o pid=,etimes=,args= | awk '$2 > 900 && $3 ~ /target\/debug\/deps\// {print $1, $2}' | while read -r pid age; do
  echo "  $(name_of "$pid") pid $pid age ${age}s"
  for t in /proc/$pid/task/*; do
    awk -v ck="$ck" '{n=($14+$15)/ck; if (n > 300) printf "    thread %s %s cpu %ds\n", $1, $2, n}' "$t/stat" 2>/dev/null
  done
done

echo "== processes whose worktree is gone"
for d in /proc/[0-9]*; do
  [[ $(readlink "$d/cwd" 2>/dev/null) == *"(deleted)"* ]] && echo "  pid ${d#/proc/} $(tr '\0' ' ' < "$d/cmdline" | cut -c1-90)"
done

echo "== wait loops (a loop on pgrep -f matches its own command line and never ends)"
ps -eww -o pid=,etimes=,args= | awk '/shell-snapshots/ && /(until|while) / && !/awk/ {line=$0; sub(/.*eval /,"",line); flag=(line ~ /pgrep -f/) ? " SELF-MATCHING" : ""; print "  pid "$1" age "$2"s"flag" :: "substr(line,1,80)}'

echo "== scratch databases"
docker compose exec -T db psql -U magic -Atc "select datname from pg_database where datname like 'magic\_%' order by 1" 2>/dev/null | sed 's/^/  /'
