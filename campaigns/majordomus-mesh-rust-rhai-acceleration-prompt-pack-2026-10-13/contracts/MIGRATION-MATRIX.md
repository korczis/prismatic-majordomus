# Rust / Rhai / plugin migration matrix

| Kind | Authority | Script/tool status | Test strategy |
|---|---|---|---|
| Domain operation, IO, Git/network, authz | Rust core | remove shell domain logic | Rust unit+integration, negative permissions |
| Workflow composition and policy flow | Rhai (bounded) | replace brittle shell orchestration | Rhai fixture + host boundary tests |
| Safe third-party composition | Approved Rhai plugin | load manifest, explicit host grants | external package fixture and cross-surface parity |
| Untrusted computation | Separate process or constrained WASM (when justified) | never arbitrary in-process native | adversarial budgets and privilege tests |
| Rust native extension | Statically linked trusted builtin | typed capability registry | compile/test and version compatibility |
| CLI adapter | Derived from canonical operation | thin forwarding parser | CLI/API/MCP parity |
| just recipes | Derived ergonomic names | no domain logic | generation drift and argument forwarding |
| .envrc / POSIX bootstrap | Minimal exceptional wrapper | bounded, no network/build per cd | shell compatibility, budget |
| GitHub workflow YAML | Runner adapter for canonical Rust operations | minimum script bridge | CI runs exact gates, no swallowed errors |
| Legacy shell tests | Gradually port semantics to Rust | keep until parity proven | mutation/negative-case testing |
