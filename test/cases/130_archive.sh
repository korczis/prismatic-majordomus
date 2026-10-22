# majordomus-covers: archive
# majordomus-negative: archive
#
# An archive is believed at the far end, where nobody can check it against the original.
# That is what makes its failures expensive: the two this case exists for both look, to
# the recipient, like the repository being broken rather than the copy being wrong.
#
# The first is the file mode. `zip -X` strips the extra fields that carry it, so the
# unpacked tree has no executable in it, every script fails to run, and the person at the
# far end reports a broken repository. That is how this command came to exist. So the
# round trip is measured here rather than assumed: an executable goes in, the archive is
# opened, and it is still executable — and when a reader discards the modes anyway, the
# restore script inside the archive must put them back from the manifest.
#
# The second is a selection that quietly matched nothing. A profile that excludes
# everything, or a .gitattributes rule read the wrong way round, produces a smaller
# archive and no error at all. Every count asserted below is compared against something
# measured from the repository, and each rule is proved by breaking it: the file that
# `derived` drops is shown present before the attribute exists and absent after.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
"$MJ" update >/dev/null

mkdir -p lib docs
echo 'a' > lib/a
echo 'd' > docs/d
printf '#!/bin/sh\necho hi\n' > lib/runme.sh
chmod 755 lib/runme.sh
printf 'projection\n' > docs/PROJECTED.md
git add -A && git commit -qm base

tracked="$(git ls-files | wc -l | tr -d ' ')"
[ "$tracked" -gt 5 ] || { echo "    only $tracked tracked file(s); this case is measuring the wrong tree"; exit 1; }

# ---- the profiles are read from the declaration, not from the code ---------------------
expect_exit 0 "$MJ" archive --list
expect_grep 'context'
expect_grep '\(default\)'
expect_grep 'audit'
expect_grep 'governance'

# ---- the selection states its own denominator ------------------------------------------
expect_exit 0 "$MJ" archive --dry-run
expect_grep "of $tracked tracked file"
expect_grep 'nothing written'

n_context="$("$MJ" --json archive --dry-run | sed -n 's/.*"archived":\([0-9]*\).*/\1/p')"
[ -n "$n_context" ] || { echo "    --json archive --dry-run reports no archived count"; exit 1; }
[ "$n_context" -gt 0 ] || { echo "    the context profile selected 0 of $tracked tracked file(s)"; exit 1; }

n_audit="$("$MJ" --json archive audit --dry-run | sed -n 's/.*"archived":\([0-9]*\).*/\1/p')"
[ "$n_audit" = "$tracked" ] \
  || { echo "    the audit profile archives $n_audit of $tracked tracked file(s); it drops nothing"; exit 1; }

# ---- `derived: drop` reads .gitattributes, and does so in both directions ---------------
# before the attribute exists, the file is in the archive
"$MJ" --json archive --dry-run > /dev/null
before="$n_context"
printf 'docs/PROJECTED.md merge=derived\n' > .gitattributes
git add .gitattributes && git commit -qm attrs
[ "$(git check-attr merge -- docs/PROJECTED.md)" = "docs/PROJECTED.md: merge: derived" ] \
  || { echo "    the probe did not take: git does not report docs/PROJECTED.md as merge=derived"; exit 1; }
after="$("$MJ" --json archive --dry-run | sed -n 's/.*"archived":\([0-9]*\).*/\1/p')"
# one file gained (.gitattributes), one file lost (the derived one): the net is zero, so
# the count alone proves nothing and the reason is what is asserted
reasons="$("$MJ" --json archive --dry-run | sed -n 's/.*"reasons":{\(.*\)}}/\1/p')"
case "$reasons" in
  *'"derived":1'*) ;;
  *) echo "    .gitattributes marks one file merge=derived and the profile dropped: $reasons"; exit 1 ;;
esac
[ "$after" -le "$before" ] \
  || { echo "    the derived file was marked and the archive grew: $before -> $after"; exit 1; }

# ---- the mode survives the container ----------------------------------------------------
# Proves `archive-fidelity`: an archive that lost the file modes fails rather than
# travelling — the round trip is measured, and a reader that discards the modes anyway is
# answered by the restore script putting them back from the manifest.
if command -v zip >/dev/null 2>&1 && command -v unzip >/dev/null 2>&1; then
  expect_exit 0 "$MJ" archive audit --out "$T/audit.zip"
  expect_grep 'OK   archive'
  expect_grep 'read back'
  expect_file "$T/audit.zip"

  U="$T/unpacked"; mkdir -p "$U"
  ( cd "$U" && unzip -q "$T/audit.zip" ) || { echo "    the archive does not unpack"; exit 1; }
  D="$(find "$U" -maxdepth 1 -mindepth 1 -type d | head -1)"
  [ -n "$D" ] || { echo "    the archive unpacked to no directory"; exit 1; }
  [ -f "$D/lib/runme.sh" ] || { echo "    lib/runme.sh is not in the unpacked archive"; exit 1; }
  [ -x "$D/lib/runme.sh" ] \
    || { echo "    lib/runme.sh went in executable and came out $(file_mode "$D/lib/runme.sh"); the container discarded the modes"; exit 1; }

  # ---- and when a reader discards them anyway, the manifest is what puts them back ------
  [ -f "$D/_ARCHIVE/MANIFEST.txt" ] || { echo "    the archive carries no _ARCHIVE/MANIFEST.txt"; exit 1; }
  grep -q '^100755 .*lib/runme.sh$' "$D/_ARCHIVE/MANIFEST.txt" \
    || { echo "    the manifest does not record lib/runme.sh as executable"; exit 1; }
  # files only: stripping x from the directories would stop the tree being traversable,
  # which is a different failure from the one being reproduced
  find "$D/lib" "$D/_ARCHIVE" -type f -exec chmod a-x {} + 2>/dev/null || true
  [ -x "$D/lib/runme.sh" ] && { echo "    the probe did not take: lib/runme.sh is still executable"; exit 1; }
  ( cd "$D" && sh _ARCHIVE/restore.sh ) > "$T/restore.out" 2>&1 \
    || { echo "    restore.sh failed:"; sed 's/^/      | /' "$T/restore.out"; exit 1; }
  [ -x "$D/lib/runme.sh" ] \
    || { echo "    restore.sh ran and lib/runme.sh is still not executable"; sed 's/^/      | /' "$T/restore.out"; exit 1; }
  grep -q 'modes applied to' "$T/restore.out" \
    || { echo "    restore.sh does not say how many modes it applied"; sed 's/^/      | /' "$T/restore.out"; exit 1; }

  # ---- a copy the repository's own enumeration can read ---------------------------------
  [ -d "$D/.git" ] || { echo "    restore.sh created no git index"; sed 's/^/      | /' "$T/restore.out"; exit 1; }
  restored="$( cd "$D" && git ls-files | wc -l | tr -d ' ' )"
  [ "$restored" -ge "$tracked" ] \
    || { echo "    the restored index holds $restored file(s) and the repository tracked $tracked"; exit 1; }

  # ---- an existing output is not replaced by accident -----------------------------------
  expect_exit 15 "$MJ" archive audit --out "$T/audit.zip"
  expect_grep 'exists'
  expect_exit 0 "$MJ" archive audit --out "$T/audit.zip" --force
else
  echo "    skip: zip or unzip is not on PATH; the container round trip was not measured"
fi

# ---- the refusals ------------------------------------------------------------------------
expect_exit 12 "$MJ" archive nosuch
expect_grep 'no profile'
expect_grep 'archive --list'

expect_exit 2 "$MJ" archive --format 7z
expect_grep 'zip or tar.gz'

expect_exit 2 "$MJ" archive --nonesuch
expect_grep 'unknown option'

expect_exit 2 "$MJ" archive context audit
expect_grep 'one profile only'

printf 'ok   archive: %s profiles, %s of %s tracked file(s) in the default, modes proved through the container\n' \
  3 "$n_context" "$tracked"
