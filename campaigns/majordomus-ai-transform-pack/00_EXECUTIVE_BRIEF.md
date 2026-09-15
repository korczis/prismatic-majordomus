# Executive brief

## Transformation objective

Change Majordomus from the current model:

```text
Majordomus installation
+
repository-specific .majordomus/
    policy
    profiles
    prompts
    providers
    project model
    generated projection state
    mutable operational state
```

into the target separation:

```text
Majordomus distribution
    read-only/stateless relative to the managed repository
    CLI + parsers + validators + schemas + migrations + default skeleton

.ai/
    portable provider-neutral repository AI layer

.ai/repo/
    tracked canonical repository-specific AI/governance context

.ai/local/
    gitignored machine/checkout-local AI and operational state
```

The optional path `.majordomus/` in a consumer repository is **only an
installation location** for the read-only Majordomus distribution (for example
a Git submodule, symlink, or vendored checkout). It is not part of the project
data format and must never be required.

Majordomus may instead be installed globally, from `~/tools/majordomus`, a
Debian package, PATH, Nix, Homebrew, or any future packaging method. All
installation forms MUST behave identically toward repository data.

## The architectural point

`.ai/` is not a "Majordomus config directory".

It is a portable, provider-neutral repository AI layer that remains
intelligible and useful without the Majordomus executable.

Majordomus is the reference control plane that can initialize, validate,
resolve, project, reconcile, migrate, and enforce this layer.

## Dogfooding

The Majordomus source repository itself MUST use `.ai/` to describe how AI works
on Majordomus.

In the Majordomus source repository:

```text
bin/
lib/
share/
test/
...
.ai/
```

are expected.

A nested `.majordomus/` is NOT expected because the repository itself is the
tool distribution.

In a consumer repository:

```text
.ai/                 project AI layer
.majordomus/         optional tool installation only
```

may coexist.

## Strategy

Do not combine every conceptual improvement into one uncontrolled rewrite.

1. Establish the new path/ownership model.
2. Move existing canonical data with minimal semantic change.
3. Move mutable state local and gitignored.
4. Convert provider instruction files into thin bootstraps.
5. Introduce versioned vendored rule objects.
6. Move doctrine authority into rule objects without losing doctrine wiring.
7. Rewire docs/site/tests/doctor.
8. Remove legacy project-data dependence on `.majordomus/`.
9. Only after parity is proven, add richer skills/workflows/knowledge behavior.

The resulting structure should be simpler to explain than the current one.
