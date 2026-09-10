# Serving: the two user interfaces of the shared server this repository runs, and the
# assets one of them needs.
#
# The commands that *start* a server are not here. `mcp`, `serve` and `inspect` are
# commands of the Rust executable, and the generated bridge writes a recipe for each from
# the command graph — one description, one declaration, one place to change them. What
# remains here is what no program implements: opening a browser, and building the
# Cockpit's static assets.
#
# Neither recipe below opens the lease. `serve status` is the executable's one typed
# reading of that file (ADR 0035, project.the-lease-is-read-once); `mcp-status` was a `cat`
# of it and `open` a `sed` regular expression over JSON, which would have found nothing the
# day anything wrote the document pretty-printed, and said "no server" rather than "I
# cannot read this".

# Where the repository's shared server is, and every other server of this repository.
[group('serve')]
mcp-status:
    @bin/majordomus-cli serve status

# Open a surface of the running shared server in a browser: `just open`, `just open cockpit`, `just open swagger`. The address is the one `serve status` reports for this checkout; the surfaces come from the server.
[group('serve')]
open surface="":
    @url="$(bin/majordomus-cli serve status --format json | jq -r '[.servers[]? | select(.this_checkout == true)] | .[0].lease.url // empty')"; \
    [ -n "$url" ] || { echo "no shared server is running (just serve, or start an MCP client)"; exit 1; }; \
    url="$url/{{surface}}"; echo "$url"; \
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
