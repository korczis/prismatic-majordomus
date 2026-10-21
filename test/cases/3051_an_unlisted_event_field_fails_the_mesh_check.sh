# majordomus-covers: none
# An EventBody field nobody listed fails scripts/ci/mesh-check (I2708, eventbody_allowlist_case).
#
# The mesh signs every event body into the journal and replicates it, in clear, to every
# linked runtime. A field added to `EventBody` — or to a struct an event carries, such as
# `SessionInfo` or `HandoverBody` — reaches every machine of the mesh the day it merges, and
# until this case nothing asked whether anyone had decided it may. The decision is now a
# line in .ai/repo/ci/mesh-event-fields.yaml: every field the wire carries is named there,
# and each says whether its value is redacted before it is signed (`redacted`) or travels
# as written (`clear`).
#
# The case drives the gate both ways. On the real tree it passes: every field of every
# variant is listed, and nothing listed is gone. On a copy of the tree it fails for each
# way the list and the source can come apart — a variant field nobody listed, a field of a
# carried struct nobody listed, a whole new variant, an entry that does not say redacted or
# clear, an entry for a field the source no longer has, and a journal the gate cannot read
# at all — and passes again once the list names the new field, which is what proves the
# list, not the source, decides.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/mesh-check"
LIST=".ai/repo/ci/mesh-event-fields.yaml"
JOURNAL="apps/majordomus-cli/src/mesh/journal.rs"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }
[ -f "$ROOT/$LIST" ] || { echo "    $LIST is missing: the allow-list is the subject of this case"; exit 1; }

# ---------------------------------------------------------------- the real tree passes
expect_exit 0 env MJ_ROOT="$ROOT" "$GATE"
expect_grep 'mesh-check: OK .*every EventBody field listed'
# the one field the mesh redacts today is the handover body (mesh/handover.rs, before it is
# signed); the list must say so rather than call it clear
expect_grep '^  HandoverBody\.body: redacted$' "$ROOT/$LIST"

# ---------------------------------------------------------------- a copy of the tree
# Everything the gate reads, copied once: the crate's source, the declaration's kind,
# schema, rules and skeleton class, and the list. Each step below edits the copy's journal
# or list and puts the original back afterwards.
F="$T/tree"
mkdir -p "$F/apps/majordomus-cli" "$F/share/schemas/majordomus" "$F/share/skeleton/ai/repo/knowledge" \
         "$F/.ai/repo/rules/project" "$F/.ai/repo/ci"
cp -R "$ROOT/apps/majordomus-cli/src" "$F/apps/majordomus-cli/src"
cp "$ROOT/share/kinds.yaml" "$F/share/kinds.yaml"
cp -R "$ROOT/share/schemas/majordomus/mesh-declaration" "$F/share/schemas/majordomus/"
cp "$ROOT/share/skeleton/ai/repo/knowledge/sources.yaml" "$F/share/skeleton/ai/repo/knowledge/"
cp "$ROOT"/.ai/repo/rules/project/mesh-*.md "$F/.ai/repo/rules/project/"
cp "$ROOT/$LIST" "$F/$LIST"
cp "$F/$JOURNAL" "$T/journal.rs.orig"
cp "$F/$LIST" "$T/list.orig"
restore() { cp "$T/journal.rs.orig" "$F/$JOURNAL"; cp "$T/list.orig" "$F/$LIST"; }

# the copy is the real tree: it passes before anything is changed
expect_exit 0 env MJ_ROOT="$F" "$GATE"

# insert_after <pattern> <line>: one line into the copy's journal, after the first match
insert_after() {
  # through the environment, not -v: awk refuses a newline inside a -v value
  PAT="$1" ADD="$2" awk '{ print } !done && index($0, ENVIRON["PAT"]) { print ENVIRON["ADD"]; done = 1 }' \
    "$F/$JOURNAL" > "$T/journal.new"
  cmp -s "$F/$JOURNAL" "$T/journal.new" && { echo "    the fixture edit did not apply: $1"; exit 1; }
  mv "$T/journal.new" "$F/$JOURNAL"
}

# ---------------------------------------------------------------- 1. a variant field
# The violation the issue names: a field added to an event variant, which every linked
# runtime would receive signed and in clear, with no line saying anyone decided it may.
insert_after '    ReviewAnswered {' '        token: Option<String>,'
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'mesh-check: FAIL EventBody\.ReviewAnswered\.token is carried by the mesh and not listed in \.ai/repo/ci/mesh-event-fields\.yaml'
expect_grep 'redacted or clear'

# listed, it passes: the list is what decides, not the shape of the source
printf '  EventBody.ReviewAnswered.token: redacted\n' >> "$F/$LIST"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
restore

# ---------------------------------------------------------------- 2. a carried struct's field
# A field of SessionInfo travels inside every session_opened event: adding it there is
# adding it to the wire just as surely, so the gate follows the types the variants carry.
insert_after 'pub struct SessionInfo {' '    pub home: Option<String>,'
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'mesh-check: FAIL SessionInfo\.home is carried by the mesh and not listed'
restore

# ---------------------------------------------------------------- 3. a new variant
insert_after 'pub enum EventBody {' '    NoteShared {
        text: String,
    },'
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'mesh-check: FAIL EventBody\.NoteShared\.text is carried by the mesh and not listed'
restore

# ---------------------------------------------------------------- 4. an entry that decides nothing
# Every entry says one of two words; anything else is a field nobody actually decided.
awk '{ if ($1 == "HandoverBody.body:") print "  HandoverBody.body: maybe"; else print }' \
  "$T/list.orig" > "$F/$LIST"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'mesh-check: FAIL \.ai/repo/ci/mesh-event-fields\.yaml lists HandoverBody\.body as "maybe"'
restore

# ---------------------------------------------------------------- 5. a stale entry
# A list that still names a field the source dropped is a list nobody reads against the
# source; the next reader trusts a line that describes nothing. One-sided, the gate would
# let the list drift into fiction.
printf '  EventBody.ClaimAcquired.lease: clear\n' >> "$F/$LIST"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'mesh-check: FAIL \.ai/repo/ci/mesh-event-fields\.yaml lists EventBody\.ClaimAcquired\.lease, which the mesh no longer carries'
restore

# ---------------------------------------------------------------- 6. a journal the gate cannot read
# A gate that cannot find what it measures must not report the tree clean.
sed 's/pub enum EventBody {/pub enum EventPayload {/' "$T/journal.rs.orig" > "$F/$JOURNAL"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'mesh-check: FAIL .*declares no EventBody'
restore

rm -f "$F/$LIST"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'mesh-check: FAIL \.ai/repo/ci/mesh-event-fields\.yaml is missing'
restore

# and the copy, restored, is the tree it accepted at the start
expect_exit 0 env MJ_ROOT="$F" "$GATE"

echo "    an EventBody field nobody listed, or a list entry nothing carries, fails the mesh check"
