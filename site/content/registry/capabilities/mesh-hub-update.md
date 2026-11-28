+++
title = "mesh.hub.update"
description = "What the hub's update agent runs every hour. It asks the release origin for the latest release, the executable for its version and the hub for the version it serves and the sessions attached to it, and decides: current, install the newer release, restart the hub onto the installed one, defer while sessions are attached, or skip a version that failed here before until a newer one is published. With `apply` it carries the decision out: the verified installer (digest and attestation), the hub checkout detached at the release's tag (refused when the checkout has uncommitted changes), a restart through the service manager, and the hub's own answer within 90 seconds; a hub that does not answer with the new version gets the previous release and checkout back and the version is recorded as failed on this node. Every run is recorded under ~/.local/state/majordomus/hub-update.json, which mesh hub reads. Offered on the command line only: it installs a release and restarts a service."
weight = 133
slug = "mesh-hub-update"
[extra]
id = "mesh.hub.update"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
