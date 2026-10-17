+++
title = "A task on a repository whose mesh is enabled works on the mesh"
description = "When the repository declares its mesh enabled, an active task's scope is claimed on the mesh — `start` brings the checkout's shared server up and claims it, `check` and `finish` claim it again when a server answers — and a task completed without that claim is refused."
weight = 45
[extra]
id = "majordomus.task-on-the-mesh"
source = "share/standard/majordomus/rules/task-on-the-mesh.v1.md"
+++
