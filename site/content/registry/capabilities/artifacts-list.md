+++
title = "artifacts.list"
description = "The manifest `majordomus generate` commits as docs/generated/artifacts.json, reconciled with the working tree: every document with the encodings it is written in, and every file with its format, schema, source, size, hash and whether the file on disk still matches. Optionally narrowed to one document or one encoding. Reads only; `majordomus generate` writes and `majordomus generate --check` is the byte-for-byte verdict."
weight = 1
slug = "artifacts-list"
[extra]
id = "artifacts.list"
source = "apps/majordomus-cli/src/capability/builtin/artifacts.rs"
+++
