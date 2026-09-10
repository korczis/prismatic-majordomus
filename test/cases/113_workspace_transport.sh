# majordomus-covers: none
# The workspace transport: one browser, a named profile, and a record that says how it was got.
#
# ADR 0025 put the authenticated half of this repository in the Node tooling layer, beside
# the UI audit and the Cockpit probe, because those already drive the system Chrome. The
# risk that arrangement creates is the one this case is about: the transport and the audits
# share a launcher, and only one of them may ever hold a signed-in profile.
#
# So the assertions are refusals more than behaviours. The parts that need a browser are
# not tested here; the parts that decide *which* browser, *whose* session and *what may be
# written down* are, and those are the parts that would be expensive to get wrong.
#
# Every check is an `if`, never `cmd && { ...; exit 1; }`: the case runs under `bash -eu`,
# where an && list that fails as a whole ends the run having printed nothing at all.
#
# It reads; it writes nothing outside its own temporary directory.
. "$ROOT/test/lib.sh"
command -v node >/dev/null || { echo "    node absent; skipping"; exit 0; }
[ -d "$ROOT/node_modules/playwright" ] || { echo "    playwright absent; skipping"; exit 0; }

SCHEMA="$ROOT/share/schemas/majordomus/workspace-record/workspace-record.v1.schema.json"

# --- the contract exists where its identifier says, and is closed
expect_file "$SCHEMA"
if command -v jq >/dev/null; then
  if [ "$(jq -r '."x-majordomus-schema"' "$SCHEMA")" != "majordomus.workspace-record/v1" ]; then
    echo "    the record schema does not declare its own identifier"; exit 1
  fi
  if [ "$(jq -r '.additionalProperties' "$SCHEMA")" != "false" ]; then
    echo "    the record schema admits unknown keys"; exit 1
  fi
fi

# --- one launcher: neither audit may open its own browser again
#
# The defect this prevents is not a duplicated line. It is the next consumer copying
# whichever launch it found first and getting the profile-carrying one by accident.
for f in "$ROOT/scripts/lib/ui-audit.mjs" "$ROOT/scripts/lib/cockpit-probe.mjs"; do
  if grep -q "chromium.launch" "$f"; then
    echo "    $(basename "$f") still launches its own browser; the launcher is scripts/lib/browser.mjs"; exit 1
  fi
  if ! grep -q "launchEphemeral" "$f"; then
    echo "    $(basename "$f") does not use the shared launcher"; exit 1
  fi
done

# --- the refusals of the profile resolver, and the record contract, in one node run
out=""
rc=0
out="$(cd "$ROOT" && MAJORDOMUS_BROWSER_PROFILES="$T/profiles.json" node --input-type=module -e '
import { writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { resolveProfileDir, observe, _internals } from "./scripts/lib/browser.mjs";
import { makeRecord, validate, contentHash } from "./scripts/lib/workspace.mjs";

const file = process.env.MAJORDOMUS_BROWSER_PROFILES;
const say = (id, ok, detail) => console.log(`${ok ? "ok" : "NO"} ${id}${detail ? " — " + detail : ""}`);
const refuses = (id, fn, needle) => {
  let threw = null;
  try { fn(); } catch (e) { threw = e; }
  if (!threw) return say(id, false, "was accepted");
  say(id, threw.message.includes(needle), threw.message.split("\n")[0]);
};

// no mapping at all: the message names the file and does not invent a default
writeFileSync(file, "{}");
refuses("empty-map", () => resolveProfileDir("chatgpt:default"), "maps no profile");

// the browser profile Chrome itself keeps, refused for two independent reasons
const os = _internals.osChromeProfiles()[0];
writeFileSync(file, JSON.stringify({ "chatgpt:default": `${os}/Default` }));
refuses("os-profile", () => resolveProfileDir("chatgpt:default"), "Chrome");

// a relative path is not a profile
writeFileSync(file, JSON.stringify({ "chatgpt:default": "somewhere" }));
refuses("relative", () => resolveProfileDir("chatgpt:default"), "not an absolute path");

// a directory of its own is what it wants
writeFileSync(file, JSON.stringify({ "chatgpt:default": `${homedir()}/.local/share/majordomus/chrome/chatgpt` }));
say("own-dir", resolveProfileDir("chatgpt:default").endsWith("/chrome/chatgpt"));

// an empty observation scope is a bug, not a quiet no-op
refuses("empty-scope", () => observe({ on() {} }, []), "at least one origin");

// the headers that are never written down, whatever case they arrive in
const red = _internals.redact({ Authorization: "Bearer x", "Set-Cookie": "s=1", "Content-Type": "application/json" });
say("redaction", !("authorization" in red) && !("set-cookie" in red) && red["content-type"] === "application/json", JSON.stringify(red));

// identity comes from the vendor and the upstream id, never from a title
const base = { workspace: "w", vendor: "chatgpt", type: "conversation", transport: "browser", acquisition: "browser_observed", support: "browser_observed" };
const a = makeRecord({ ...base, upstreamId: "c-1", title: "Design", body: { parts: [] } });
const b = makeRecord({ ...base, upstreamId: "c-1", title: "Renamed upstream", body: { parts: [] } });
const c = makeRecord({ ...base, upstreamId: "c-2", title: "Design", body: { parts: [] } });
say("identity-stable", a.id === b.id, `${a.id} vs ${b.id}`);
say("identity-distinct", a.id !== c.id, `${a.id} vs ${c.id}`);

// the same content twice is the same hash, in whatever key order it arrived
say("hash-stable", contentHash({ x: 1, y: [2, 3] }) === contentHash({ y: [2, 3], x: 1 }));

// the contract is applied, and applied from the file rather than restated here
const problems = validate({ ...a, invented: 1 });
say("unknown-field", problems.some((p) => p.includes("does not allow")), problems.join("; ") || "accepted it");
refuses("bad-transport", () => makeRecord({ ...base, transport: "carrier-pigeon", upstreamId: "c-3" }), "must be one of");
refuses("bad-type", () => makeRecord({ ...base, type: "rumour", upstreamId: "c-4" }), "must be one of");
' 2>&1)" || rc=$?
if [ "$rc" != 0 ]; then
  echo "    the node run failed:"
  echo "$out" | sed 's/^/      /'
  exit 1
fi

echo "$out" | sed 's/^/      /'
if echo "$out" | grep -q '^NO '; then
  echo "    the transport does not refuse what it must"
  exit 1
fi

# every assertion the run was asked for, so a silently shortened run is not a pass
n="$(echo "$out" | grep -c '^ok ' || true)"
if [ "$n" != "12" ]; then
  echo "    expected 12 assertions from the transport, got $n"; exit 1
fi

# --- the store is never tracked
if ! git -C "$ROOT" check-ignore -q .ai/local/workspaces; then
  echo "    .ai/local/workspaces is not ignored; synced content would be committed"; exit 1
fi
