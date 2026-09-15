# majordomus-covers: none
# claims: none
# Nothing in the repository runs Perl.
#
# Perl was never a declared dependency, and it was reached twice without anyone choosing it:
# as the timing clock's fallback in lib/common.sh (`perl -MTime::HiRes`), and through
# `shasum`, which on macOS is a Perl script (`head -1 /usr/bin/shasum`) and was the SHA-256
# of lib/, of four scripts, of the installer and of twenty cases. SHA-256 is now sha256sum,
# else openssl (lib/sha256.sh), and the clock is EPOCHREALTIME, else whole seconds.
#
# This holds it: no tracked code under bin/ lib/ scripts/ share/ test/ .github/ .githooks/
# .claude/ .just/ apps/, the justfile or the published installer names either command
# outside a comment. The words are matched whole and case-sensitively, so "properly" and
# "Perl" in prose pass; a comment line (`#`, or `//` in Rust) is prose and passes; a
# shebang naming perl does not. Markdown is prose and is not read. Every exception is one
# exact line, listed below with its reason, so an exception cannot grow into a file.
#
# The scanner is proved on a planted fixture before its verdict on the repository counts:
# a scanner that finds nothing in a tree that has something is the green this case would
# otherwise be.
. "$ROOT/test/lib.sh"

# The two words, assembled, so that this file is scanned like every other and needs no
# exemption of its own.
P="pe""rl"; S="sha""sum"

# <file>\t<exact line>: data that names the command without running it.
EXEMPT="$T/exempt"
printf '%s\t%s\n' \
  "test/cases/100_environment.sh" \
  "  local forbidden='git|grep|sed|awk|find|jq|wc|curl|wget|cargo|npm|pnpm|yarn|docker|make|xargs|$P|python|python3|ruby|node'" \
  > "$EXEMPT"

# scan <repository>: prints <path>:<line>: <text> for every use, from the files git tracks.
scan() {
  (
    cd "$1" || exit 1
    git ls-files -z -- bin lib scripts share test .github .githooks .claude .just apps justfile \
        site/static/install.sh \
      | tr '\0' '\n' \
      | grep -vE '\.(md|md\.in|json|svg|png|jpg|ico|lock)$' \
      | while IFS= read -r f; do [ -f "$f" ] && printf '%s\0' "$f"; done \
      | xargs -0 awk -v p="$P" -v s="$S" -v exempt="$EXEMPT" '
          BEGIN {
            while ((getline line < exempt) > 0) { t = index(line, "\t"); ok[substr(line, 1, t - 1) SUBSEP substr(line, t + 1)] = 1 }
            re = "(^|[^A-Za-z0-9_])(" p "|" s ")([^A-Za-z0-9_]|$)"
          }
          FNR == 1 && /^#!/ { if ($0 ~ re) print FILENAME ":" FNR ": " $0; next }
          /^[[:space:]]*#/ { next }
          FILENAME ~ /\.rs$/ && /^[[:space:]]*\/\// { next }
          (FILENAME SUBSEP $0) in ok { next }
          { code = $0; sub(/[[:space:]]#.*$/, "", code); if (code ~ re) print FILENAME ":" FNR ": " $0 }
        '
  )
}

# --- the scanner, on a tree that has every shape of use and every shape of non-use ---------
F="$T/fixture"; mkdir -p "$F/lib" "$F/scripts" "$F/.github/workflows" "$F/test/cases" "$F/apps/x/src" "$F/docs"
( cd "$F" && git init -q . )
printf '#!/usr/bin/%s\nprint 1;\n' "$P" > "$F/scripts/shebang"
printf 'h="$(git ls-files | %s -a 256)"\n' "$S" > "$F/lib/pipe.sh"
printf 'now() { %s -MTime::HiRes=time -e 1; }\n' "$P" > "$F/lib/clock.sh"
printf 'find . -exec %s {} +\nx=1 # %s in a trailing comment is prose\n' "$S" "$S" > "$F/scripts/exec"
printf 'jobs:\n  a:\n    steps:\n      - run: %s -e 1\n' "$P" > "$F/.github/workflows/w.yml"
printf 'let c = Command::new("%s");\n' "$S" > "$F/apps/x/src/main.rs"
# not uses: comments, prose words, a longer word, markdown, the exempt line itself
printf '# %s was the fallback\n  # and %s too\necho properly %sish Perl\n' "$P" "$S" "$S" > "$F/lib/prose.sh"
printf '/// `%s` writes to stderr\n' "$S" > "$F/apps/x/src/doc.rs"
printf 'run %s here\n' "$P" > "$F/docs/NOTE.md"
cp "$ROOT/test/cases/100_environment.sh" "$F/test/cases/100_environment.sh"
( cd "$F" && git add -A )
scan "$F" > "$T/planted"
for want in 'scripts/shebang:1:' 'lib/pipe.sh:1:' 'lib/clock.sh:1:' 'scripts/exec:1:' \
            '.github/workflows/w.yml:4:' 'apps/x/src/main.rs:1:'; do
  grep -qF "$want" "$T/planted" || { echo "    the scanner missed a planted use at $want; it found:"; sed 's/^/      /' "$T/planted"; exit 1; }
done
[ "$(wc -l < "$T/planted" | tr -d ' ')" = 6 ] || { echo "    the scanner flagged something that is not a use:"; sed 's/^/      /' "$T/planted"; exit 1; }

# --- the repository ------------------------------------------------------------------------
scan "$ROOT" > "$T/found"
if [ -s "$T/found" ]; then
  echo "    tracked code runs $P or $S (sha256: mj_sha256_hex in lib/sha256.sh; time: EPOCHREALTIME):"
  sed 's/^/      /' "$T/found"
  exit 1
fi

# --- and SHA-256 still works both ways the tool can compute it -----------------------------
printf abc > "$T/abc"
want=ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
[ "$(mj_sha256_hex "$T/abc")" = "$want" ] || { echo "    mj_sha256_hex is wrong"; exit 1; }
if command -v openssl >/dev/null 2>&1; then
  got="$(MJ_SHA256_TOOL=openssl mj_sha256_hex "$T/abc")"
  [ "$got" = "$want" ] || { echo "    the openssl branch hashes wrong: $got"; exit 1; }
  got="$(printf '%s\0' "$T/abc" | MJ_SHA256_TOOL=openssl mj_sha256_xargs -0)"
  [ "$got" = "$want  $T/abc" ] || { echo "    the openssl branch does not print sha256sum's line: $got"; exit 1; }
fi
# neither tool: a refusal naming both, never an empty hash
empty="$T/emptybin"; mkdir -p "$empty"
for tool in xargs cut sed; do ln -s "$(command -v "$tool")" "$empty/$tool"; done
rc=0; out="$( (PATH="$empty"; unset MJ_SHA256_TOOL; mj_sha256_hex "$T/abc") 2>&1)" || rc=$?
[ "$rc" != 0 ] || { echo "    with no digest tool mj_sha256_hex succeeded: $out"; exit 1; }
case "$out" in *"need sha256sum or openssl"*) ;; *) echo "    with no digest tool the refusal did not name the tools: $out"; exit 1 ;; esac
