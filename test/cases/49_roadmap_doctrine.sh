# The roadmap is a projection, never a document.
#
# A doctrine that only passes is not evidence of anything, so this case moves the exit code
# in both directions: a document that invents a release, and a document that hides one. The
# releases are not typed anywhere: a milestone's release is the first release tag containing
# the merge of its last accepted issue (released_in, ADR 0166), so the fixture accepts real
# work, commits it and tags it. The last cases are the ones that matter longest — with no
# roadmap section at all, the check holds trivially, because the doctrine exists to refuse
# the regression rather than to require the table.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH
pj_init

rm_milestone() { # ID ORDER [DEP ...]
  local id="$1" ord="$2"; shift 2
  { cat <<Y
id: $id
title: Milestone $id
slug: $id
order: $ord
priority: p1
problem: "A problem worth solving."
outcome: "The outcome once it is solved."
acceptance_criteria:
  - The outcome is reached
validation:
  - true
evidence_required:
  - proof
Y
    if [ $# -gt 0 ]; then printf 'depends_on:\n'; for d in "$@"; do printf -- '  - %s\n' "$d"; done; fi
  } > ".ai/repo/project/milestones/$id.yaml"
}
accept() { # ISSUE MILESTONE — the issue accepted with its evidence, the milestone's too, committed
  pj_issue "$1" "$2"
  "$MJ" plan start "$1" >/dev/null
  "$MJ" plan verify "$1" >/dev/null
  "$MJ" plan evidence "$1" --covers proof --type test --command true --result ok >/dev/null
  "$MJ" plan 'done' "$1" >/dev/null
  printf 'evidence:\n  - covers: proof\n    type: test\n    command: "true"\n    result: "ok"\n    recorded_at: 2026-10-16\n' \
    >> ".ai/repo/project/milestones/$2.yaml"
  git add -A >/dev/null && git commit -qm "feat: accept $1 ($2)"
}
readme() { # write a README with the given release rows
  { printf '# Fixture\n\n## Roadmap\n\n| release | adds |\n|---|---|\n'
    for v in "$@"; do printf '| %s | something |\n' "$v"; done
    printf '\n## Contributing\n\nRead it, and AGENTS.md.\n'
  } > README.md
}

rm_milestone alpha 10
rm_milestone beta  20 alpha
rm_milestone gamma 30 beta
expect_exit 0 "$MJ" plan validate
accept A1 alpha; git tag v0.1.0
accept B1 beta;  git tag v0.2.0
accept G1 gamma  # accepted, merged, not yet tagged: unreleased
[ "$("$MJ" plan roadmap --json | jq -r '[.milestones[] | "\(.id)=\(.released_in)"] | join(" ")')" \
  = "alpha=v0.1.0 beta=v0.2.0 gamma=unreleased" ] \
  || { echo "    the releases are not inferred from the tags:"; "$MJ" plan roadmap | sed 's/^/    | /'; exit 1; }

# A fresh checkout has neither declared enforcement hook, and their absence is a real doctor
# failure that would mask the ones this case is about. Written after the history above, so
# the fixture's own commits do not run doctor.
mkdir -p .git/hooks
printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push

# --- a table that matches the inferred releases in both directions is allowed, with or
#     without the tag's v
readme v0.1.0 v0.2.0
expect_exit 0 "$MJ" doctor
expect_grep 'OK   roadmap'
readme 0.1.0 0.2.0
expect_exit 0 "$MJ" doctor
expect_grep 'OK   roadmap'

# --- a document that invents a release no milestone was released in
readme v0.1.0 v0.2.0 v2.0.0
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL roadmap'
expect_grep 'lists release\(s\) no milestone was released in: v2.0.0'
# and watch reports the same thing as drift rather than passing it
expect_exit 11 "$MJ" watch

# --- a document that hides a release the tags show
readme v0.1.0
expect_exit 10 "$MJ" doctor
expect_grep 'omits release\(s\) the tags show a milestone was released in: v0.2.0'

# --- a tag makes a previously honest table dishonest, with the table and the records
#     untouched; a pre-release tag is not a release and does not
readme v0.1.0 v0.2.0
expect_exit 0 "$MJ" doctor
git tag v0.3.0-rc.1
expect_exit 0 "$MJ" doctor
git tag v0.3.0
expect_exit 10 "$MJ" doctor
expect_grep 'omits release\(s\) the tags show a milestone was released in: v0.3.0'

# --- a roadmap section that lists no releases is prose pointing at the projection, not a
#     second authority. This is the intended end state once the table is replaced by a link.
{ printf '# Fixture\n\n## Roadmap\n\nThe roadmap is not written here; run `majordomus plan roadmap`.\n\n'
  printf '## Contributing\n\nRead it, and AGENTS.md.\n'; } > README.md
expect_exit 0 "$MJ" doctor
expect_grep 'lists no releases'

# --- with no roadmap section the check holds trivially: the doctrine refuses the
#     regression, it does not require the table
printf '# Fixture\n\nNo roadmap here. See AGENTS.md.\n' > README.md
expect_exit 0 "$MJ" doctor
expect_grep 'no authored roadmap section'

# --- the doctrine is declared, dispatched, and names a claim that exists
"$MJ" doctrine list | grep -q '^majordomus.roadmap-integrity ' || { echo "    majordomus.roadmap-integrity is not in the registry"; exit 1; }
"$MJ" doctrine list | grep -qE '^majordomus.roadmap-integrity +blocking' || { echo "    majordomus.roadmap-integrity is not blocking"; exit 1; }
grep -q '^  - id: roadmap-derived$' "$ROOT/docs/CLAIMS.yaml" || { echo "    the claim it names is not in CLAIMS.yaml"; exit 1; }
[ -f "$ROOT/docs/claims/roadmap-derived.md" ] || { echo "    the claim has no detail page"; exit 1; }
