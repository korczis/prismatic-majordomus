+++
title = "Follow a procedure the repository defines, and add another one"
description = "Read the repository''s skills, take one as it is written, and add a new one by writing its file — no registry, no code, and a check that says what it examined."
weight = 25
[extra]
id = "follow-a-skill-the-repository-defines"
source = ".ai/repo/use-cases/follow-a-skill-the-repository-defines.md"
category = "extension"
maturity = "guaranteed"
+++

## Situation

A repository has procedures that its workers, human and machine, are supposed to follow: how a review is done here, how a change is carried out, how the site is deployed. They live in somebody's head, in a wiki nobody updates, or pasted into a provider's own configuration file where the next tool cannot read them.

## Outcome

Each procedure is one directory under the layer's skills section holding `SKILL.md`, discovered because the file exists. `skills list` is the inventory, `skills show` is the procedure as written with the path it came from, and `skills check` reports what it examined — skills, examples and references — so a pass is evidence rather than a word. The same catalogue is what the website's skills pages and the MCP resources are projected from, so no surface keeps its own list.
