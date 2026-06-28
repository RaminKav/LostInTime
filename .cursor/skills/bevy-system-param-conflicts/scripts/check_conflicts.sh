#!/usr/bin/env bash
# Scan a run/log file for Bevy B0002 system-param conflict panics.
# Usage: check_conflicts.sh <logfile>
# Exit 0 = no conflicts found, 1 = conflicts found, 2 = bad usage.
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "usage: $0 <logfile>" >&2
  exit 2
fi

log="$1"
if [ ! -f "$log" ]; then
  echo "no such file: $log" >&2
  exit 2
fi

# Match B0001/B0002 and the human-readable "conflicts with a previous" line.
if grep -nE 'B000[12]|conflicts with a previous' "$log"; then
  echo "---"
  echo "Bevy system-param conflict detected (see lines above)."
  echo "B0001: merge conflicting Queries into ParamSet or add Without<> filters."
  echo "B0002: remove duplicate resource access (route through composite SystemParam)."
  exit 1
fi

echo "No B0001/B0002 system-param conflicts found in $log"
exit 0
