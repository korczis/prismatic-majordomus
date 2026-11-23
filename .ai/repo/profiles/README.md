---
schema: context/v1
id: ai.repo.profiles
kind: context
title: Profiles
description: How much context, effort and ceremony a task gets, declared per working mode.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [lib/common.sh, share/allow/profile.txt]
---

# Profiles

A profile is one working mode: how much context a task of that kind is assembled with,
how much effort it is worth, how the worker reports, and what it must produce before it
may finish. One YAML file per mode, its `name` equal to the file name; the policy names
the default (`profiles.default`), and a task carries the profile it was started with.

The keys are closed by `share/allow/profile.txt` — a key nothing reads is an error, not a
silent no-op. `context.*` switches the sections the context builder assembles;
`verification.*` is what `majordomus finish` refuses to skip. Nothing here decides *what* a
worker does; it decides how much the tool spends on helping.

## Who reads a profile

No resolver exists yet. Each command opens the file itself: `start`, `check`, `context`,
`finish`, `watch`, `doctor` and `update` in shell, and `reasoning`, `proceed.step` and the
object index in the executable. `docs/PROFILES.md` traces every reader to its line and
marks each link proven, advisory, duplicated or missing.
`.ai/repo/adrs/0232-one-resolver-in-the-profile-module-owns-the-effective-profile.md`
(ADR 0232) decides where this goes next. One resolver, in the executable's `profile` module, owns
the effective profile. Requirements and limits intersect, and preferences take
precedence. A key is added to `majordomus.profile/v1` only when it is optional and comes
with the code that reads it.

## Adding one

```bash
$EDITOR .ai/repo/profiles/my-mode.yaml     # name must equal the file name
majordomus doctor                          # parses every profile, resolves the default
majordomus start "..." --profile my-mode
```
