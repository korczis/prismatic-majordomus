+++
title = "machine.schedule"
description = "The per-user scheduler entry that runs `majordomus machine sweep --reclaim` every fifteen minutes from this checkout: a launchd agent (~/Library/LaunchAgents/dev.majordomus.sweep.plist) on macOS, a systemd --user service and timer (~/.config/systemd/user/majordomus-sweep.*) on Linux. `status` reports whether its files are there; `install` writes them and loads them with launchctl or systemctl, each command's outcome reported; `remove` unloads and deletes them. The entry starts the `majordomus` on PATH unless another program is named, so an upgrade does not leave it pointing at a removed version. Refused on a platform with neither scheduler."
weight = 102
slug = "machine-schedule"
[extra]
id = "machine.schedule"
source = "apps/majordomus-cli/src/capability/builtin/machine.rs"
+++
