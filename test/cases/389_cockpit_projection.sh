# majordomus-covers: none
# The gate of ADR 0012's "no page reads the index directly", driven against fixture trees
# rather than against this checkout. A case that asserted only this repository's own
# Cockpit would pass the day somebody added a read and adjusted the assertion in the same
# commit; what is asserted here is the gate's behaviour — what it refuses, what it
# deliberately does not refuse, and what it says while refusing.
#
# The refusals are planted, because a check nobody has watched fail is a check nobody
# knows works: a page that counts the index's objects, a page that hands the index to
# something else, and a test module that compares a page against the index are each
# written into a clean tree and the gate is asked.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/cockpit-projection-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }

# A tree with the shape the gate reads: a Cockpit that asks capabilities for everything,
# and — outside the Cockpit — a capability handler and a generator that do read the index,
# because that is where reading it is the job.
fixture() {
  local f="$T/tree$1"
  rm -rf "$f"
  mkdir -p "$f/apps/majordomus-cli/src/cockpit" "$f/apps/majordomus-cli/src/capability/builtin"
  cat > "$f/apps/majordomus-cli/src/cockpit/nav.rs" <<'RS'
//! The kinds are asked for, never read: ctx.index is what ADR 0012 forbids here.
pub fn build(ctx: &Context) -> Navigation {
    let held = ctx.execute("repository.info", json!({})).unwrap();
    Navigation::of(held["kinds"].clone())
}
RS
  cat > "$f/apps/majordomus-cli/src/cockpit/pages.rs" <<'RS'
pub fn objects(ctx: &Context) -> Page {
    let list: ObjectList = ask(ctx, "objects.list", json!({})).unwrap();
    Page::new(list.count)
}
RS
  cat > "$f/apps/majordomus-cli/src/capability/builtin/objects.rs" <<'RS'
fn objects_list(ctx: &Context, _: ListInput) -> Result<ObjectList> {
    Ok(ObjectList { count: ctx.index.objects.len() })
}
RS
  printf 'fn generate(ctx: &Context) { let _ = ctx.index.kinds(); }\n' \
    > "$f/apps/majordomus-cli/src/generate.rs"
  printf '%s' "$f"
}

# --- a Cockpit that asks passes, and the gate says what it measured
F="$(fixture 0)"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'ok   cockpit-projection'
expect_grep 'no page reads the index directly'

# --- what it does NOT refuse. The capability handler and the generator read the index in
# the clean tree above: that is their job, and a gate that flagged them would be turned off
# within a week. The comment in nav.rs names ctx.index and is not a read either.
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_no_grep 'builtin/objects.rs'
expect_no_grep 'generate.rs'

# --- a page that counts the index's objects is refused, by file and line
F="$(fixture 1)"
cat > "$F/apps/majordomus-cli/src/cockpit/nav.rs" <<'RS'
pub fn build(ctx: &Context) -> Navigation {
    Navigation::of(ctx.index.objects.len())
}
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'cockpit/nav.rs:2'
expect_grep 'reads the index directly'
# and it says what to ask instead, rather than only that something is wrong
expect_grep 'repository.info'
expect_grep 'objects.list'
expect_grep 'ADR 0012'

# --- a page that hands the index to something else is refused too: passing it on is
# reading it, and this is the shape the case-provider call had
F="$(fixture 2)"
cat > "$F/apps/majordomus-cli/src/cockpit/pages.rs" <<'RS'
pub fn capability(ctx: &Context) -> Page {
    let cases = provider(&CaseContext { index: &ctx.index });
    Page::new(cases)
}
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'cockpit/pages.rs:2'

# --- a test module of a Cockpit file is held to the same rule: an assertion that compares
# a page against the index proves the page agrees with a source the page may not read, so
# it keeps passing after the page starts disagreeing with the capability
F="$(fixture 3)"
cat >> "$F/apps/majordomus-cli/src/cockpit/nav.rs" <<'RS'
#[cfg(test)]
mod tests {
    #[test]
    fn the_kinds_are_the_indexs() {
        assert_eq!(build(&ctx).kinds(), ctx.index.kinds().into_keys().collect::<Vec<_>>());
    }
}
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'cockpit/nav.rs'

# --- a tree with no Cockpit is unusable, not clean: a gate that reports "ok" when its
# subject is missing is the shape of a check that silently stops measuring
F="$(fixture 4)"
rm -rf "$F/apps/majordomus-cli/src/cockpit"
expect_exit 12 env MJ_ROOT="$F" "$GATE"
expect_grep 'no Cockpit at'

# --- and this repository's own Cockpit passes, which is the state the gate defends
expect_exit 0 "$GATE"
expect_grep 'no page reads the index directly'
