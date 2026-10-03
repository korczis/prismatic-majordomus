#!/usr/bin/env sh
set -eu

required="
.ai/00_README.md
.ai/10_CORE.md
.ai/20_ARCHITECTURE.md
.ai/30_DEVELOPMENT.md
.ai/40_TESTING.md
.ai/50_PRISMATIC_PORTING.md
.ai/60_ROADMAP.md
.ai/90_PROVIDER_INTEGRATION.md
AGENTS.md
CLAUDE.md
GEMINI.md
"

failed=0

for file in $required; do
  if [ ! -f "$file" ]; then
    echo "MISSING: $file" >&2
    failed=1
  fi
done

if [ "$failed" -ne 0 ]; then
  exit 1
fi

for file in AGENTS.md CLAUDE.md GEMINI.md; do
  if ! grep -q '\.ai/' "$file"; then
    echo "INVALID: $file does not reference canonical .ai/ content" >&2
    failed=1
  fi
done

if grep -R -nE 'prismatic[^[:space:]]*[[:space:]]*(dependency|dep)|from[[:space:]]+Prismatic|import[[:space:]].*Prismatic'   AGENTS.md CLAUDE.md GEMINI.md .ai 2>/dev/null; then
  echo "REVIEW: possible Prismatic dependency wording detected above" >&2
fi

if [ "$failed" -ne 0 ]; then
  exit 1
fi

echo "AI layer: OK"
