+++
title = "self.update"
description = "Reads the release metadata the installation's share/distribution.yaml names (releases/latest.json, or releases/<tag>.json for a named version; MAJORDOMUS_RELEASE_BASE_URL overrides the base as it does for install.sh) and judges it against the running version: an upgrade applies only to a newer, stable, listed release; a downgrade, a pre-release and an unlisted version are refused with the reason. Without --apply it writes nothing. With --apply it fetches install.sh from the same base and runs it with --version <tag>, --prefix and --install-dir of this installation, then reads the installed version back from the launcher. Exit 0 nothing to do or done, 10 refused or the installer failed, 12 not answered."
weight = 205
slug = "self-update"
[extra]
id = "self.update"
source = "apps/majordomus-cli/src/capability/builtin/self_update.rs"
+++
