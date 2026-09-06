# The repository environment: the one typed snapshot of what this checkout is, which the
# direnv banner, the HTTP route `/api/v1/environment`, the MCP resource
# `majordomus://environment` and the Cockpit all render. `docs/ENVIRONMENT.md` is the
# whole of it.
#
# A recipe's description is the single comment line immediately above it — `just` reads no
# more than that — and it is what `just --list`, the generated reference and the banner all
# show. A description split over two lines silently becomes its second line.

# What this checkout is: project, version control, toolchains, what the layer holds, workflows, provider projections, local services. Extra arguments pass through (--format json).
[group('env')]
env *args: build
    "{{rust_bin}}" env {{args}}

# Where one value of the snapshot came from and how far it can be trusted; `just env-explain vcs` narrows it to one field or prefix.
[group('env')]
env-explain *args: build
    "{{rust_bin}}" env explain {{args}}

# The banner direnv draws on entering this repository, in full, whatever the terminal would have chosen for itself.
[group('env')]
env-banner *args: build
    "{{rust_bin}}" env banner --mode full {{args}}
