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
# The fixture's manifest is written the way campaigns/README.md says to write one: every file
# but the top-level manifest itself, and never with `shasum`, which is a Perl script on macOS.
if command -v sha256sum >/dev/null 2>&1; then
  hash_all() { xargs -0 sha256sum; }
else
  hash_all() { xargs -0 openssl dgst -sha256 -r; }
fi
manifest() {
  ( cd "$F/campaigns" && find . -type f ! -path ./MANIFEST.sha256 -print0 | LC_ALL=C sort -z | hash_all \
      | sed 's|^\([0-9a-f]*\) [ *]\./|\1  |' > MANIFEST.sha256 )
}
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

# ---------------------------------------------------------------- 3b. a manifest below the top
# Only campaigns/MANIFEST.sha256 is left out of its own list. A check that dropped the name at
# any depth would never see a pack's own manifest (kept as received, so a byte of the brief),
# and would never see a new file of that name planted anywhere below the top. Each plant here
# passes a check that excludes by name, so each one is a finding only when the exclusion is
# the one path it should be.
printf 'not the manifest of anything\n' > "$F/campaigns/alpha-pack/phases/MANIFEST.sha256"
run_check
[ "$rc" = 10 ] || { echo "    an unrecorded file named MANIFEST.sha256 below the top exited $rc, not 10 (the check excludes the name, not the path)"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q 'alpha-pack/phases/MANIFEST.sha256 — held by the tree and absent from the manifest' \
  || { echo "    the unrecorded nested manifest was not named"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
rm -f "$F/campaigns/alpha-pack/phases/MANIFEST.sha256"
echo "    a new file named MANIFEST.sha256 below the top is a finding, not an exclusion"

printf '0000  ./README.md\n' > "$F/campaigns/beta-pack/MANIFEST.sha256"
manifest
grep -q '  beta-pack/MANIFEST.sha256$' "$F/campaigns/MANIFEST.sha256" \
  || { echo "    the fixture's manifest did not record the pack's own manifest"; exit 1; }
run_check
[ "$rc" = 0 ] || { echo "    a pack's own manifest, recorded, exited $rc, not 0"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf '0001  ./phases/99_later.md\n' >> "$F/campaigns/beta-pack/MANIFEST.sha256"
run_check
[ "$rc" = 10 ] || { echo "    a tampered pack manifest exited $rc, not 10"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q 'beta-pack/MANIFEST.sha256 — its bytes are not the bytes the manifest recorded' \
  || { echo "    the tampered pack manifest was not named"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
rm -f "$F/campaigns/beta-pack/MANIFEST.sha256"
manifest
echo "    a pack's own manifest is recorded like any byte, and appending to it is a finding"

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

# ---------------------------------------------------------------- 8. and it may name what this tree does not have
# A brief describes the repository somebody asked for, which is not always the one that was
# built: several of these name a layout that was considered and refused, or built and since
# moved. `reference-check` exempts campaigns/ for the same reason it exempts session records
# and decisions — editing one so its paths resolve would make the record of the request agree
# with the outcome. The exemption has to be narrow, so the control is the same sentence
# outside campaigns/, which must still be a finding.
G="$T/refs"
mkdir -p "$G/campaigns/alpha-pack" "$G/docs"
( cd "$G" && git init -q . && git config user.email t@t && git config user.name t )
printf '# Alpha\n\nThe plan lives in docs/NOWHERE.md and nowhere else.\n' > "$G/campaigns/alpha-pack/README.md"
( cd "$G" && git add -A >/dev/null && git commit -qm brief >/dev/null )
rc=0; "$ROOT/scripts/ci/reference-check" "$G" >"$T/ref1.out" 2>&1 || rc=$?
[ "$rc" = 0 ] || {
  echo "    a brief naming a path this tree does not have was reported as a broken reference"
  grep -vE '^(OK|INFO)' "$T/ref1.out" | tail -3 | sed 's/^/      /'
  exit 1
}
printf '# A note\n\nThe plan lives in docs/NOWHERE.md and nowhere else.\n' > "$G/docs/NOTE.md"
( cd "$G" && git add -A >/dev/null && git commit -qm note >/dev/null )
rc=0; "$ROOT/scripts/ci/reference-check" "$G" >"$T/ref2.out" 2>&1 || rc=$?
[ "$rc" != 0 ] || {
  echo "    the same sentence outside campaigns/ was not a finding; the exemption exempts everything"
  exit 1
}
echo "    a brief may name a path this repository does not have, and only a brief may"

# ---------------------------------------------------------------- 9. either hasher, and names with spaces
# The check hashes with sha256sum where it is present and with openssl where it is not; each
# branch is forced here so that neither is the one nobody ran. A name with whitespace travels
# NUL-separated and must be one row, not two.
printf 'a phase with a spaced name\n' > "$F/campaigns/alpha-pack/phases/03 spaced name.md"
manifest
ran=0
for h in sha256sum openssl; do
  command -v "$h" >/dev/null 2>&1 || { echo "    $h is not on this machine; its branch is proven where it is"; continue; }
  ran=$((ran + 1))
  rc=0; LAST_OUT="$(MJ_SHA256="$h" MJ_ROOT="$F" "$CHK" 2>&1)" || rc=$?
  [ "$rc" = 0 ] || { echo "    MJ_SHA256=$h on a recorded tree exited $rc, not 0"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
  printf '%s\n' "$LAST_OUT" | grep -q "hashed by $h" \
    || { echo "    MJ_SHA256=$h did not say which tool hashed"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
  printf 'edited\n' >> "$F/campaigns/alpha-pack/phases/03 spaced name.md"
  rc=0; LAST_OUT="$(MJ_SHA256="$h" MJ_ROOT="$F" "$CHK" 2>&1)" || rc=$?
  [ "$rc" = 10 ] || { echo "    MJ_SHA256=$h on an edited spaced name exited $rc, not 10"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
  printf '%s\n' "$LAST_OUT" | grep -q 'alpha-pack/phases/03 spaced name.md — its bytes are not' \
    || { echo "    MJ_SHA256=$h did not name the edited spaced file whole"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
  printf 'a phase with a spaced name\n' > "$F/campaigns/alpha-pack/phases/03 spaced name.md"
done
[ "$ran" -gt 0 ] || { echo "    neither sha256sum nor openssl is here, so no hasher branch was proven"; exit 1; }
rc=0; LAST_OUT="$(MJ_SHA256=shasum MJ_ROOT="$F" "$CHK" 2>&1)" || rc=$?
[ "$rc" = 12 ] || { echo "    MJ_SHA256 naming the Perl hasher exited $rc, not 12 (refusal)"; exit 1; }
rm -f "$F/campaigns/alpha-pack/phases/03 spaced name.md"
manifest
echo "    each hasher decides the same set, a spaced name is one row, and the Perl hasher is refused"

# ---------------------------------------------------------------- 10. names and entries no row can carry
# Each plant here passed the check before it refused them: the internal tables are
# `<path>\t<hash>`, so a name holding a TAB split into a declared path and a hash the planter
# chose; `find -type f` never saw a symbolic link, to a file or to a directory; and a file the
# hasher could not read ended the check under pipefail with an exit nobody documented.
run_check
[ "$rc" = 0 ] || { echo "    the fixture before section 10 exited $rc, not 0"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }

# 10a. "<recorded path><TAB><its recorded hash>", any content, the manifest untouched.
alpha_hash="$(cd "$F/campaigns/alpha-pack" && printf 'README.md\0' | hash_all | cut -c1-64)"
TABC="$(printf '\t')"
printf 'anything at all\n' > "$F/campaigns/alpha-pack/README.md$TABC$alpha_hash"
run_check
[ "$rc" = 12 ] || { echo "    a name aliasing a recorded row with a TAB and its hash exited $rc, not 12 (a TAB split the path)"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q 'control character' \
  || { echo "    the TAB-name refusal does not say why"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
rm -f "$F/campaigns/alpha-pack/README.md$TABC$alpha_hash"
printf 'a byte below space\n' > "$F/campaigns/beta-pack/$(printf 'odd\001name').md"
run_check
[ "$rc" = 12 ] || { echo "    a name holding a 0x01 byte exited $rc, not 12"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
rm -f "$F/campaigns/beta-pack/$(printf 'odd\001name').md"
# The same alias written into the manifest instead of the tree: a declared line whose path
# holds a TAB is one the tables would split, so it is refused, not compared.
cp "$F/campaigns/MANIFEST.sha256" "$T/manifest.kept"
printf '%s  alpha-pack/README.md\t%s\n' "$alpha_hash" "$alpha_hash" >> "$F/campaigns/MANIFEST.sha256"
run_check
[ "$rc" = 12 ] || { echo "    a manifest line whose path holds a TAB exited $rc, not 12"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q "MANIFEST.sha256 holds a line that is not" \
  || { echo "    the malformed manifest line was not named"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
cp "$T/manifest.kept" "$F/campaigns/MANIFEST.sha256"
echo "    a name or a manifest line holding a TAB or another control byte is refused, not split into a row"

# 10b. a symbolic link to a file: no manifest line records it, and find -type f never saw it.
ln -s README.md "$F/campaigns/alpha-pack/linked.md"
run_check
[ "$rc" = 10 ] || { echo "    a symbolic link to a file exited $rc, not 10 (links are not walked)"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q 'alpha-pack/linked.md — a symbolic link' \
  || { echo "    the linked file was not named"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
rm -f "$F/campaigns/alpha-pack/linked.md"

# 10c. a symbolic link to a directory, here one outside campaigns/ altogether.
mkdir -p "$T/elsewhere"
printf 'a brief nobody kept here\n' > "$T/elsewhere/README.md"
ln -s "$T/elsewhere" "$F/campaigns/alpha-pack/phases/linked-dir"
run_check
[ "$rc" = 10 ] || { echo "    a symbolic link to a directory exited $rc, not 10"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q 'alpha-pack/phases/linked-dir — a symbolic link' \
  || { echo "    the linked directory was not named"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q 'linked-dir/README.md' \
  && { echo "    the check followed the linked directory into what it reaches"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
rm -f "$F/campaigns/alpha-pack/phases/linked-dir"
# and campaigns/ itself as a link: the briefs it reaches are not kept in this tree
H="$T/linked-root"
mkdir -p "$H"
ln -s "$F/campaigns" "$H/campaigns"
rc=0; LAST_OUT="$(MJ_ROOT="$H" "$CHK" 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    campaigns/ as a symbolic link exited $rc, not 10"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q '^FAIL campaigns  campaigns — a symbolic link' \
  || { echo "    campaigns/ as a symbolic link was not named as one"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
run_check
[ "$rc" = 0 ] || { echo "    removing the links did not settle the tree: exit $rc"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
echo "    a symbolic link, to a file, to a directory or as campaigns/ itself, is a finding and is never followed"

# 10d. a file the hasher cannot read is a refusal that names it, not a crash with exit 1.
chmod 000 "$F/campaigns/beta-pack/README.md"
if [ -r "$F/campaigns/beta-pack/README.md" ]; then
  echo "    (this user reads a mode-000 file, as root does; the unreadable plant is proven where it cannot)"
else
  for h in sha256sum openssl; do
    command -v "$h" >/dev/null 2>&1 || continue
    rc=0; LAST_OUT="$(MJ_SHA256="$h" MJ_ROOT="$F" "$CHK" 2>&1)" || rc=$?
    [ "$rc" = 12 ] || { chmod 644 "$F/campaigns/beta-pack/README.md"; echo "    MJ_SHA256=$h on an unreadable file exited $rc, not 12"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
    printf '%s\n' "$LAST_OUT" | grep -q 'campaigns/beta-pack/README.md — not readable' \
      || { chmod 644 "$F/campaigns/beta-pack/README.md"; echo "    MJ_SHA256=$h did not name the unreadable file"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }
  done
  echo "    a file the hasher cannot read is refused with exit 12 and named"
fi
chmod 644 "$F/campaigns/beta-pack/README.md"
run_check
[ "$rc" = 0 ] || { echo "    restoring the mode did not settle the tree: exit $rc"; printf '%s\n' "$LAST_OUT" | sed 's/^/      /'; exit 1; }

echo "    a brief is kept as received: bytes, both directions of the set, and the index"
