#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cp -R "$ROOT/.majordomo" "$TMP/.majordomo"
"$ROOT/scripts/majordomus-doctor" "$TMP" >/dev/null
rm "$TMP/.majordomo/policy.yaml"
if "$ROOT/scripts/majordomus-doctor" "$TMP" >/dev/null 2>&1; then echo 'FAIL: doctor accepted missing policy.yaml' >&2; exit 1; fi
echo 'test-doctor: PASS'
