+++
title = "pack.verify"
description = "Read a pack directory inside the repository against its pack.json: every file it names is present with its digest and nothing else is, every carried file's content has the digest its marker records, no carried path is a binary, an artifact, a link or a gitlink under the profile the manifest names, no file is over the profile's token budget or holds a NUL byte or a leak, and there are no more files than the profile allows. An unreadable manifest is unmeasured, never a pass."
weight = 127
slug = "pack-verify"
[extra]
id = "pack.verify"
source = "apps/majordomus-cli/src/capability/builtin/pack.rs"
+++
