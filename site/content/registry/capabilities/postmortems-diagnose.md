+++
title = "postmortems.diagnose"
description = "Searches the text of a failure — a failing log, a job's summary, an excerpt — for the signals of every record, ignoring case and matching each signal as written. Every record with at least one signal in the text is returned with its status, cause, the signals found, its components, what fixed it, what now prevents it and what else it concerned, the record with the most signals first and among equals the newest. No weighting and no closeness: a record is named because its own words are in the log. Reads the index and nothing else."
weight = 149
slug = "postmortems-diagnose"
[extra]
id = "postmortems.diagnose"
source = "apps/majordomus-cli/src/capability/builtin/postmortems.rs"
+++
