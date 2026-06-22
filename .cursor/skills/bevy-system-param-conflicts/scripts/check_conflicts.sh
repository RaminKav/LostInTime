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

# Match the B0002 error and the human-readable "conflicts with a previous" line.
if grep -nE 'B0002|conflicts with a previous .* access' "$log"; then
  echo "---"
  echo "B0002 system-param conflict detected (see lines above)."
  echo "Open the named system and remove the duplicate access (route it through the composite param)."
  exit 1
fi

echo "No B0002 system-param conflicts found in $log"
exit 0
