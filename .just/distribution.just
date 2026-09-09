# How this project reaches somebody else's machine: the model, the releases, the installer.

# How this project is packaged, published and installed: the install command, the prefix, every declared target.
[group('distribution')]
distribution *args: build
    "{{rust_bin}}" distribution show {{args}}

# Whether the advertised one-line installation works right now; exit 10 naming the missing link and the command that changes it. Local and offline.
[group('distribution')]
distribution-status *args: build
    "{{rust_bin}}" distribution status {{args}}

# Everything about the distribution that can be checked without a network: the model, every release record, and the projections generated from them. Exit 10 on the first violation.
[group('distribution')]
distribution-check: build
    "{{rust_bin}}" distribution validate
    "{{rust_bin}}" generate distribution --check
