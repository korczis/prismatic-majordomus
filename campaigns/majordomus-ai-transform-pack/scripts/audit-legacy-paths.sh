#!/usr/bin/env bash
set -eu

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
[ -n "$ROOT" ] || { echo "not in a git repository" >&2; exit 2; }

cd "$ROOT"
rg '\.majordomus/(policy|profiles|project|prompts|providers|state|generated)' \
  --hidden \
  -g '!tmp/**' \
  -g '!.git/**' \
  -g '!node_modules/**' || true
