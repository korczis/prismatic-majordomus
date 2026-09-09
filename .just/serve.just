# Serving: the two user interfaces of the shared server this repository runs, and the
# assets one of them needs.
#
# The commands that *start* a server are not here. `mcp`, `serve` and `inspect` are
# commands of the Rust executable, and the generated bridge writes a recipe for each from
# the command graph — one description, one declaration, one place to change them. What
# remains here is what no program implements: opening a browser, and building the
# Cockpit's static assets.

# Where the repository's shared server is, if one runs (from the lease under .ai/local/).
[group('serve')]
mcp-status:
    @f=".ai/local/state/mcp/server.json"; if [ -f "$f" ]; then cat "$f"; echo; else echo "no shared server lease at $f"; fi

# Open a surface of the running shared server in a browser: `just open`, `just open cockpit`, `just open swagger`. The address comes from the lease; the surfaces come from the server.
[group('serve')]
open surface="":
    @f=".ai/local/state/mcp/server.json"; [ -f "$f" ] || { echo "no shared server is running (just serve, or start an MCP client)"; exit 1; }; \
    url="$(sed -n 's/.*"url":"\([^"]*\)".*/\1/p' "$f")/{{surface}}"; echo "$url"; \
    (command -v open >/dev/null && open "$url") || xdg-open "$url"

# Open Swagger UI of the running shared server in the browser.
[group('serve')]
swagger-ui: (open "swagger")

# Open the running shared server's home page — every surface it serves, derived from the topology.
[group('serve')]
home: (open "")

# Open the Cockpit of the running shared server in the browser.
[group('serve')]
cockpit: (open "cockpit")

# Build the Cockpit's static assets: compile share/cockpit/cockpit.css, vendor the pinned libraries (needs npm ci).
[group('serve')]
cockpit-assets:
    scripts/cockpit-assets

# The committed Cockpit stylesheet matches its source; exit 10 when it is stale.
[group('serve')]
cockpit-assets-check:
    scripts/cockpit-assets --check

# Every Cockpit route the server serves, answered; then the pages and the interactions in a real browser.
[group('serve')]
cockpit-probe *args:
    scripts/cockpit-probe {{args}}
