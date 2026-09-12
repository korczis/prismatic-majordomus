# majordomus-covers: none
# A catalogue loader's cost must not grow with the size of the corpus it loads.
#
# Three of this repository's loaders used to run a process per document. mj_uc_load
# flattened each use case's front matter and each scenario on its own; mj_adr_load did the
# same for each decision; and mj_adr_rel_valid, reached from the record validator through a
# command substitution, reloaded the whole effective rule set inside every one of those
# subshells — 3350 mj_rule_scan calls for fifty-one decisions, where one load is enough.
# `mj_yaml_flatten_many` had existed the whole time.
#
# The guard is a scaling property, not a number. A number goes stale the day the vendor rule
# set grows, and the reviewer who has to update it learns nothing from doing so; the property
# is the thing that was wrong. So the corpus is loaded at two sizes and what is asserted is
# that the second costs no more parses than the first. A loader written one-document-at-a-time
# again fails this the moment someone writes it, whatever the constant is:
#
#   per document   adding 12 use cases adds 24 flattens, 12 decisions add 12
#   per subshell   adding 12 decisions that name a rule adds 12 whole rule-set loads
#   batched        adding either adds none
#
# yaml_flatten is the counter to read because it is the one-file spelling: mj_yaml_flatten
# parses exactly one file per process, so its count IS the process count. Its batched
# sibling is counted separately as yaml_flatten_many and is allowed to grow — one more call
# per corpus is the whole point.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null && git commit -qm install

UC=.ai/repo/use-cases
AD=.ai/repo/adrs

# a rule the vendor set actually has, so that the `related: rule:` branch of
# mj_adr_rel_valid is exercised rather than skipped — that branch is where the reloading
# happened, and a fixture that never reaches it cannot catch its return
rule="$("$MJ" rules list 2>/dev/null | awk 'NR > 0 && $1 ~ /^[a-z][a-z0-9.-]+$/ { print $1; exit }')"
[ -n "$rule" ] || { echo "    the vendor rule set listed no rule to point a decision at"; exit 1; }

# ---------------------------------------------------------------- the corpus, at one size
# n use cases and n decisions, each decision naming the rule above
seed() {
  local from="$1" to="$2" i pad
  i="$from"
  while [ "$i" -le "$to" ]; do
    pad="$(printf '%03d' "$i")"
    cat > "$UC/probe-$pad.md" <<MD
---
id: probe-$pad
kind: use-case
title: 'Probe $pad'
summary: 'One document of a corpus a loader has to read.'
category: adoption
status: active
target: advisory
actors: [operator]
difficulty: basic
commands: [version]
doctrines: []
claims: []
responsibilities: []
applications: []
---

# Situation

A corpus of a known size.

# Scenario

\`\`\`yaml
setup: bare
given:
  - 'nothing installed'
steps:
  - id: print
    run: ['version']
    note: 'the version string'
    expect:
      exit: 0
then:
  - 'the corpus has one more document'
\`\`\`

# Outcome

One more document.
MD
    cat > "$AD/9$pad-probe-$pad.md" <<MD
---
id: adr-9$pad
kind: adr
schema: adr/v1
status: proposed
date: 2026-09-12
title: 'Probe decision $pad'
related:
  - rule:$rule
provenance:
  origin: authored
---

# Context

A decision of a corpus a loader has to read.

# Decision

Be one document.

# Consequences

The corpus is one document larger.
MD
    i=$((i + 1))
  done
}

# ---------------------------------------------------------------- the measurement
# The one-file parse count of one command. MJ_TIMING puts the counters on stderr, so stdout
# goes to /dev/null and stderr into the pipe; an absent counter reads as 0, which is a
# legitimate answer and not a missing measurement.
flattens() {
  local counter="$1"; shift
  MJ_TIMING=1 "$@" 2>&1 >/dev/null \
    | awk -v c="$counter" '$1 == "count" && $3 == c { n = $2 } END { printf "%d\n", n + 0 }'
}

# Both sizes are loaded through the same commands, in the same repository, one after the
# other. A ratio between two corpora in two repositories would be measuring the fixture.
seed 1 4
git add -A >/dev/null && git commit -qm 'corpus of four'
uc_small="$(flattens yaml_flatten "$MJ" usecase list)"
ad_small="$(flattens yaml_flatten "$MJ" adr list)"

seed 5 16
git add -A >/dev/null && git commit -qm 'corpus of sixteen'
uc_large="$(flattens yaml_flatten "$MJ" usecase list)"
ad_large="$(flattens yaml_flatten "$MJ" adr list)"

printf '    use cases   4 -> %s parses, 16 -> %s parses\n' "$uc_small" "$uc_large"
printf '    decisions   4 -> %s parses, 16 -> %s parses\n' "$ad_small" "$ad_large"

# the corpora really were read: a loader that discovered nothing would pass every bound below
expect_exit 0 "$MJ" usecase list
expect_grep 'use cases: 1[0-9] in '
expect_exit 0 "$MJ" adr list
seen="$("$MJ" adr list | grep -c '^adr-9' || true)"
[ "$seen" -ge 16 ] || { printf '    only %s of the 16 probe decisions were discovered\n' "$seen"; exit 1; }

# ---------------------------------------------------------------- the property
# Twelve more documents may cost a few more parses — a loader is allowed a fixed extra
# read, and one document more in a list is not a regression. What it may not do is cost one
# per document: twelve is the number a per-document loader adds, so the bound is under it.
bound=12
for pair in "use cases:$uc_small:$uc_large" "decisions:$ad_small:$ad_large"; do
  what="${pair%%:*}"; rest="${pair#*:}"; small="${rest%%:*}"; large="${rest##*:}"
  growth=$((large - small))
  if [ "$growth" -ge "$bound" ]; then
    printf '    %s: 12 more documents cost %s more one-file parses (%s -> %s).\n' \
      "$what" "$growth" "$small" "$large"
    printf '    That is a process per document. mj_yaml_flatten_many in lib/common.sh\n'
    printf '    parses a whole corpus in one process, and mj_front_cache_build wraps it for\n'
    printf '    a loader that still wants to ask one record at a time; lib/usecase.sh and\n'
    printf '    lib/adr.sh are the two callers to read. A validator reached through a command\n'
    printf '    substitution runs in a subshell, so anything it caches is lost when it exits:\n'
    printf '    load it once in the walker, before the loop.\n'
    exit 1
  fi
done

# and the batched spelling is the one doing the work: at least one call to it per corpus
many="$(flattens yaml_flatten_many "$MJ" usecase list)"
[ "$many" -ge 1 ] || { echo "    the use-case loader made no batched parse at all"; exit 1; }
many="$(flattens yaml_flatten_many "$MJ" adr list)"
[ "$many" -ge 1 ] || { echo "    the decision loader made no batched parse at all"; exit 1; }
