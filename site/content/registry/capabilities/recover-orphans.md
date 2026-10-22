+++
title = "recover.orphans"
description = "Walk the publish temps of every record store, the rename temps under the layer's local half and the tracked sessions section, and the site generator's staging directories at the repository root; give each a verdict by reading its age and then its content; and, unless the call is a check, remove the ones the verdict says nothing is holding. Nothing is deleted for being unrecognised: a temp holding the only copy of a record is reported as `incomplete` and left for the caller to publish, content this version cannot classify is `foreign` and left exactly where it is, a candidate whose age this platform cannot read is `unmeasurable` and never a candidate, and no directory is ever removed. The threshold is required and has no default, because a sweep measuring against an absent threshold would find every file stale."
weight = 87
slug = "recover-orphans"
[extra]
id = "recover.orphans"
source = "apps/majordomus-cli/src/capability/builtin/recover.rs"
+++
