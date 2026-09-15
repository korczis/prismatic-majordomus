#!/usr/bin/env bash
set -eu

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
[ -n "$ROOT" ] || { echo "not in a git repository" >&2; exit 2; }

OUT="$ROOT/tmp/transform/_run"
mkdir -p "$OUT"

{
  echo "# Baseline"
  echo
  echo "## Git status"
  echo '```'
  git -C "$ROOT" status --short --branch || true
  echo '```'
  echo
  echo "## Recent commits"
  echo '```'
  git -C "$ROOT" log --oneline -20 || true
  echo '```'
  echo
  echo "## Doctor"
  echo '```'
  if [ -x "$ROOT/bin/majordomus" ]; then
    (cd "$ROOT" && bin/majordomus doctor) || true
  else
    echo "bin/majordomus not found"
  fi
  echo '```'
  echo
  echo "## Plan validate"
  echo '```'
  if [ -x "$ROOT/bin/majordomus" ]; then
    (cd "$ROOT" && bin/majordomus plan validate) || true
  fi
  echo '```'
} > "$OUT/BASELINE.md"

echo "$OUT/BASELINE.md"
