# majordomus-covers: none
# A workspace record says how it was acquired, and who it is never depends on its title (I1302).
#
# A record synced from an external workspace (ADR 0032) is a raw observation, and one that does
# not say how it was got cannot be told apart from a guess. workspace-record/v1 types it, and
# scripts/lib/workspace.mjs, the schema's first reader, applies the schema from the file rather
# than restating it. The bytes came back from 0c5419192f2 (feature/workspace-browser-transport),
# which never reached master. This case holds the five things I1302 asks of them, without a
# browser and without the network:
#
#   1  identity is <vendor>:<type>:<upstream_id>: stable across a rename, distinct across ids
#      and across types that share one upstream id, never a function of the title
#   2  transport, acquisition and support are enumerations; `export` (ADR 0212, I3279) is one
#   3  the content hash is a fact about content: key order does not move it, a changed body does
#   4  a record gone upstream carries a tombstone with a time and a reason; a tombstone with no
#      time, a time that is not one, or an unknown reason is refused
#   5  the schema carries its version: an unknown version is refused, a closed record refuses
#      an unknown key, and the reader refuses a schema file that names another identifier
#
# Every check is an `if`, never `cmd && { ...; exit 1; }`: the case runs under `bash -eu`.
. "$ROOT/test/lib.sh"
command -v node >/dev/null || skip "node is absent, and scripts/lib/workspace.mjs runs under node"

SCHEMA="$ROOT/share/schemas/majordomus/workspace-record/workspace-record.v1.schema.json"
expect_file "$SCHEMA"

out=""
rc=0
out="$(cd "$ROOT" && node --input-type=module -e '
import { makeRecord, validate, contentHash, schema } from "./scripts/lib/workspace.mjs";

const say = (id, ok, detail) => console.log(`${ok ? "ok" : "NO"} ${id}${detail ? " — " + detail : ""}`);
const refuses = (id, fn, needle) => {
  let threw = null;
  try { fn(); } catch (e) { threw = e; }
  if (!threw) return say(id, false, "was accepted");
  say(id, threw.message.includes(needle), threw.message.split("\n").slice(0, 2).join(" | "));
};
const finds = (id, problems, needle) =>
  say(id, problems.some((p) => p.includes(needle)), problems.join("; ") || "accepted it");

const base = { workspace: "w", vendor: "chatgpt", type: "conversation", transport: "browser", acquisition: "browser_observed", support: "browser_observed" };

// 1. identity
const a = makeRecord({ ...base, upstreamId: "c-1", title: "Design", body: { parts: [] } });
const renamed = makeRecord({ ...base, upstreamId: "c-1", title: "Renamed upstream", body: { parts: [] } });
const other = makeRecord({ ...base, upstreamId: "c-2", title: "Design", body: { parts: [] } });
const sibling = makeRecord({ ...base, type: "instructions", upstreamId: "c-1", body: { text: "x" } });
say("identity-stable", a.id === renamed.id, `${a.id} vs ${renamed.id}`);
say("identity-distinct", a.id !== other.id, `${a.id} vs ${other.id}`);
say("identity-typed", a.id !== sibling.id, `${a.id} vs ${sibling.id}`);
say("identity-shape", a.id === "chatgpt:conversation:c-1", a.id);

// 2. typed provenance
refuses("bad-transport", () => makeRecord({ ...base, transport: "carrier-pigeon", upstreamId: "c-3" }), "must be one of");
refuses("bad-acquisition", () => makeRecord({ ...base, acquisition: "rumour", upstreamId: "c-4" }), "must be one of");
refuses("bad-support", () => makeRecord({ ...base, support: "hearsay", upstreamId: "c-5" }), "must be one of");
const exported = makeRecord({ ...base, transport: "official", acquisition: "export", support: "official", upstreamId: "c-6" });
say("export-accepted", exported.acquisition === "export");

// 3. content hash
say("hash-stable", contentHash({ x: 1, y: [2, 3] }) === contentHash({ y: [2, 3], x: 1 }));
const changed = makeRecord({ ...base, upstreamId: "c-1", title: "Design", body: { parts: ["edited"] } });
say("hash-changes", changed.content_hash !== a.content_hash && changed.id === a.id, "same identity, new content");

// 4. tombstone
say("tombstone-ok", validate({ ...a, tombstone: { at: "2026-10-21T13:00:00Z", reason: "gone" } }).length === 0);
finds("tombstone-no-time", validate({ ...a, tombstone: { reason: "gone" } }), "missing at");
finds("tombstone-bad-time", validate({ ...a, tombstone: { at: "yesterday", reason: "gone" } }), "not an RFC 3339 date-time");
finds("tombstone-bad-reason", validate({ ...a, tombstone: { at: "2026-10-21T13:00:00Z", reason: "vanished" } }), "must be one of");
finds("tombstone-closed", validate({ ...a, tombstone: { at: "2026-10-21T13:00:00Z", reason: "gone", by: "x" } }), "does not allow");

// 5. version
finds("unknown-version", validate({ ...a, schema: "workspace-record/v2" }), "must be \"workspace-record/v1\"");
finds("unknown-field", validate({ ...a, invented: 1 }), "does not allow");
finds("bad-seen", validate({ ...a, first_seen: "a while ago" }), "not an RFC 3339 date-time");
const s = schema();
say("schema-id", s["x-majordomus-schema"] === "majordomus.workspace-record/v1" && s.additionalProperties === false);
' 2>&1)" || rc=$?
if [ "$rc" != 0 ]; then
  echo "    the node run failed:"; echo "$out" | sed 's/^/      /'; exit 1
fi
echo "$out" | sed 's/^/      /'
if echo "$out" | grep -q '^NO '; then
  echo "    the record contract does not hold what I1302 asks"; exit 1
fi
# every assertion the run was asked for, so a silently shortened run is not a pass
n="$(echo "$out" | grep -c '^ok ' || true)"
if [ "$n" != "19" ]; then
  echo "    expected 19 assertions, got $n"; exit 1
fi

# --- the reader refuses a schema file that names another identifier
mkdir -p "$T/repo/scripts/lib" "$T/repo/share/schemas/majordomus/workspace-record"
cp "$ROOT/scripts/lib/workspace.mjs" "$T/repo/scripts/lib/"
sed 's#majordomus.workspace-record/v1#majordomus.workspace-record/v9#' "$SCHEMA" \
  > "$T/repo/share/schemas/majordomus/workspace-record/workspace-record.v1.schema.json"
rc=0
msg="$(cd "$T/repo" && node --input-type=module -e 'import { schema } from "./scripts/lib/workspace.mjs"; schema();' 2>&1)" || rc=$?
if [ "$rc" = 0 ]; then
  echo "    the reader accepted a schema file that declares another identifier"; exit 1
fi
if ! printf '%s' "$msg" | grep -q 'this reader applies majordomus.workspace-record/v1'; then
  echo "    the refusal does not name the identifier it applies: $msg"; exit 1
fi

# --- the store is never tracked
if ! git -C "$ROOT" check-ignore -q .ai/local/workspaces; then
  echo "    .ai/local/workspaces is not ignored; synced content would be committed"; exit 1
fi

echo "    a workspace record says how it was acquired, and its identity never depends on its title"
