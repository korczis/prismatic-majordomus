+++
title = "Program map"
description = "Every module of the CLI, every file it reads and writes, scanned out of the shell sources rather than drawn by hand."
template = "map.html"
[extra]
graph = "architecture"
+++
Nobody drew this. `scripts/lib/architecture-scan.awk` reads `bin/majordomus` and `lib/*.sh`
on every build and reports what it can prove: which module sources which, and which file
under `.majordomus/` each module names. A module that stops touching a file loses its edge
the same day. A diagram that cannot go stale is worth more than a prettier one that can.

## What an edge means

Direction comes from the line the reference sits on — the redirection it is behind, or the
command word that introduces it. A path handed straight to a helper is drawn as **uses**,
not guessed at, because a wrong arrow here would be a claim the sources do not support.
Read the scanner if you want the exact rule; it is forty lines.
