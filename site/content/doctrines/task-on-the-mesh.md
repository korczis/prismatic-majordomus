+++
title = "A task on a repository whose mesh is enabled works on the mesh"
description = "When the repository declares its mesh enabled, an active task's scope is claimed on the mesh — `start` claims it when the checkout's shared server answers and `finish` claims it again before judging — and a task completed without that claim is refused."
weight = 44
[extra]
id = "majordomus.task-on-the-mesh"
source = "share/standard/majordomus/rules/task-on-the-mesh.v1.md"
+++
