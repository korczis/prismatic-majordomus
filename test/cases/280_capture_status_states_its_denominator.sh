# majordomus-covers: none
# `capture status` names the providers it did not examine, and why.
#
# The distribution declares six providers and ships a capture adapter for one of them, so
# the listing is two rows: `claude-code` and `claude-code:session`, the prompt and lifecycle
# wirings of the same provider. That is every row there can be — and with no denominator
# printed, it reads as a listing of what exists rather than of what was examined.
#
# On 2026-09-12 that misled a session badly enough to be repeated to two others and to enter
# a work queue as "four providers are not reported at all, ~120 lines to fix". There were no
# missing providers: five of the six declare neither `lifecycle` nor `prompt_capture`, and
# `share/providers.yaml` says three lines above its own mapping what that means — a provider
# with no lifecycle is one the tool ships no adapter for, which costs its worker the
# automation and none of the model. The measurement behind the claim was true; the inference
# from it was not, and nothing in the output contradicted it.
#
# So this case proves the denominator is stated, on both surfaces, and that it is *derived*:
#
#   - the text listing names every declared provider that has no adapter
#   - `--json` carries the same set, because a verdict that states its subject on one
#     projection and not the other is half a verdict on the surface that omits it
#   - the set comes from `share/providers.yaml`, not from a list in the shell: a second
#     hand-maintained copy of the provider set is the defect this line exists to report, so
#     a provider added to the distribution must appear here without anybody editing lib/
#
# The rule is project.a-verdict-states-its-subject.
. "$ROOT/test/lib.sh"

MJ="$ROOT/bin/majordomus"
expect_file "$ROOT/lib/capture.sh"
expect_file "$ROOT/share/providers.yaml"

# ------------------------------------------------------- the text surface names the rest
out="$T/status.txt"
( cd "$ROOT" && env -u MAJORDOMUS_SHARE "$MJ" capture status ) > "$out" 2>&1 || true
expect_grep "no adapter" "$out"
expect_grep "agents" "$out"
expect_grep "generic" "$out"
# the adapted provider is still reported as itself, not folded into the summary line
expect_grep "^claude-code +" "$out"

# ------------------------------------------------------------ the json surface agrees
js="$T/status.json"
( cd "$ROOT" && env -u MAJORDOMUS_SHARE "$MJ" --json capture status ) 2>/dev/null | tail -1 > "$js" || true
python3 - "$js" <<'PY' || exit 1
import json, sys
d = json.load(open(sys.argv[1]))
un = d.get("unadapted")
assert isinstance(un, list), "the json surface carries no unadapted set"
assert "agents" in un and "generic" in un, f"unadapted is missing declared providers: {un}"
assert not any(p["provider"] in un for p in d["providers"]), \
    "a provider cannot be both adapted and unadapted"
PY

# ------------------------------------ the set is derived, not listed: add one and see it
# A provider declared in the distribution with no lifecycle and no prompt_capture must
# appear without anybody editing lib/. Done against a copy of the share so the checkout the
# suite runs in is never written to.
SHARE="$T/share"
cp -R "$ROOT/share" "$SHARE"
printf '  zz-probe:\n    title: A provider invented by case 280\n' >> "$SHARE/providers.yaml"
out2="$T/status-probe.txt"
( cd "$ROOT" && MAJORDOMUS_SHARE="$SHARE" "$MJ" capture status ) > "$out2" 2>&1 || true
expect_grep "zz-probe" "$out2"
