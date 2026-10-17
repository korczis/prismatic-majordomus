# majordomus-covers: none
# A leak finding names the file it is in and the kind it is, and never the text (I1023).
#
# scripts/site-check scans the built site for the shapes the repository's one definition,
# apps/majordomus-cli/src/quality/leak-patterns.tsv, says a published file must never carry:
# a developer's home directory, an internal host name, a credential-shaped assignment. Until
# this case it reported "local path in output" and stopped there: the person publishing knew
# the site was refused and not where. A finding has to say where, and it must not say what —
# the text is the secret, and a check that printed it would copy it into every CI log.
#
# This drives check 12 as the script carries it — lifted out of the file, not copied — with
# site-check's own `ok`, `bad` and counter, against fixture sites with synthetic markers
# planted in them. The markers are assembled at run time, so this file holds none of them.
. "$ROOT/test/lib.sh"

SC="$ROOT/scripts/site-check"
awk '/^LEAKS=/ { on = 1 } /^# 13\. / { exit } on { print }' "$SC" > "$T/leaks.sh"
grep -q '^leak_files() {$' "$T/leaks.sh" \
  || { echo "    check 12 of scripts/site-check defines no leak_files(); the scan cannot name a file"; exit 1; }
[ "$(grep -c 'leak_files "\$' "$T/leaks.sh")" -ge 2 ] \
  || { echo "    check 12 does not scan both classes through leak_files"; exit 1; }

# the caller's half, as site-check declares it
fails=0
ok()  { printf 'OK   %-12s %s\n' "$1" "$2"; }
bad() { printf 'FAIL %-12s %s\n' "$1" "$2"; fails=$((fails+1)); }
site_files() { find "$PUB" -type f -print0; }
scan() { fails=0; PUB="$T/$1"; . "$T/leaks.sh" > "$T/out.txt" 2>&1; }

# The synthetic markers: a home directory, an internal host, a credential value.
home="/Users"; home="$home/someone/dev/project"
host="build.example"; host="$host.internal"
value="Zq8v3XkP"; value="${value}2mN7rT1wY6uB4cE9"

# ---------------------------------------------------------------- 1. a clean site is clean
mkdir -p "$T/clean/docs"
printf '<html><body><p>the token is passed in the environment</p></body></html>\n' > "$T/clean/docs/index.html"
printf 'token: SOME_CONSTANT_NAME\npermissions: id-token: write\nsecret: ${{ secrets.GITHUB_TOKEN }}\n' > "$T/clean/docs/workflow.txt"
scan clean
[ "$fails" = 0 ] || { echo "    a clean site recorded $fails failure(s):"; sed 's/^/    | /' "$T/out.txt"; exit 1; }
grep -q '^OK   private' "$T/out.txt" && grep -q '^OK   secrets' "$T/out.txt" \
  || { echo "    a clean site did not report both scans clean:"; sed 's/^/    | /' "$T/out.txt"; exit 1; }

# ---------------------------------------------------------------- 2. each finding says where and what kind
mkdir -p "$T/leaky/guide" "$T/leaky/data" "$T/leaky/ops"
cp "$T/clean/docs/index.html" "$T/leaky/guide/clean.html"
printf '<html><body><pre>%s/src/main.rs</pre></body></html>\n' "$home" > "$T/leaky/guide/index.html"
printf 'see https://%s/status\n' "$host" > "$T/leaky/ops/hosts.txt"
# The assignment shape the definition names. A JSON key (`"api_key": "…"`) is not matched by
# it today — the closing quote stands between the name and the colon — and is left to its own
# change of the definition, which the crate's rustdoc scan compiles in as well.
printf 'api_key = "%s"\n' "$value" > "$T/leaky/data/config.json"
scan leaky
[ "$fails" = 3 ] || { echo "    three planted leaks recorded $fails failure(s), not 3:"; sed 's/^/    | /' "$T/out.txt"; exit 1; }
grep -q '^FAIL private .*guide/index.html: a local path' "$T/out.txt" \
  || { echo "    the home directory finding does not name guide/index.html:"; sed 's/^/    | /' "$T/out.txt"; exit 1; }
grep -q '^FAIL private .*ops/hosts.txt: ' "$T/out.txt" \
  || { echo "    the internal host finding does not name ops/hosts.txt:"; sed 's/^/    | /' "$T/out.txt"; exit 1; }
grep -q '^FAIL secrets .*data/config.json: a credential' "$T/out.txt" \
  || { echo "    the credential finding does not name data/config.json:"; sed 's/^/    | /' "$T/out.txt"; exit 1; }
grep -q 'clean.html' "$T/out.txt" && { echo "    a file with no leak was named:"; sed 's/^/    | /' "$T/out.txt"; exit 1; }

# ---------------------------------------------------------------- 3. never the text
for marker in "$value" "$home" "$host" someone; do
  grep -qF -- "$marker" "$T/out.txt" \
    && { echo "    the findings print what they found ($marker is in the output)"; exit 1; }
done

# ---------------------------------------------------------------- 4. the definition is the one the gate reads
LEAKS_FILE="$ROOT/apps/majordomus-cli/src/quality/leak-patterns.tsv"
grep -q 'leak-patterns.tsv' "$T/leaks.sh" \
  || { echo "    check 12 no longer reads $LEAKS_FILE"; exit 1; }
echo "    ok: every planted leak is named by file and kind, nothing clean is named, no marker is printed"
