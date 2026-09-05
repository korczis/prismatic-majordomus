+++
title = "Move a repository from the pre-.ai layout to the layer"
description = "See what a migration would move, move it with a backup, and prove the result is a layer the tool reads."
weight = 29
[extra]
id = "migrate-from-the-old-layout"
source = ".ai/repo/use-cases/migrate-from-the-old-layout.md"
category = "adoption"
maturity = "guaranteed"
+++

## Situation

A repository installed Majordomus before the `.ai/` layer existed and keeps its policy under `.majordomus/`. Every command refuses to read it, and the maintainer wants to know what will move before anything does.

## Outcome

`migrate --dry-run` lists every move; `migrate` performs it with a backup and writes the manifest; a second run says there is nothing to migrate, and `doctor` proves the layer is real.
