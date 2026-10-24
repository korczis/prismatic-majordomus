+++
title = "served.forge"
description = "Asks the forge, through its own command-line tool and with the caller's credential, for the newest deployments to every environment and the newest releases, in one query, and records the answer in the checkout-local state `deploy.observed` reads. Exit 0 when the forge answered and the answer was recorded, 12 when it could not be asked (no tool, no credential, no network, a repository the forge does not know): what was recorded before then stands, with its age."
weight = 190
slug = "served-forge"
[extra]
id = "served.forge"
source = "apps/majordomus-cli/src/capability/builtin/served.rs"
+++
