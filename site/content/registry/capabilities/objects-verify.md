+++
title = "objects.verify"
description = "Read every file the layer was built from and compare it with what this process is serving. The index is built once at start-up and kept, which is what makes every other request cost nothing and what makes a file edited afterwards be served as it was; this is how a running server says whether that has happened, without being restarted to find out. A file that is one object is compared byte for byte; a collection file, whose objects the index keeps as members rather than as text, is compared by size, and every finding says which comparison was made. It reads every file of the layer, so it reports its progress file by file and stops when it is asked to."
weight = 36
slug = "objects-verify"
[extra]
id = "objects.verify"
source = "apps/majordomus-cli/src/capability/builtin/objects.rs"
+++
