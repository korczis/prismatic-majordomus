# An MCP client opened in the repository starts the server itself, building the executable when it must, through the client configurations at the root and bin/majordomus-mcp

## What it means

Nobody runs a server by hand. Open this repository in Claude Code, Gemini CLI or Codex and the client finds its configuration at the root, spawns `bin/majordomus-mcp`, and speaks MCP to it; the launcher builds the Rust executable when it is missing or older than its sources, then runs `majordomus mcp`, which starts the repository's shared server or attaches to the one already running. The first client to open the repository becomes the server; the second shares it.

## How it works

`bin/majordomus-mcp` (portable bash) resolves the repository from its own location, picks `apps/majordomus-cli/target/<profile>/majordomus` (`MAJORDOMUS_BUILD_PROFILE`, default `debug`), compares it with the sources with `find -newer`, runs `cargo build --locked` when needed with the output on stderr, sets `MAJORDOMUS_SHARE` to the distribution's `share/` beside it, and `exec`s the executable with `mcp` and every argument it was given. It never writes to stdout, which belongs to the protocol. `MAJORDOMUS_BIN` names an executable to use without building; `MAJORDOMUS_NO_BUILD=1` refuses to build and exits 12 with the command to run. `.mcp.json` (Claude Code, project scope, approved once), `.gemini/settings.json` (Gemini CLI, `mcpServers.majordomus`) and `.codex/config.toml` (Codex, `[mcp_servers.majordomus]`, loaded when the project is trusted) all name it.

## How to see it

```bash
cat .mcp.json .gemini/settings.json .codex/config.toml            # the same launcher in each
bin/majordomus-mcp --help                                         # builds if needed, then: Usage: majordomus mcp ...
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"you","version":"0"}}}\n' | bin/majordomus-mcp
# stderr: shared server listening on http://127.0.0.1:8741 (swagger ui http://127.0.0.1:8741/docs, ...); stdout: the initialize result
claude                                                            # in this directory: approve the project server once; the majordomus_* tools are there
```

## What it does not cover

The first build needs a Rust toolchain and fetches the crates `Cargo.lock` pins; a client's startup timeout may need raising for it (`MCP_TIMEOUT` in Claude Code, `startup_timeout_sec` in Codex, `timeout` in Gemini CLI; the shipped configurations set the latter two) or `just build` run once beforehand. The launcher is for this repository's layout (`bin/` beside `apps/` and `share/`); a repository that only installs the tool names the built executable directly, as `apps/majordomus-cli/README.md` shows. Whether a client honours a project configuration is the client's: Claude Code asks for approval, Codex requires trust.

## Why it exists

The operator asked that starting a client should build, start and use the server automatically, for Claude, Codex and Gemini alike. The configurations are the clients' own formats; the launcher is the one place the build and the share directory are decided, so that the three stay identical. `test/cases/90_mcp_shared_server.sh` runs the launcher with two clients in a repository the shell tool's `init` wrote, checks the three configurations name it, checks that stdout carries protocol frames only, and checks the refusals when it may not build.
