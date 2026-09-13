#!/usr/bin/env bash
ROOT=/Users/korczis/dev/prismatic-majordomus/.claude/worktrees/agent-a49e63d835ab36ae7
cd "$ROOT/apps/majordomus-cli" || exit 2
unset MAJORDOMUS_SHARE
cargo build --release > "$ROOT/.mj-cp-build.log" 2>&1
echo "BUILD_EXIT=$?"
tail -3 "$ROOT/.mj-cp-build.log"
