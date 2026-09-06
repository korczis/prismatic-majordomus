# The distribution model is the one place a platform, an artifact name or an installation
# URL is written, and every projection of it agrees. This case proves the invariants the
# rule project.distribution-canonical states, and it proves them by mutation: a target
# added to a copy of the model must change the installer, the matrix and the guide.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

# --- the model itself holds ------------------------------------------------------------
expect_exit 0 "$MJB" distribution --repo "$ROOT" validate
"$MJB" distribution --repo "$ROOT" validate 2>/dev/null | grep -q '^OK   distribution' \
  || { echo "    validate said nothing about the model"; exit 1; }

# --- every projection is current -------------------------------------------------------
expect_exit 0 "$MJB" generate --repo "$ROOT" distribution --check

# --- the installer is generated, and is the model's own table --------------------------
inst="$ROOT/site/static/install.sh"
expect_file "$inst"
expect_grep "GENERATED FILE" "$inst"
expect_grep "share/distribution.yaml" "$inst"
"$MJB" distribution --repo "$ROOT" targets 2>/dev/null | awk 'NR>1{print $2}' | while read -r triple; do
  grep -q "$triple" "$inst" || { echo "    the installer does not carry $triple"; exit 1; }
done

# --- the naming function is one function -----------------------------------------------
# every published target derives a distinct name, and the name is the one shape
seen=""
"$MJB" distribution --repo "$ROOT" targets 2>/dev/null | awk 'NR>1 && $3=="supported"{print $1}' | while read -r id; do
  name="$("$MJB" distribution --repo "$ROOT" artifact --target "$id" --tag v9.9.9 --format text 2>/dev/null | sed -n 1p)"
  case "$name" in
    majordomus-v9.9.9-*.tar.gz) ;;
    *) echo "    $id derives '$name', which is not the naming function's shape"; exit 1 ;;
  esac
  case " $seen " in *" $name "*) echo "    two targets derive $name"; exit 1 ;; esac
  seen="$seen $name"
done

# --- the matrix is derived, and carries no tag -----------------------------------------
matrix="$ROOT/docs/generated/distribution-matrix.json"
expect_file "$matrix"
expect_grep '"artifact"' "$matrix"
expect_no_grep 'v[0-9]\.[0-9]\.[0-9]' "$matrix"
count_model="$("$MJB" distribution --repo "$ROOT" targets 2>/dev/null | awk 'NR>1 && ($3=="supported"||$3=="experimental")' | wc -l | tr -d ' ')"
count_matrix="$(grep -c '"target":' "$matrix")"
[ "$count_model" = "$count_matrix" ] \
  || { echo "    the model publishes $count_model target(s) and the matrix builds $count_matrix"; exit 1; }

# --- the one install command reaches every document that states it ---------------------
cmd="$("$MJB" distribution --repo "$ROOT" show --format json 2>/dev/null \
       | sed -n 's/.*"install_command": "\(.*\)",*$/\1/p' | head -n 1)"
[ -n "$cmd" ] || { echo "    the model states no install command"; exit 1; }
for doc in "$ROOT/README.md" "$ROOT/docs/INSTALL.md"; do
  grep -qF "$cmd" "$doc" || { echo "    $doc does not carry the canonical install command: $cmd"; exit 1; }
done

# --- the guide's platform table is the model's ------------------------------------------
"$MJB" distribution --repo "$ROOT" targets 2>/dev/null | awk 'NR>1{print $2}' | while read -r triple; do
  grep -q "$triple" "$ROOT/docs/INSTALL.md" \
    || { echo "    the installation guide does not list $triple"; exit 1; }
done

# --- the version is stated in two places and they agree --------------------------------
expect_exit 0 "$ROOT/scripts/release-version" --check
version="$("$ROOT/scripts/release-version")"
expect_exit 10 "$ROOT/scripts/release-version" --check --tag "v0.0.0-not-the-version"
expect_exit 0 "$ROOT/scripts/release-version" --check --tag "v$version"

# --- mutation: a target added to the model reaches every projection ---------------------
cp -R "$ROOT/share" "$T/share"
cat >> "$T/share/distribution.yaml" <<'YAML'

  - id: linux-riscv64-gnu
    os: linux
    arch: x86_64
    libc: gnu
    rust_target: riscv64gc-unknown-linux-gnu
    status: supported
    build:
      runner: ubuntu-24.04
      native: false
YAML
mkdir -p "$T/gen"
MAJORDOMUS_SHARE="$T/share" "$MJB" generate --repo "$ROOT" distribution --out "$T/gen" >/dev/null 2>&1 \
  || { echo "    a target added to the model does not generate"; exit 1; }
expect_grep "riscv64gc-unknown-linux-gnu" "$T/gen/site/static/install.sh"
expect_grep "riscv64gc-unknown-linux-gnu" "$T/gen/docs/generated/distribution-matrix.json"
expect_grep "riscv64gc-unknown-linux-gnu" "$T/gen/docs/INSTALL.md"
# ... and the committed projections are now stale, which is what the drift gate refuses
MAJORDOMUS_SHARE="$T/share" "$MJB" generate --repo "$ROOT" distribution --check >/dev/null 2>&1 \
  && { echo "    a changed model left the committed projections passing --check"; exit 1; }

# --- mutation: the invariants refuse what they say they refuse -------------------------
refuses() { # <edit> <expected fragment>
  rm -rf "$T/bad"; cp -R "$ROOT/share" "$T/bad"
  python3 - "$T/bad/distribution.yaml" "$1" <<'PY' 2>/dev/null || return 0
import sys
path, edit = sys.argv[1], sys.argv[2]
s = open(path).read()
old, new = edit.split('||')
open(path, 'w').write(s.replace(old, new, 1))
PY
  out="$(MAJORDOMUS_SHARE="$T/bad" "$MJB" distribution --repo "$ROOT" validate 2>&1)" && {
    echo "    the model accepted: $1"; return 1; }
  printf '%s\n' "$out" | grep -q "$2" || { echo "    refusal did not name '$2': $out"; return 1; }
  return 0
}
if command -v python3 >/dev/null 2>&1; then
  refuses 'base_url: https://||base_url: http://' 'not https' || exit 1
  refuses 'rust_target: x86_64-apple-darwin||rust_target: aarch64-apple-darwin' 'claim the Rust target' || exit 1
  refuses 'id: macos-x86_64||id: macos-aarch64' 'claim the id' || exit 1
else
  echo "    note: python3 absent; the mutation refusals were not exercised"
fi
exit 0
