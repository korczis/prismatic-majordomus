# Installed and wired, with one moment, one audience and one area under the why section:
# enough for the catalogue to hold something, and small enough to read. Nothing here
# registers the entries — the section and its three source classes come from `init`.
. "$FIXTURE_SETUP/installed-wired.sh"
mkdir -p .ai/repo/why/moments .ai/repo/why/audiences .ai/repo/why/areas
cat > .ai/repo/why/audiences/small-team.md <<'MD'
---
schema: audience/v1
id: small-team
kind: audience
title: A small team
summary: 'A team small enough that nothing is written down and large enough that it should be.'
status: draft
weight: 10
---

# A small team

Because the fixture says so.
MD
cat > .ai/repo/why/areas/continuity.md <<'MD'
---
schema: area/v1
id: continuity
kind: area
title: Continuity
summary: 'What survives when a session ends.'
status: stable
weight: 10
---

# Continuity

Because the fixture says so.
MD
cat > .ai/repo/why/moments/the-same-explanation-twice.md <<'MD'
---
schema: moment/v1
id: the-same-explanation-twice
kind: moment
title: 'Explaining the same thing to the next session'
hook: 'explained the same thing to the next session'
summary: 'Knowledge that lives only in a conversation has to be re-transmitted by hand.'
status: stable
severity: medium
frequency: common
weight: 10
featured: true
audiences: [small-team]
areas: [continuity]
tags: [context]
signals:
  - id: explained-again
    text: 'The same explanation was typed into a fresh session again this week.'
examples:
  - id: monday
    audience: small-team
    title: 'Monday knew it'
    before: 'Tuesday rediscovers it.'
    after: 'Tuesday reads the handover.'
  - id: tuesday
    audience: small-team
    title: 'Tuesday did not'
    before: 'The context was in a window that closed.'
    after: 'The context is a record.'
  - id: wednesday
    audience: small-team
    title: 'Wednesday asked a person'
    before: 'The only copy was in somebody.'
    after: 'The copy is in the repository.'
commands: [handover, context]
---

## The moment

Because the fixture says so.

## Why it happens

Because the fixture says so.

## What it does not do

Nothing the fixture does not say.
MD
git add -A >/dev/null 2>&1
git commit -qm "one moment" >/dev/null 2>&1
