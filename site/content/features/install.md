+++
title = "One command to install, one to initialise, nothing else to trust"
description = "One distribution model owns every platform, artifact name and URL; the installer, the release build matrix, the installation guide, the website's install block and the public release metadata are generated from it, and a release publishes an artifact for every supported target or it is not published."
weight = 120
[extra]
id = "install"
status = "stable"
source = ".ai/repo/features/install.md"
+++
{% raw %}

## What it does

`majordomus init` creates the layer from the tool's skeleton and refuses to overwrite an
existing one; `--extend` adds what is missing and overwrites nothing. A repository set up
before the layer existed is moved by `migrate`, previewed with `--dry-run`, with a verified
backup. The tool itself needs a POSIX shell, git and a checksum program, and it never
reaches the network while it works.

With `--personal`, on `init` or on the installer, the same layer is created as one
person's own in a shared repository: it is covered in the git directory's own exclude file,
nothing is written to `.gitignore`, and the command fails unless `git status` reads the
same after it as before. `update` and `capture install` cover what they write the same
way, `doctor` re-verifies it on every run, and `init --extend --shared` hands the layer to
the team when it has made its case.

## What it does not do

It does not install anything into your project beyond `.ai/` and the files the policy
names, and it does not install git hooks: `doctor` names the two lines you still need to
add. A personal layer does not travel: it is one checkout's, on one machine, until it is shared. A Windows archive is not published while the shell half has no Windows answer, and the
distribution model says so rather than shipping an executable nothing can drive.
{% endraw %}
