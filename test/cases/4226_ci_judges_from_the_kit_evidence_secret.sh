# majordomus-covers: doctor init
# majordomus-negative: doctor init
# Public CI judges the declared kit from a licence-evidence secret, and fails closed without
# it (I4336 (d)). MAJORDOMUS_KIT_EVIDENCE_<KIT> carries base64(gzip(kit-evidence/v1)); the
# shell decodes it to a 0600 file and hands the executable its path, never the value.
#
#   1  set, in a public fixture declaring examplekit with a UI: doctor hands the decoded
#      evidence to `design licence`, mode 0600 and byte for byte the text, and reports the
#      verdict it returns, naming the file and the licensed source
#   2  unset there: doctor fails closed, whether the executable answers not_judged or has
#      no design licence at all
#   In every part that fails, doctor counts each FAIL it prints and exits non-zero: a FAIL
#   line that leaves the count alone is a gate that passes.
#   3  malformed: doctor fails naming the variable and never prints the value
#   4  private, or no kit declared: nothing fails for want of evidence
#   5  the same secret refuses a fleet pack file whose bytes are licensed, for a public tree;
#      without it the pack's kit is not judged and the pack is refused, never installed
#   6  with the executable of this tree, when it answers `design licence`: set judges the
#      page, unset fails closed
#
# Parts 1-5 use a stub executable: what is under test there is the shell's half of the seam
# (decode, hand over, read the verdict, fail closed). design.licence's own verdicts are its
# cases'.
. "$ROOT/test/lib.sh"

command -v gzip >/dev/null 2>&1 && command -v base64 >/dev/null 2>&1 || skip "no gzip or base64"
unset MAJORDOMUS_FLEET_PACK MAJORDOMUS_VISIBILITY MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT 2>/dev/null || true
export XDG_CONFIG_HOME="$T/xdg"
git remote add origin https://forge.example.test/owner/site.git
"$MJ" init >/dev/null 2>&1
printf '<button class="ek-btn">Go</button>\n' > index.html
git add index.html && git commit -qm 'a page'
sed -i.bak 's/^  kit: ""/  kit: examplekit/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
grep -q '^  kit: examplekit$' .ai/repo/policy.yaml || { echo "    the fixture policy declares no kit"; exit 1; }

# kit-evidence/v1: the page's exact and normalised hashes, as the owner's evidence holds them
exact="$(sha256_of_file index.html)"
norm="$(tr '\t\r\n' '   ' < index.html | tr -s ' ' | mj_sha256_hex)"
printf '# kit-evidence/v1\n%s\t%s\texamplekit/button.html\n' "$exact" "$norm" > evidence.txt
SECRET="$(gzip -c evidence.txt | base64 | tr -d '\n')"

# the stub executable: `design licence` reports what it was handed and answers as the real
# one does; every other subcommand is unknown to it
mkdir -p stub
cat > stub/majordomus <<'B'
#!/usr/bin/env bash
[ "${1:-} ${2:-}" = "design licence" ] || exit 2
shift 2; ev=""
while [ $# -gt 0 ]; do case "$1" in --evidence) ev="$2"; shift 2 ;; *) shift ;; esac; done
if [ -z "$ev" ]; then printf '{"verdict":"not_judged","why":"no licence evidence","matches":[]}\n'; exit 10; fi
# GNU first: GNU's `stat -f` is the file system's status, and it succeeds
{ stat -c %a "$ev" 2>/dev/null || stat -f %Lp "$ev"; } > "$STUB_SEEN.mode"
cp "$ev" "$STUB_SEEN.evidence"
printf '{"verdict":"refused","matches":[{"path":"index.html","source":"examplekit/button.html","kind":"exact"}]}\n'
exit 10
B
chmod +x stub/majordomus
export STUB_SEEN="$T/seen"
# doctor, with its exit and its own count of failures; DOC_BIN overrides the stub
doctor() { DOC_RC=0; MAJORDOMUS_BIN="${DOC_BIN:-$T/stub/majordomus}" "$MJ" doctor > doctor.out 2>&1 || DOC_RC=$?; }
# the last doctor failed, and counted every FAIL it printed
counted() {
  local said seen
  said="$(sed -n 's/^doctor: \([0-9][0-9]*\) failure(s)$/\1/p' doctor.out)"
  seen="$(grep -c '^FAIL' doctor.out || true)"
  [ "$said" = "$seen" ] && [ "$DOC_RC" != 0 ] \
    || { echo "    doctor printed $seen FAIL line(s), counted ${said:-none} and exited $DOC_RC"; return 1; }
}
# the licence FAILs raise doctor's count by exactly that many over the tree without a kit
kit_off() { sed -i.bak 's/^  kit: examplekit/  kit: ""/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak; }
kit_on()  { sed -i.bak 's/^  kit: ""/  kit: examplekit/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak; }
kit_off; doctor; BASE="$(sed -n 's/^doctor: \([0-9][0-9]*\) failure(s)$/\1/p' doctor.out)"; kit_on
raised() { # the number of licence FAILs the last doctor should have added
  local said; said="$(sed -n 's/^doctor: \([0-9][0-9]*\) failure(s)$/\1/p' doctor.out)"
  [ "$said" = $((BASE + $1)) ] || { echo "    doctor counted $said failure(s); $BASE without a kit, plus $1 licence FAIL(s)"; return 1; }
}

# ---------------------------------------------------------------- 1. set: judged
MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT="$SECRET" doctor
expect_grep 'FAIL licence +index\.html — is a licensed file of examplekit \(exact copy of examplekit/button\.html\); a public tree never carries it' doctor.out || exit 1
counted && raised 1 || exit 1
[ "$(cat "$STUB_SEEN.mode")" = 600 ] || { echo "    the decoded evidence was handed over with mode $(cat "$STUB_SEEN.mode")"; exit 1; }
cmp -s "$STUB_SEEN.evidence" evidence.txt || { echo "    the evidence handed over is not the text the secret carries"; exit 1; }
grep -qF -- "$SECRET" doctor.out && { echo "    doctor printed the secret"; exit 1; }

# ---------------------------------------------------------------- 2. unset: fails closed
doctor
expect_grep 'FAIL licence +examplekit — not judged: no licence evidence; a public tree that declares a kit fails closed \(set MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT in CI\)' doctor.out || exit 1
counted && raised 1 || exit 1
DOC_BIN="$T/stub/absent" doctor
expect_grep 'FAIL licence +examplekit — not judged: the executable answers no design licence' doctor.out || exit 1
counted || exit 1

# ---------------------------------------------------------------- 3. malformed: named, never printed
BAD="bm90LWV2aWRlbmNlLXNlY3JldC12YWx1ZQ"
MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT="$BAD" doctor
expect_grep 'FAIL licence +MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT — does not hold base64\(gzip\(kit-evidence/v1\)\)' doctor.out || exit 1
counted && raised 1 || exit 1
grep -qF -- "$BAD" doctor.out && { echo "    doctor printed the malformed value"; exit 1; }
# a well-formed gzip of text that is not evidence is malformed too
MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT="$(printf 'not evidence\n' | gzip -c | base64 | tr -d '\n')" doctor
expect_grep 'FAIL licence +MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT — does not hold' doctor.out || exit 1

# ---------------------------------------------------------------- 4. private, or no kit
sed -i.bak 's/^  visibility: public/  visibility: private/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
doctor
expect_grep 'INFO licence +examplekit — not judged: no licence evidence' doctor.out || exit 1
expect_no_grep 'FAIL licence' doctor.out || exit 1
# a private declaration is tightened by the environment, never loosened
MAJORDOMUS_VISIBILITY=public doctor
expect_grep 'FAIL licence +examplekit — not judged' doctor.out || exit 1
counted || exit 1
sed -i.bak -e 's/^  visibility: private/  visibility: public/' -e 's/^  kit: examplekit/  kit: ""/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
doctor
expect_no_grep 'licence' doctor.out || exit 1

# ---------------------------------------------------------------- 5. the secret guards the pack
P="$XDG_CONFIG_HOME/majordomus/fleet"; mkdir -p "$P/kits"
cp index.html "$P/kits/examplekit.yaml"
printf 'schema: ai-fleet-pack/v1\nversion: 1\napplies_to:\n  remotes: ["*example.test/owner/*"]\nentries:\n  - { path: kits/examplekit.yaml, kind: kit }\n' > "$P/manifest.yaml"
MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT="$SECRET" "$MJ" rules vendor update > update.out 2>&1 && { echo "    a licensed pack file was installed into a public tree"; exit 1; }
expect_grep 'refused kits/examplekit\.yaml: is a licensed file \(exact copy of examplekit/button\.html\)' update.out || exit 1
[ ! -e .ai/repo/rules/vendor/fleet ] || { echo "    the refused pack wrote into the tree"; exit 1; }
grep -qF -- "$SECRET" update.out && { echo "    the refusal printed the secret"; exit 1; }
# no evidence for the pack's kit: not judged, so not installed, whatever its bytes are
printf 'id: examplekit\n' > "$P/kits/examplekit.yaml"
expect_exit 15 "$MJ" rules vendor update || exit 1
expect_grep 'refused: not judged: a kit is in play \(MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT\) and no licence evidence is here' || exit 1
[ ! -e .ai/repo/rules/vendor/fleet ] || { echo "    an unjudged pack wrote into the tree"; exit 1; }
# with the evidence, the same harmless profile is judged and installed, and the decoded
# evidence is gone afterwards (a temporary directory of its own, so no other case's is seen)
mkdir -p "$T/tmpd"
MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT="$SECRET" TMPDIR="$T/tmpd" expect_exit 0 "$MJ" rules vendor update || exit 1
[ -f .ai/repo/rules/vendor/fleet/kits/examplekit.yaml ] || { echo "    the judged profile was not installed"; exit 1; }
[ -z "$(ls -A "$T/tmpd")" ] || { echo "    the install left behind: $(ls -A "$T/tmpd")"; exit 1; }

# ---------------------------------------------------------------- 6. the real executable
# The same seam with the executable of this tree, when it answers `design licence`: the
# secret judges the page, and without it the public tree fails closed.
rm -rf "$XDG_CONFIG_HOME/majordomus" .ai/repo/rules/vendor/fleet
kit_on
RB="$(rust_bin 2>/dev/null)" || RB=""
{ [ -n "$RB" ] && "$RB" design licence --help >/dev/null 2>&1; } \
  || skip_case "the executable answers no design licence, so its verdict through doctor was not measured"
MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT="$SECRET" DOC_BIN="$RB" doctor
expect_grep 'FAIL licence +index\.html — is a licensed file of examplekit \(exact copy of examplekit/button\.html\)' doctor.out || exit 1
counted || exit 1
grep -qF -- "$SECRET" doctor.out && { echo "    doctor printed the secret"; exit 1; }
DOC_BIN="$RB" doctor
expect_grep 'FAIL licence +examplekit — not judged' doctor.out || exit 1
counted || exit 1
exit 0
