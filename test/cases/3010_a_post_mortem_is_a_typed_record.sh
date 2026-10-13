# majordomus-covers: none
# A post-mortem is a typed record, and every clause that makes it one is refused when broken
# (rule project.a-ci-failure-has-a-post-mortem, ADR 0156).
#
# The explanation of a red trunk used to live on a board, in a pull request body or in a
# commit subject. It is a record now, and a record is only worth requiring if the tool can
# tell a finished one from an unfinished one: a draft that claims a cause, an analysed record
# that names no component or no signal, a resolved one that names no fix, and a reference to
# a file or a case the tree does not hold are each written here alone, so that the finding
# is that clause's and nothing else's. The schema's own half is the index's: an unknown key
# and a missing section are refused before the judgement is asked.
. "$ROOT/test/lib.sh"

RB="$(rust_bin)" || rust_bin_exit $?
PM() { env -u MAJORDOMUS_ROOT -u MAJORDOMUS_URL MAJORDOMUS_SHARE="$ROOT/share" "$RB" postmortem "$@"; }

# ---------------------------------------------------------------- this repository's own
# every record the repository holds is judged, and none is found wanting
( cd "$ROOT" && expect_exit 0 PM check ) || exit 1
held="$(find "$ROOT/.ai/repo/post-mortems" -name 'pm-*.md' | wc -l | tr -d ' ')"
[ "$held" -ge 1 ] || { echo "    the repository holds no post-mortem to judge"; exit 1; }
LAST_OUT="$(cd "$ROOT" && PM list 2>/dev/null)"
for f in "$ROOT"/.ai/repo/post-mortems/pm-*.md; do
  id="$(basename "$f" .md)"
  expect_grep "$id" || { echo "    list does not name $id"; exit 1; }
done

# ---------------------------------------------------------------- one clause at a time
"$MJ" init >/dev/null
D=.ai/repo/post-mortems
mkdir -p "$D" src
printf 'a component\n' > src/part.txt

# record ID STATUS CLASS [EXTRA-FRONT-MATTER] ; the analysed baseline departs in one way
record() {
  local id="$1" status="$2" class="$3" extra="${4-components: [\"file:src/part.txt\"]
signals:
  - id: line
    text: \"the part fell over\"}"
  {
    printf -- '---\nschema: post-mortem/v1\nid: %s\nkind: post-mortem\n' "$id"
    printf 'title: "The part fell over"\ndescription: "Hand-made for the check."\n'
    printf 'status: %s\ndate: 2026-10-13\n' "$status"
    printf 'runs:\n  - workflow: validate\n    id: "42"\n    commit: abcdef1\n    jobs: ["structure"]\n'
    printf 'cause:\n  class: %s\n  summary: "Hand-made."\n' "$class"
    [ -n "$extra" ] && printf '%s\n' "$extra"
    printf -- '---\n\n# The part fell over\n\n## What failed\n\nIt fell.\n\n## Root cause\n\nGravity.\n\n## Fix\n\nA shelf.\n\n## Prevention\n\nA case.\n'
  } > "$D/$id.md"
}
clear_store() { find "$D" -name 'pm-*.md' -delete; git add -A "$D" src >/dev/null; }
accepted() { git add -A "$D" src >/dev/null; expect_exit 0 PM check || exit 1; clear_store; }
refused() {   # pattern
  git add -A "$D" src >/dev/null
  expect_exit 10 PM check || exit 1
  expect_grep "$1" || exit 1
  clear_store
}

# the three statuses, each as it should be
record pm-20261013-good-analysed analysed defect
accepted
record pm-20261013-good-draft draft undetermined ""
accepted
record pm-20261013-good-resolved resolved defect 'components: ["file:src/part.txt"]
signals:
  - id: line
    text: "the part fell over"
fixed_by: ["commit:abcdef1"]'
accepted

# a draft that claims a cause, and an analysed record that claims none
record pm-20261013-draft-knows draft defect ""
refused 'pm-20261013-draft-knows'
record pm-20261013-analysed-unknown analysed undetermined
refused 'pm-20261013-analysed-unknown'

# analysed with no component, and with no signal
record pm-20261013-no-component analysed defect 'signals:
  - id: line
    text: "the part fell over"'
refused 'pm-20261013-no-component'
record pm-20261013-no-signal analysed defect 'components: ["file:src/part.txt"]'
refused 'pm-20261013-no-signal'

# resolved without a fix, and a fix on a record that is not resolved
record pm-20261013-no-fix resolved defect
refused 'pm-20261013-no-fix'
record pm-20261013-early-fix analysed defect 'components: ["file:src/part.txt"]
signals:
  - id: line
    text: "the part fell over"
fixed_by: ["commit:abcdef1"]'
refused 'pm-20261013-early-fix'

# a component and a guard the tree does not hold
record pm-20261013-gone analysed defect 'components: ["file:src/absent.txt"]
signals:
  - id: line
    text: "the part fell over"'
refused 'pm-20261013-gone'
record pm-20261013-no-guard analysed defect 'components: ["file:src/part.txt"]
signals:
  - id: line
    text: "the part fell over"
prevention: ["test:test/cases/0_absent.sh"]'
refused 'pm-20261013-no-guard'

# ---------------------------------------------------------------- the schema's half
# an unknown key, and a required section left out: the index refuses the document, so the
# record is not among those judged and the tool says the layer is degraded
record pm-20261013-unknown-key analysed defect 'components: ["file:src/part.txt"]
signals:
  - id: line
    text: "the part fell over"
blame: somebody'
git add -A "$D" src >/dev/null
LAST_OUT="$(PM check 2>&1 || true)"
expect_grep 'blame' || { echo "    an unknown front-matter key was carried silently"; exit 1; }
clear_store

record pm-20261013-no-section analysed defect
sed -i.bak '/^## Root cause$/,/^Gravity\.$/d' "$D/pm-20261013-no-section.md"; rm -f "$D/pm-20261013-no-section.md.bak"
git add -A "$D" src >/dev/null
LAST_OUT="$(PM check 2>&1 || true)"
expect_grep 'Root cause' || { echo "    a record without its Root cause section was accepted"; exit 1; }
clear_store
