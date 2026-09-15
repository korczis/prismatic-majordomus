+++
title = "Recovery of the record stores"
description = "The stray files a killed publish, an interrupted rename or an unfinished site build leave in the record stores, classified by reading the clock and the content, and swept exactly once. This is the capability that backs `majordomus recover` (ADR 0040): the command no longer decides what a stray is, it asks."
weight = 28
slug = "recover"
[extra]
id = "recover"
source = "apps/majordomus-cli/src/capability/builtin/recover.rs"
+++
