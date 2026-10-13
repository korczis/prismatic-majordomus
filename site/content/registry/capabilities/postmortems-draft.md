+++
title = "postmortems.draft"
description = "Asks the forge, through its own command-line tool, for one run and its failed jobs, and returns the text of a `draft` record for it: the front matter the schema asks for, with the cause class `undetermined`, and the four required sections, each saying that nothing is established yet. The identity is `pm-`, the day of the run, the workflow and the run's identifier. The text is returned, with the path a record of that identity lives at and the records that already name the run; nothing is written. When `runs` is given the run is looked up there and the forge is not asked. Exit 0 when drafted, 12 when the forge could not be asked."
weight = 150
slug = "postmortems-draft"
[extra]
id = "postmortems.draft"
source = "apps/majordomus-cli/src/capability/builtin/postmortems.rs"
+++
