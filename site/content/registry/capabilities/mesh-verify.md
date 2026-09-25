+++
title = "mesh.verify"
description = "Run a live verification: cooperation is active, the repository identity resolves, the endpoints are reachable beyond loopback, the heartbeat beats, the journal holds no stuck gap; then one sync round with every peer this runtime dials, timed, and whether both sides now hold the same high-water marks. Each failed check names its impact and remedy. Changes nothing but the journal's replication."
weight = 78
slug = "mesh-verify"
[extra]
id = "mesh.verify"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
