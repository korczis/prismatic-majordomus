# majordomus-covers: doctor
# The worktree layout doctrine, end to end, against real git.
#
# A linked worktree outside the canonical container is a policy violation: doctor reports it
# and changes nothing, the migration plan proposes the move and changes nothing, and only
# `migrate --apply` moves it. The container is derived from the primary checkout and the
# policy's suffix, so this case also proves the two things easiest to get wrong — that the
# answer is the same from inside a linked worktree (and is not a container nested inside that
# worktree), and that changing only the policy's suffix moves the container.
#
# The Rust crate's own suite (apps/majordomus-cli/tests/worktree.rs) covers the refusals in
# depth. What is here is the part that crosses the two programs: the shell doctor reporting
# what the executable's service found.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

# The repository is nested one level down on purpose: its container is its *sibling*, so a
# repository rooted at the case's temporary directory would put the container beside that
# directory and leave it behind. Everything this case creates stays under $T.
W="$T/wt"; R="$W/repo"
mkdir -p "$W"
fixture_repo "$R" >/dev/null
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
git -C "$R" add -A >/dev/null
git -C "$R" commit -qm fixture >/dev/null
[ -f "$R/.ai/manifest.yaml" ] || { echo "    the fixture repository has no layer"; exit 1; }

# every invocation of the executable, from a directory the caller names
mj() { local cwd="$1"; shift; ( cd "$cwd" && MAJORDOMUS_SHARE="$R/share" "$RB" "$@" ); }

# ---------------------------------------------------------------- 1. the derived container
expect_exit 0 mj "$R" worktree root
[ "$LAST_OUT" = "$R-wt" ] || { echo "    root is '$LAST_OUT', expected '$R-wt'"; exit 1; }

# ---------------------------------------------------------------- 2. create derives the path
expect_exit 0 mj "$R" worktree create feature-x
[ -d "$R-wt/feature-x" ] || { echo "    $R-wt/feature-x was not created"; exit 1; }
[ -e "$R/feature-x" ] && { echo "    a directory was created inside the repository"; exit 1; }
git -C "$R" worktree list --porcelain | grep -qx "worktree $R-wt/feature-x" \
  || { echo "    git did not register $R-wt/feature-x"; exit 1; }

# ---------------------------------------------------------------- 3. the same answer from within
mkdir -p "$R-wt/feature-x/apps/foo/src"
expect_exit 0 mj "$R-wt/feature-x/apps/foo/src" worktree root
[ "$LAST_OUT" = "$R-wt" ] || { echo "    from inside a linked worktree the root is '$LAST_OUT', expected '$R-wt'"; exit 1; }
expect_no_grep "feature-x-wt"

# a worktree created from inside the first lands in the same container
expect_exit 0 mj "$R-wt/feature-x" worktree create feature-y
[ -d "$R-wt/feature-y" ] || { echo "    a worktree created from a linked worktree left the container"; exit 1; }

# ---------------------------------------------------------------- 4. idempotence
expect_exit 10 mj "$R" worktree create feature-x
expect_grep "already registered"

# ---------------------------------------------------------------- 5. a worktree outside it
git -C "$R" worktree add -q -b legacy "$W/outside"
before="$(git -C "$R" worktree list --porcelain)"

expect_exit 10 mj "$R" worktree list
expect_grep "FAIL"

# doctor reports it, by path, and moves nothing
expect_exit 10 env -i PATH="$PATH" HOME="$HOME" MAJORDOMUS_BIN="$RB" \
  sh -c "cd '$R' && '$R/bin/majordomus' doctor"
expect_grep "FAIL worktree"
expect_grep "$W/outside"
[ "$(git -C "$R" worktree list --porcelain)" = "$before" ] || { echo "    doctor changed the topology"; exit 1; }
[ -d "$W/outside" ] || { echo "    doctor removed a worktree"; exit 1; }

# ---------------------------------------------------------------- 6. the plan changes nothing
expect_exit 0 mj "$R" worktree migrate --plan
expect_grep "$W/outside"
expect_grep "$R-wt/outside"
[ "$(git -C "$R" worktree list --porcelain)" = "$before" ] || { echo "    --plan changed the topology"; exit 1; }
[ -d "$R-wt/outside" ] && { echo "    --plan created the destination"; exit 1; }

# ---------------------------------------------------------------- 7. a dirty worktree is refused
echo "work in progress" > "$W/outside/untracked.txt"
expect_exit 0 mj "$R" worktree migrate --plan
expect_grep "BLOCKED"
expect_exit 10 mj "$R" worktree remove outside
expect_grep "dirty"
[ -d "$W/outside" ] || { echo "    a dirty worktree was removed"; exit 1; }
[ -f "$W/outside/untracked.txt" ] || { echo "    uncommitted work was lost"; exit 1; }
rm -f "$W/outside/untracked.txt"

# ---------------------------------------------------------------- 8. the primary is protected
expect_exit 10 mj "$R" worktree remove "$R"
expect_grep "primary checkout"
[ -f "$R/.ai/manifest.yaml" ] || { echo "    the primary checkout was damaged"; exit 1; }

# ---------------------------------------------------------------- 9. apply moves it
expect_exit 0 mj "$R" worktree migrate --apply
[ -d "$R-wt/outside" ] || { echo "    --apply did not move the worktree"; exit 1; }
[ -d "$W/outside" ] && { echo "    the old path survived the move"; exit 1; }
expect_exit 0 mj "$R" worktree list
expect_no_grep "FAIL"

# ---------------------------------------------------------------- 10. remove keeps the branch
expect_exit 0 mj "$R" worktree remove feature-y
[ -d "$R-wt/feature-y" ] && { echo "    the directory survived remove"; exit 1; }
git -C "$R" show-ref --verify --quiet refs/heads/feature-y \
  || { echo "    remove deleted the branch; the two lifecycles are separate"; exit 1; }

# ---------------------------------------------------------------- 11. prune is metadata only
rm -rf "$R-wt/outside"
expect_exit 0 mj "$R" worktree prune --dry-run
git -C "$R" worktree list --porcelain | grep -q "outside" \
  || { echo "    --dry-run pruned the record"; exit 1; }
expect_exit 0 mj "$R" worktree prune
git -C "$R" worktree list --porcelain | grep -q "outside" && { echo "    prune left the stale record"; exit 1; }

# ---------------------------------------------------------------- 12. the single source of truth
#      Only the policy changes here. No implementation file is touched, and the container moves.
perl -pi -e 's/^    suffix: "-wt"$/    suffix: "-trees"/' "$R/.ai/repo/policy.yaml"
grep -q 'suffix: "-trees"' "$R/.ai/repo/policy.yaml" || { echo "    the policy edit did not take"; exit 1; }
git -C "$R" add -A >/dev/null; git -C "$R" commit -qm policy >/dev/null
expect_exit 0 mj "$R" worktree root
[ "$LAST_OUT" = "$R-trees" ] || { echo "    after the policy edit the root is '$LAST_OUT', expected '$R-trees'"; exit 1; }
# and the worktree that was compliant a moment ago is now out of place
expect_exit 10 mj "$R" worktree list
expect_grep "FAIL"

echo "    worktree layout: derived, enforced, migrated, and driven by one policy value"
