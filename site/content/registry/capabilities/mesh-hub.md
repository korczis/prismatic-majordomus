+++
title = "mesh.hub"
description = "Whether this machine's hub runs as its host's service and follows the release (I4816): each unit file — the launchd agent and its update agent on macOS, the systemd user unit, the update service and its hourly timer on Linux — as missing, drifted or as rendered, and whether the manager has it loaded and running; whether the hub answers on its port, with the version it serves, the sessions attached to it and the runtimes it is linked with; the published, installed and served versions; the update's last run; and every linked runtime behind the published release. Fails when a unit is missing, drifted or not loaded, the hub does not answer, it is linked with nobody, the update has not run within two hours, or it serves an older release than is published with no update on it. Reads the host; changes nothing."
weight = 130
slug = "mesh-hub"
[extra]
id = "mesh.hub"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
