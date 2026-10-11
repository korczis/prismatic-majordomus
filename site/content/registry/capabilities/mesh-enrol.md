+++
title = "mesh.enrol"
description = "Add this machine's public key to trust.allow of the repository's mesh declaration, under a comment naming the machine, creating the node identity first when there is none. Idempotent: a listed key changes nothing. The comments and every other line of the declaration stay as they were, and an edit that would not parse back as a declaration is refused before anything is written. Under deny_unknown a machine opens its mesh only once its key is listed, so this is the one step that joins a machine; commit the file and restart the server."
weight = 98
slug = "mesh-enrol"
[extra]
id = "mesh.enrol"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
