# The roadmap is a projection, never a document.
#
# A doctrine that only passes is not evidence of anything, so this case moves the exit code
# in both directions: a document that invents a release, and a document that hides one. The
# third case is the one that matters longest — with no roadmap section at all, the check
# holds trivially, because the doctrine exists to refuse the regression rather than to
# require the table.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
# A fresh checkout has neither declared enforcement hook, and their absence is a real doctor
# failure that would mask the ones this case is about.
mkdir -p .git/hooks
printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push
PATH="$(dirname "$MJ"):$PATH"; export PATH
pj_init

rm_milestone() { # ID VERSION ORDER [DEP ...]
  local id="$1" ver="$2" ord="$3"; shift 3
  { cat <<Y
id: $id
title: Milestone $id
slug: $id
version: "$ver"
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
  } > ".majordomus/project/milestones/$id.yaml"
}
readme() { # write a README with the given version rows
  { printf '# Fixture\n\n## Roadmap\n\n| version | adds |\n|---|---|\n'
    for v in "$@"; do printf '| %s | something |\n' "$v"; done
    printf '\n## Contributing\n\nRead it.\n'
  } > README.md
}

rm_milestone alpha 0.1 10
rm_milestone beta  0.2 20 alpha
expect_exit 0 "$MJ" plan validate

# --- a table that matches the model in both directions is allowed
readme 0.1 0.2
expect_exit 0 "$MJ" doctor
expect_grep 'OK   roadmap'

# --- a document that invents a release the model does not declare
readme 0.1 0.2 2.0
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL roadmap'
expect_grep 'lists version\(s\) no milestone declares: 2.0'
# and watch reports the same thing as drift rather than passing it
expect_exit 11 "$MJ" watch

# --- a document that hides a release the model does declare
readme 0.1
expect_exit 10 "$MJ" doctor
expect_grep 'omits milestone version\(s\) the model declares: 0.2'

# --- adding a milestone makes a previously honest table dishonest, with the table untouched
readme 0.1 0.2
expect_exit 0 "$MJ" doctor
rm_milestone gamma 0.3 30 beta
expect_exit 10 "$MJ" doctor
expect_grep 'omits milestone version\(s\) the model declares: 0.3'

# --- a roadmap section that lists no versions is prose pointing at the projection, not a
#     second authority. This is the intended end state once the table is replaced by a link.
{ printf '# Fixture\n\n## Roadmap\n\nThe roadmap is not written here; run `majordomus plan roadmap`.\n\n'
  printf '## Contributing\n\nRead it.\n'; } > README.md
expect_exit 0 "$MJ" doctor
expect_grep 'lists no versions'

# --- with no roadmap section the check holds trivially: the doctrine refuses the
#     regression, it does not require the table
printf '# Fixture\n\nNo roadmap here.\n' > README.md
expect_exit 0 "$MJ" doctor
expect_grep 'no authored roadmap section'

# --- the doctrine is declared, dispatched, and names a claim that exists
"$MJ" doctrine list | grep -q '^roadmap_integrity ' || { echo "    roadmap_integrity is not in the registry"; exit 1; }
"$MJ" doctrine list | grep -qE '^roadmap_integrity +blocking' || { echo "    roadmap_integrity is not blocking"; exit 1; }
grep -q '^  - id: roadmap-derived$' "$ROOT/docs/CLAIMS.yaml" || { echo "    the claim it names is not in CLAIMS.yaml"; exit 1; }
[ -f "$ROOT/docs/claims/roadmap-derived.md" ] || { echo "    the claim has no detail page"; exit 1; }
