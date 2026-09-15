# Compatibility, migration safety, and rollback

## Compatibility goal

The transformation may change on-disk project paths and local-state durability,
but should preserve existing public command semantics wherever not explicitly
changed.

Avoid unnecessary command renames.

## Schema version

`.ai/manifest.yaml` MUST carry an explicit format/schema identifier such as:

```text
ai-repository/v1
```

Majordomus must fail clearly when the repository format is newer than the
executable supports.

Do not infer compatibility solely from Majordomus binary version.

## Legacy detection

Legacy marker:

```text
.majordomus/policy.yaml
```

New portable marker:

```text
.ai/manifest.yaml
```

Optional tool-install marker may include:

```text
.majordomus/bin/majordomus
```

Never assume `.majordomus/` means project config in the new architecture.

## Mixed state

If both:

```text
.majordomus/policy.yaml
```

and a new tool distribution occupy `.majordomus/`, do not overwrite.

Return a typed refusal explaining the collision and the exact safe manual path.

## No silent auto-migration on ordinary commands

Prefer:

```text
majordomus migrate
```

or an explicit migration flag.

Ordinary `check`, `doctor`, `start`, etc. may detect legacy layout and explain
the required migration, but should not perform destructive moves implicitly.

A temporary read compatibility mode is acceptable only if it is centralized,
clearly deprecated, tested, and removed by the end of the transformation if
possible.

## Migration transaction

Use staging + atomic rename/copy where portable.

Before writing:

- validate source formats,
- inventory source files,
- detect destination conflicts,
- confirm Git ignore plan,
- render preview.

After writing:

- validate `.ai`,
- run doctor,
- compare expected file inventory,
- preserve unknown source files or refuse.

Only then remove legacy known paths.

## Rollback

The safest rollback is Git for tracked canonical files plus a pre-migration
local-state backup.

Because `.ai/local/` is ignored, migration should create a temporary ignored
backup/snapshot such as:

```text
tmp/majordomus-migrate-backup/<timestamp>/
```

or equivalent.

Do not claim rollback if local state has been moved/deleted without a recovery
copy.

The migration command should print where its local backup lives.

## Current Majordomus repository dogfood migration

The current source repository's `.majordomus/` is legacy project customization,
not the tool distribution.

Migrate its project data into `.ai/`.

Do not move root `bin/lib/share/test` into `.majordomus/`.

After dogfooding migration, root `.majordomus/` should disappear unless the
operator separately chooses to create an optional self-installation, which is
not necessary.
