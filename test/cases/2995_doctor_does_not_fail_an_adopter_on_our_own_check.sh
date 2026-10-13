# majordomus-covers: doctor
# claims: review-is-never-fabricated
# `reasoning check` holds a repository to the reasoning design, and four of its families
# are rules about the design's own home: what its code may name, what its CI may depend
# on, what its documents may claim. Run in an adopter's repository they judged the
# adopter's product instead — an archived workflow that names a model credential, an
# architecture decision that says the product requires a local model — and `doctor`
# exited 10 on a repository that had done nothing wrong, with a reproduce hint naming a
# path only this repository has.
#
# What it must not become:
#
#   a check that stopped running. The same two files in the repository that authors the
#   advisor catalogue are refused exactly as before, and an adopter is still held to what
#   it adopted: a reasoning record that does not parse fails there too.
#
#   a silent narrowing. The answer names the families that ran, so "no finding" never
#   covers a family that was not asked.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

# the two files, as the adopter had them
plant_product() {   # plant_product ROOT
  mkdir -p "$1/.github/workflows/archive" "$1/docs/adrs"
  printf 'jobs:\n  review:\n    env:\n      A: ${{ secrets.ANTHROPIC_API_KEY }}\n      O: ${{ secrets.OPENAI_API_KEY }}\n' > "$1/.github/workflows/archive/ci.yml"
  printf '# ADR 0023: local model integration\n\nThe ingestion pipeline requires Ollama on every analyst workstation.\n' > "$1/docs/adrs/adr-0023-ollama-model-integration-architecture.md"
}

# ---------------------------------------------------------------- an adopter: its product is not judged
AD="$T/adopter"; mkdir -p "$AD"
git -C "$AD" init -q .
git -C "$AD" config user.email t@example.com
git -C "$AD" config user.name t
( cd "$AD" && "$MJ" init >/dev/null && "$MJ" update >/dev/null )
plant_product "$AD"
git -C "$AD" add -A >/dev/null && git -C "$AD" commit -qm adopter >/dev/null
[ ! -e "$AD/share" ] || { echo "    the adopter fixture carries a share directory of its own"; exit 1; }
ad() { ( cd "$AD" && MAJORDOMUS_SHARE="$ROOT/share" "$RB" "$@" ); }

out="$(ad reasoning check --format json 2>/dev/null)" || { echo "    reasoning check fails an adopter on its own files:"; ad reasoning check 2>&1 | grep -v ' INFO ' | tail -5; exit 1; }
[ "$(jq -r '.ok' <<<"$out")" = true ] || { echo "    the adopter's answer is not ok: $out"; exit 1; }
[ "$(jq -r '.checks | join(" ")' <<<"$out")" = "catalogue records" ] \
  || { echo "    the answer does not name the families that ran: $(jq -c '.checks' <<<"$out")"; exit 1; }

# doctor: no reasoning failure, the families named, and no hint an adopter cannot run
( cd "$AD" && MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$ROOT/share" "$MJ" doctor > "$T/doctor.txt" 2>&1 ) || true
grep -q '^FAIL reasoning' "$T/doctor.txt" && { echo "    doctor fails the adopter on the reasoning design's own rules:"; grep '^FAIL reasoning' "$T/doctor.txt"; exit 1; }
grep -Eq '^OK +reasoning +- — reasoning check: catalogue, records — no finding$' "$T/doctor.txt" \
  || { echo "    doctor does not say which families ran:"; grep -i reasoning "$T/doctor.txt"; exit 1; }
grep -q 'bin/majordomus-cli' "$T/doctor.txt" && { echo "    doctor prints a hint naming a path only this repository has:"; grep 'bin/majordomus-cli' "$T/doctor.txt" | head -3; exit 1; }

# what the adopter did adopt is still held: a record of its own that does not parse
state="$AD/.ai/local/state/reasoning/t-1"; mkdir -p "$state"
printf '{ not a record' > "$state/assessment-broken.json"
ad reasoning check --format json > "$T/ad.json" 2>/dev/null && { echo "    an unreadable record of the adopter's own passed"; exit 1; }
[ "$(jq -r '[.findings[].check] | unique | join(" ")' "$T/ad.json")" = records ] \
  || { echo "    the adopter's own record was not the finding: $(cat "$T/ad.json")"; exit 1; }
( cd "$AD" && MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$ROOT/share" "$MJ" doctor > "$T/doctor.txt" 2>&1 ) || true
grep -Eq '^FAIL reasoning .*records: a reasoning record that does not parse +\[reproduce: majordomus reasoning check\]$' "$T/doctor.txt" \
  || { echo "    doctor's finding does not carry a hint the adopter can run:"; grep -i '^FAIL reasoning' "$T/doctor.txt"; exit 1; }
rm -r "$AD/.ai/local/state/reasoning"

# ---------------------------------------------------------------- the design's home: the same two files are refused
reasoning_fixture
[ -f "$R/share/advisors.yaml" ] || { echo "    the home fixture does not author the catalogue"; exit 1; }
home="$(rz reasoning check --format json 2>/dev/null)" || { echo "    the home fixture is not clean before the two files are planted"; exit 1; }
[ "$(jq -r '.checks | join(" ")' <<<"$home")" = "catalogue adapters coupling ci claims records" ] \
  || { echo "    the home does not run every family: $(jq -c '.checks' <<<"$home")"; exit 1; }
plant_product "$R"
expect_exit 10 rz reasoning check
expect_grep 'FAIL ci +\.github/workflows/archive/ci\.yml.*CI names the model credential ANTHROPIC_API_KEY'
expect_grep 'FAIL ci +\.github/workflows/archive/ci\.yml.*CI names the model credential OPENAI_API_KEY'
expect_grep "FAIL claims +docs/adrs/adr-0023-ollama-model-integration-architecture\.md.*asserts 'requires ollama'"
exit 0
