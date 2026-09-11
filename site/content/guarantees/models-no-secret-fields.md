+++
title = "The model catalogue reports whether a vendor's named credential variable is set and can carry no secret value — the schema has no field one could hide in"
description = "A vendor entry may name the environment variable its credential lives in"
weight = 170
[extra]
claim_id = "models-no-secret-fields"
status = "guaranteed"
source = "docs/claims/models-no-secret-fields.md"
+++
{% raw %}

## What it means

A vendor entry may name the environment variable its credential lives in
(`credential_env: ANTHROPIC_API_KEY`), and every listing answers one bit about it:
configured or not. No code path reads the variable's value; no field of the catalogue,
its schema, or any projection can carry a key, a token or a password — so a leak
through the models surface would require a new field to be added first, which is a
reviewed change, not an accident.

## How it works

Presence is `std::env::var_os(name).is_some()` and nothing more
(`apps/majordomus-cli/src/capability/builtin/models.rs`); the answer is a boolean
beside the vendor. The property is held by a test that walks the catalogue's own JSON
schema (`apps/majordomus-cli/src/models/mod.rs`) and refuses any property name that
smells like a credential — so the schema, not a convention, is what cannot hold a
secret. `test/cases/131_models.sh` proves the operator-visible half: setting the named
variable flips the report to "configured" and the value appears nowhere in the output.
{% endraw %}
