# majordomus-covers: none
# A campaign brief this repository keeps is the one it was handed, and the gate proves it.
#
# `campaigns/` is provenance: nothing reads it at run time, no projection renders it, no test
# depends on what a pack says. That is precisely why it rots without a gate — a pack edited to
# agree with what the repository later did still reads like the brief that drove the work, and
# a half-copied pack still reads like a pack. The manifest is what makes "verbatim" a claim
# anybody can check, and this case is what makes the manifest more than a file.
#
# The mutation that matters most is the one-sided shape this repository keeps rediscovering:
# a checker that proves every declared file exists, and never that every existing file is
# declared, passes a tree somebody quietly added a file to. Section 3 plants exactly that.
. "$ROOT/test/lib.sh"

CHK="$ROOT/scripts/ci/campaigns-check"
[ -x "$CHK" ] || { echo "    scripts/ci/campaigns-check is not executable"; exit 1; }

F="$T/tree"
mkdir -p "$F"
run_check() { rc=0; LAST_OUT="$(MJ_ROOT="$F" "$CHK" 2>&1)" || rc=$?; return 0; }

# ---------------------------------------------------------------- 0. a tree with no campaigns
# A repository that was never handed a brief owes none, and a gate that failed such a tree
# would be a gate every other repository has to disable.
run_check
[ "$rc" = 0 ] || { echo "    a tree with no campaigns/ exited $rc, not 0"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q 'keeps no campaigns' \
  || { echo "    a tree with no campaigns/ did not say so"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }

# ---------------------------------------------------------------- the fixture
mkdir -p "$F/campaigns/alpha-pack/phases" "$F/campaigns/beta-pack"
printf '# Alpha Pack\n\nWhat was asked for on the day.\n' > "$F/campaigns/alpha-pack/README.md"
printf 'Phase one.\n' > "$F/campaigns/alpha-pack/phases/01_first.md"
printf '# Beta Pack\n\nAnother brief.\n' > "$F/campaigns/beta-pack/README.md"
index() {
  {
    printf '# Campaign briefs\n\nProvenance, not instruction.\n\n'
    printf '| pack | of | files | first heading | taken from |\n|---|---|---|---|---|\n'
    for p in "$@"; do printf '| `%s` | 2026-09-15 | 1 | %s | `~/Downloads/%s` |\n' "$p" "$p" "$p"; done
  } > "$F/campaigns/README.md"
}
manifest() { ( cd "$F/campaigns" && find . -type f ! -name MANIFEST.sha256 | LC_ALL=C sort | xargs shasum -a 256 | sed 's|\./||' > MANIFEST.sha256 ); }
index alpha-pack beta-pack
manifest

# ---------------------------------------------------------------- 1. the tree as received
run_check
[ "$rc" = 0 ] || { echo "    an untouched campaigns/ exited $rc, not 0"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q '2 pack(s)' \
  || { echo "    the verdict does not name what it measured"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
echo "    a tree whose bytes are the manifest's bytes passes, and says what it measured"

# ---------------------------------------------------------------- 2. an edited brief
printf 'and a sentence nobody was handed\n' >> "$F/campaigns/alpha-pack/README.md"
run_check
[ "$rc" = 10 ] || { echo "    an edited brief exited $rc, not 10"; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q 'alpha-pack/README.md — its bytes are not' \
  || { echo "    the edited file was not named"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
git -C "$F" checkout . 2>/dev/null || true
manifest; index alpha-pack beta-pack
printf '# Alpha Pack\n\nWhat was asked for on the day.\n' > "$F/campaigns/alpha-pack/README.md"
manifest
echo "    a brief edited after the fact is named, with the file and the reason"

# ---------------------------------------------------------------- 3. a file nobody declared
# The one-sided failure: every declared file still exists, so a checker that only walks the
# manifest passes this tree. The subject is the set, in both directions.
printf 'slipped in later\n' > "$F/campaigns/alpha-pack/phases/02_added.md"
run_check
[ "$rc" = 10 ] || { echo "    a file added without the manifest exited $rc, not 10 (the checker walks only one side)"; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q '02_added.md — held by the tree and absent from the manifest' \
  || { echo "    the undeclared file was not named"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
rm -f "$F/campaigns/alpha-pack/phases/02_added.md"
echo "    a file added into a pack without the manifest is a finding, not a silence"

# ---------------------------------------------------------------- 4. a brief that went missing
rm -f "$F/campaigns/alpha-pack/phases/01_first.md"
run_check
[ "$rc" = 10 ] || { echo "    a missing declared file exited $rc, not 10"; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q '01_first.md — the manifest declares it' \
  || { echo "    the missing file was not named"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf 'Phase one.\n' > "$F/campaigns/alpha-pack/phases/01_first.md"
echo "    a file the manifest declares and the tree lost is a finding"

# ---------------------------------------------------------------- 5. a pack the index forgot
mkdir -p "$F/campaigns/gamma-pack"
printf '# Gamma Pack\n\nA third brief.\n' > "$F/campaigns/gamma-pack/README.md"
manifest
run_check
[ "$rc" = 10 ] || { echo "    a pack missing from the index exited $rc, not 10"; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q 'gamma-pack — a pack the index does not list' \
  || { echo "    the unlisted pack was not named"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
index alpha-pack beta-pack gamma-pack
manifest
run_check
[ "$rc" = 0 ] || { echo "    listing the pack did not settle it: exit $rc"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
echo "    a pack the index does not list is a finding, and listing it settles it"

# ---------------------------------------------------------------- 6. an index that promises a pack
index alpha-pack beta-pack gamma-pack delta-pack
manifest
run_check
[ "$rc" = 10 ] || { echo "    an index naming a pack that does not exist exited $rc, not 10"; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q 'lists `delta-pack`' \
  || { echo "    the promised-but-absent pack was not named"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
index alpha-pack beta-pack gamma-pack
manifest
echo "    an index that names a pack the tree does not hold is a finding"

# ---------------------------------------------------------------- 7. what cannot be decided
# Absence of the manifest is not "everything matches". A gate that reported clean here would be
# reporting on a set it never read.
rm -f "$F/campaigns/MANIFEST.sha256"
run_check
[ "$rc" = 12 ] || { echo "    campaigns/ without a manifest exited $rc, not 12 (refusal)"; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q 'nothing here can be proven' \
  || { echo "    the refusal does not say why"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
manifest
mv "$F/campaigns/README.md" "$F/campaigns/README.kept"
run_check
[ "$rc" = 12 ] || { echo "    campaigns/ without an index exited $rc, not 12 (refusal)"; exit 1; }
mv "$F/campaigns/README.kept" "$F/campaigns/README.md"
manifest
echo "    a tree that cannot be measured is refused, not passed"

echo "    a brief is kept as received: bytes, both directions of the set, and the index"
