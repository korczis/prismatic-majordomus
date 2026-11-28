+++
title = "mesh.hub.install"
description = "Render the hub's service from the mesh declaration and this checkout — on macOS the launch agent dev.majordomus.hub (RunAtLoad, KeepAlive) and the update agent dev.majordomus.hub-update (at load and every hour), on Linux the systemd user units majordomus-hub.service (Restart=always, no start limit) and majordomus-hub-update.service with its hourly timer — each running the installer's launcher, whose path does not move with the version, on this checkout with --host 0.0.0.0, the declared hub port and --idle 0. Refuses on a machine the declaration does not name as a hub and for a checkout on a branch, because a hub serves a dedicated checkout detached at a release tag. Without `apply` it plans: every file missing or drifted and every manager command owed. With `apply` it writes them and loads them; a second install finds everything current and does nothing. A `root` is a staging home: files only, no command. Offered on the command line only: it installs a service, which nothing reachable over HTTP or MCP may do."
weight = 131
slug = "mesh-hub-install"
[extra]
id = "mesh.hub.install"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
