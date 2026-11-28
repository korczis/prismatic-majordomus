+++
title = "mesh.hub.uninstall"
description = "Stop the hub and its update through the service manager (launchctl bootout, systemctl --user disable --now) and remove every unit file mesh hub install writes. Without `apply` it plans; a host without the units owes nothing. Offered on the command line only."
weight = 132
slug = "mesh-hub-uninstall"
[extra]
id = "mesh.hub.uninstall"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
