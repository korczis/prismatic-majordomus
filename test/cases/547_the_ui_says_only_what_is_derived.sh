# majordomus-covers: none
# The ui-integrity gate of ADR 0089 (project.ui-derived-state), driven against fixture
# trees rather than against this checkout: each of its four checks is shown a tree that
# violates it and refuses it by check, file and line, and a clean tree passes. A case that
# asserted only this repository's own sources would pass the day a check stopped firing,
# because this repository's findings sit in the baseline; what is asserted here is what the
# gate refuses, what it deliberately does not refuse, and how its ratchet behaves.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/cockpit-projection-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }
SRC=apps/majordomus-cli/src

# A tree with every subject the gate reads, and nothing it refuses: a navigation that reads
# the registry for its module listing (the declared allow-list), a page that asks, a health
# check whose status is decided plus one ok literal that says why it is right, a default
# that is not a healthy word, and counts rendered as unknown when absent.
fixture() {
  local f="$T/tree$1"
  rm -rf "$f"
  mkdir -p "$f/$SRC/cockpit" "$f/$SRC/capability/builtin" "$f/$SRC/models"
  cat > "$f/$SRC/cockpit/nav.rs" <<'RS'
pub fn build(ctx: &Context) -> Navigation {
    let summary = ctx.registry.summary();
    let modules = ctx
        .registry
        .modules()
        .map(|m| ctx.product.module_area(m.id.as_str()).and_then(|id| ctx.why.area(id)));
    Navigation::of(summary, modules)
}
RS
  cat > "$f/$SRC/cockpit/pages.rs" <<'RS'
//! A page asks; it never reads ctx.registry itself.
pub fn capabilities(ctx: &Context) -> Page {
    let list: CapabilityList = ask(ctx, "capabilities.list", json!({})).unwrap();
    Page::new(list.total)
}

fn upstream_cell(u: &Upstream) -> String {
    match (u.ahead, u.behind) {
        (Some(a), Some(b)) => format!("+{a} -{b}"),
        _ => "?".into(),
    }
}

fn label(v: &Value) -> String {
    v.get("title").and_then(Value::as_str).unwrap_or_default().to_string()
}
RS
  cat > "$f/$SRC/capability/builtin/health.rs" <<'RS'
fn health(ctx: &Context) -> Health {
    checks.push(HealthCheck {
        id: "index".into(),
        status: if broken { HealthStatus::Fail } else { HealthStatus::Ok },
    });
    checks.push(HealthCheck {
        id: "registry".into(),
        // ui-integrity: a registry that does not build stops the process before this runs
        status: HealthStatus::Ok,
    });
}
RS
  cat > "$f/$SRC/models/mod.rs" <<'RS'
pub enum Inference {
    /// Served by a vendor.
    #[default]
    Remote,
    Local,
}
RS
  printf '%s' "$f"
}

# --- a clean tree passes, and the gate says what it measured
F="$(fixture 0)"
expect_exit 0 env MJ_ROOT="$F" "$GATE" --ui-integrity
expect_grep '^ok   ui-integrity'
expect_grep 'ADR 0089'
# the allow-listed navigation reads, the comment in pages.rs, the decided status, the
# exempted literal, the non-healthy default and the string default are none of them findings
expect_no_grep '^FAIL'

# --- direct-read: a page that reads the registry itself instead of asking
F="$(fixture 1)"
cat >> "$F/$SRC/cockpit/pages.rs" <<'RS'

pub fn topology(ctx: &Context) -> Page {
    Page::new(ctx.registry.summary())
}
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE" --ui-integrity
expect_grep "^FAIL $SRC/cockpit/pages.rs:19  direct-read"
expect_grep 'ctx.registry is read directly in fn topology'
expect_grep 'Context::execute'

# the same read split across lines, and a read of the product model and of the Why
# catalogue, are each refused: the allow-list is the navigation's and nobody else's
F="$(fixture 2)"
cat >> "$F/$SRC/cockpit/pages.rs" <<'RS'

pub fn modules(ctx: &Context) -> Page {
    let n = ctx
        .registry
        .len();
    Page::new(ctx.product.module_area("x"), ctx.why.areas(), n)
}
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE" --ui-integrity
expect_grep "pages.rs:20  direct-read: ctx.registry"
expect_grep "pages.rs:22  direct-read: ctx.product"
expect_grep "pages.rs:22  direct-read: ctx.why"

# --- unconditional-ok: a health check that is ok by literal and says nothing about why
F="$(fixture 3)"
cat >> "$F/$SRC/capability/builtin/health.rs" <<'RS'
fn peers(ctx: &Context) -> HealthCheck {
    HealthCheck {
        id: "peers".into(),
        status: HealthStatus::Ok,
    }
}
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE" --ui-integrity
expect_grep "^FAIL $SRC/capability/builtin/health.rs:15  unconditional-ok"
expect_grep 'the check peers is given status HealthStatus::Ok as a literal'
expect_no_grep 'health.rs:9 '

# an exemption with no reason is no exemption
F="$(fixture 4)"
cat >> "$F/$SRC/capability/builtin/health.rs" <<'RS'
fn scope() -> HealthCheck {
    HealthCheck {
        id: "scope".into(),
        // ui-integrity:
        status: HealthStatus::Ok,
    }
}
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE" --ui-integrity
expect_grep "health.rs:16  unconditional-ok: the check scope"

# --- healthy-default: an enum whose default is a healthy variant, doc comment in between
F="$(fixture 5)"
cat >> "$F/$SRC/models/mod.rs" <<'RS'
pub enum ModelStatus {
    /// Generally available.
    #[default]
    /// Declared by the catalogue.
    Available,
    Preview,
}
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE" --ui-integrity
expect_grep "^FAIL $SRC/models/mod.rs:9  healthy-default"
expect_grep 'ModelStatus::Available the value nobody declared'

# --- count-default: an absent count rendered as zero, both spellings
F="$(fixture 6)"
cat >> "$F/$SRC/cockpit/pages.rs" <<'RS'

fn ahead(u: &Upstream) -> String {
    format!("+{}", u.ahead.unwrap_or(0))
}

fn phase(v: &Value) -> u64 {
    v.get("count")
        .and_then(Value::as_u64)
        .unwrap_or_default()
}
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE" --ui-integrity
expect_grep "^FAIL $SRC/cockpit/pages.rs:19  count-default: an absent count renders as zero in fn ahead"
expect_grep "pages.rs:25  count-default: an absent count renders as zero in fn phase"

# --- the ratchet: a finding the baseline holds is debt and passes; a new one still fails;
# a baseline line nothing matches is paid and does not fail; --strict ignores the baseline
F="$(fixture 7)"
cat >> "$F/$SRC/cockpit/pages.rs" <<'RS'

fn ahead(u: &Upstream) -> String {
    format!("+{}", u.ahead.unwrap_or(0))
}
RS
expect_exit 0 env MJ_ROOT="$F" "$GATE" --ui-integrity --write-baseline
expect_file "$F/.ai/repo/ui-integrity-baseline.txt"
expect_grep '^count-default apps/majordomus-cli/src/cockpit/pages.rs ahead format' "$F/.ai/repo/ui-integrity-baseline.txt"
expect_no_grep ':[0-9]+' "$F/.ai/repo/ui-integrity-baseline.txt"
expect_exit 0 env MJ_ROOT="$F" "$GATE" --ui-integrity
expect_grep "^debt $SRC/cockpit/pages.rs:19  count-default"
expect_exit 10 env MJ_ROOT="$F" "$GATE" --ui-integrity --strict
expect_grep "^FAIL $SRC/cockpit/pages.rs:19  count-default"
# an edit above the finding moves its line and not its key
printf '// moved\n' | cat - "$F/$SRC/cockpit/pages.rs" > "$F/p" && mv "$F/p" "$F/$SRC/cockpit/pages.rs"
expect_exit 0 env MJ_ROOT="$F" "$GATE" --ui-integrity
expect_grep "^debt $SRC/cockpit/pages.rs:20  count-default"
cat >> "$F/$SRC/cockpit/pages.rs" <<'RS'

fn behind(u: &Upstream) -> String {
    format!("-{}", u.behind.unwrap_or(0))
}
RS
expect_exit 10 env MJ_ROOT="$F" "$GATE" --ui-integrity
expect_grep "count-default: an absent count renders as zero in fn behind"
expect_grep '1 finding\(s\) the baseline does not hold \(1 known'
F2="$(fixture 8)"
mkdir -p "$F2/.ai/repo"
cp "$F/.ai/repo/ui-integrity-baseline.txt" "$F2/.ai/repo/"
expect_exit 0 env MJ_ROOT="$F2" "$GATE" --ui-integrity
expect_grep '^paid count-default'

# --- a tree without a subject is unusable, not clean
F="$(fixture 9)"
rm "$F/$SRC/capability/builtin/health.rs"
expect_exit 12 env MJ_ROOT="$F" "$GATE" --ui-integrity
expect_grep 'cannot read apps/majordomus-cli/src/capability/builtin/health.rs'
expect_exit 2 env MJ_ROOT="$F" "$GATE" --strict

# --- the index check of ADR 0012 is untouched by the widening: without --ui-integrity the
# gate still asks only whether a page reads the index
F="$(fixture 10)"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'ok   cockpit-projection'

# --- and this repository passes, which is the state the gate defends
expect_exit 0 "$GATE" --ui-integrity
expect_grep '^ok   ui-integrity'
