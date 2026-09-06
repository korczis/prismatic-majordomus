+++
title = "Adopt a repository that already has its own rules"
description = "Install the supervisory layer into a repository with a hand-written CLAUDE.md, without the tool taking ownership of a file somebody else wrote."
weight = 1
[extra]
id = "adopt-an-existing-repository"
source = ".ai/repo/use-cases/adopt-an-existing-repository.md"
category = "adoption"
maturity = "guaranteed"
+++

## Situation

The repository already has a governance root — a CLAUDE.md, an AGENTS.md, a contributing guide people actually follow. A tool that regenerates that file wholesale would overwrite months of authored judgement, so the usual answer is to not adopt the tool at all.

## What you run

- `init`: writes .ai/ and refuses if an installation is already there
- `update`: renders the projection into the region between the markers, leaving the rest of the file alone
- `doctor`: proves the projection matches its own stamp and the declared enforcement is actually invoked

## Outcome

The authored text stays authoritative and the generated section sits inside it, stamped like any other projection. An edit outside the markers is never drift; an edit inside them is caught.
