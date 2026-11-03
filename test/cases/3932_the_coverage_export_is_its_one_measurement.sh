# majordomus-covers: none
# The coverage exports are the coverage measurement, not a second one.
#
# scripts/rust-coverage --lcov FILE and --cobertura FILE write the measurement in the formats
# Coveralls, Codecov and a graph over time read. The measurement was kept nowhere (the llvm-cov
# JSON is a fourteen-day CI artifact), and the obvious way to keep it, `cargo llvm-cov report
# --lcov`, counts the #[cfg(test)] modules and the files under tests/, and so publishes a
# number the gate never reports. This case holds the other way: the exports are written from
# the tallies the report prints, after the test code is excluded.
#
# The fixture is a hand-written llvm-cov export over a scratch crate: one source file with an
# executed function (line 2, count 7), an unexecuted one (line 5, count 0) and an inline
# `#[cfg(test)] mod tests` whose body is executed (line 11), and one file under tests/ that is
# executed throughout. The product code is 1 line of 2 and 1 function of 2; counting the test
# code would say 3 of 4 lines and 2 of 3 functions. It holds that
#   - the LCOV LF/LH sums, the Cobertura lines-valid/lines-covered and the report's own
#     summary (--summary-json) are one pair of numbers;
#   - no test line, and no file under tests/, is in either export;
#   - the LCOV is well formed (every SF record ends with end_of_record) and carries FN/FNDA
#     whose FNF/FNH are the report's function figures;
#   - the XML parses (xmllint when there is one, else python3);
#   - the exports also come from --report --export, and paths are relative to the root.
. "$ROOT/test/lib.sh"

mj_runs python3 || skip "$(mj_runs_why python3)"
command -v jq >/dev/null 2>&1 || skip "no jq"

W="$T/repo"
SRC="apps/majordomus-cli/src/sample.rs"
TST="apps/majordomus-cli/tests/sample_it.rs"
mkdir -p "$W/$(dirname "$SRC")" "$W/$(dirname "$TST")" "$W/scripts"
cp "$ROOT/scripts/rust-coverage" "$W/scripts/rust-coverage"; chmod +x "$W/scripts/rust-coverage"
cat > "$W/$SRC" <<'RS'
pub fn add(a: u32, b: u32) -> u32 {
    a + b
}
pub fn unused(x: u32) -> u32 {
    x * 2
}
#[cfg(test)]
mod tests {
    #[test]
    fn adds() {
        assert_eq!(super::add(1, 2), 3);
    }
}
RS
cat > "$W/$TST" <<'RS'
#[test]
fn it_runs() {
    assert_eq!(1 + 1, 2);
}
RS

# Segments are (line, col, count, has_count, is_region_entry, is_gap_region); a region is
# [line_start, col_start, line_end, col_end, count, file_id, expanded_file_id, kind].
EXPORT="$T/export.json"
W="$W" SRC="$SRC" TST="$TST" python3 - "$EXPORT" <<'PY'
import json, os, sys
src = os.path.join(os.environ["W"], os.environ["SRC"])
tst = os.path.join(os.environ["W"], os.environ["TST"])
def seg(line, count):
    return [[line, 5, count, True, True, False], [line, 20, 0, False, False, False]]
def fn(name, path, l1, l2, count):
    return {"name": name, "count": count, "filenames": [path],
            "regions": [[l1, 1, l2, 2, count, 0, 0, 0]]}
data = {"version": "2.0.1", "type": "llvm.coverage.json.export", "data": [{
    "files": [
        {"filename": src, "segments": seg(2, 7) + seg(5, 0) + seg(11, 1), "summary": {}},
        {"filename": tst, "segments": seg(3, 1), "summary": {}},
    ],
    "functions": [
        fn("_RNvadd", src, 1, 3, 7), fn("_RNvunused", src, 4, 6, 0),
        fn("_RNvtests5adds", src, 10, 12, 1), fn("_RNvit_runs", tst, 2, 4, 1),
    ],
    "totals": {"lines": {"percent": 100.0}, "functions": {"percent": 100.0},
               "regions": {"percent": 100.0}},
}]}
json.dump(data, open(sys.argv[1], "w"))
PY

# ---------------------------------------------------------------- 1. the report, with both exports
cd "$W" || exit 1
expect_exit 0 scripts/rust-coverage --report --from "$EXPORT" \
  --lcov "$T/a.info" --cobertura "$T/a.xml" --summary-json "$T/s.json"
expect_grep "the crate, test code excluded +50\\.00%"
expect_grep " 1/2 +lines"
expect_file "$T/a.info"
expect_file "$T/a.xml"

# the report's own figures, read from its summary rather than from a second parser
want_lc="$(jq -r '.crate.lines.covered' "$T/s.json")"
want_lv="$(jq -r '.crate.lines.total' "$T/s.json")"
want_fc="$(jq -r '.crate.functions.covered' "$T/s.json")"
want_fv="$(jq -r '.crate.functions.total' "$T/s.json")"
[ "$want_lc/$want_lv" = "1/2" ] || { echo "    the fixture's report is $want_lc/$want_lv lines, not 1/2"; exit 1; }
[ "$want_fc/$want_fv" = "1/2" ] || { echo "    the fixture's report is $want_fc/$want_fv functions, not 1/2"; exit 1; }

# ---------------------------------------------------------------- 2. LCOV is that measurement
got_lh="$(awk -F: '/^LH:/ { n += $2 } END { print n + 0 }' "$T/a.info")"
got_lf="$(awk -F: '/^LF:/ { n += $2 } END { print n + 0 }' "$T/a.info")"
got_fh="$(awk -F: '/^FNH:/ { n += $2 } END { print n + 0 }' "$T/a.info")"
got_ff="$(awk -F: '/^FNF:/ { n += $2 } END { print n + 0 }' "$T/a.info")"
[ "$got_lh/$got_lf" = "$want_lc/$want_lv" ] \
  || { echo "    LCOV LH/LF is $got_lh/$got_lf; the report says $want_lc/$want_lv"; exit 1; }
[ "$got_fh/$got_ff" = "$want_fc/$want_fv" ] \
  || { echo "    LCOV FNH/FNF is $got_fh/$got_ff; the report says $want_fc/$want_fv"; exit 1; }
# DA records are the counted lines themselves, not only their sum
expect_grep "^SF:$SRC\$" "$T/a.info"
expect_grep "^DA:2,7\$" "$T/a.info"
expect_grep "^DA:5,0\$" "$T/a.info"
expect_grep "^FNDA:7,_RNvadd\$" "$T/a.info"
expect_grep "^FNDA:0,_RNvunused\$" "$T/a.info"
# the test code is not in it: the executed line of the test module (11), the file under tests/,
# and the test function
expect_no_grep "^DA:11," "$T/a.info"
expect_no_grep "tests/" "$T/a.info"
expect_no_grep "sample_it|adds" "$T/a.info"
expect_no_grep "^SF:/" "$T/a.info"
# well formed: every SF record is closed by end_of_record, none is left open
sf="$(grep -c '^SF:' "$T/a.info")"
eor="$(grep -c '^end_of_record$' "$T/a.info")"
[ "$sf" = 1 ] && [ "$eor" = 1 ] || { echo "    LCOV has $sf SF record(s) and $eor end_of_record line(s)"; exit 1; }
[ "$(tail -n 1 "$T/a.info")" = end_of_record ] || { echo "    LCOV does not end with end_of_record"; exit 1; }

# ---------------------------------------------------------------- 3. Cobertura is that measurement
if command -v xmllint >/dev/null 2>&1; then
  xmllint --noout "$T/a.xml" || { echo "    xmllint refuses the Cobertura XML"; exit 1; }
else
  python3 -I -c 'import sys, xml.etree.ElementTree as E; E.parse(sys.argv[1])' "$T/a.xml" \
    || { echo "    the Cobertura XML does not parse"; exit 1; }
fi
xml_got="$(python3 -I - "$T/a.xml" <<'PY'
import sys, xml.etree.ElementTree as E
root = E.parse(sys.argv[1]).getroot()
lines = [l for l in root.iter("line")]
print(root.get("lines-covered"), root.get("lines-valid"), root.get("line-rate"),
      len(lines), sum(1 for l in lines if int(l.get("hits")) > 0),
      ",".join(sorted(c.get("filename") for c in root.iter("class"))))
PY
)"
[ "$xml_got" = "$want_lc $want_lv 0.5000 $want_lv $want_lc $SRC" ] \
  || { echo "    Cobertura says '$xml_got'; the report says $want_lc/$want_lv lines in $SRC"; exit 1; }

# ---------------------------------------------------------------- 4. the same files from --report --export
expect_exit 0 scripts/rust-coverage --report --from "$EXPORT" --export "$T/kept.json" \
  --lcov="$T/b.info" --cobertura="$T/b.xml"
expect_file "$T/kept.json"
grep -v '^TN:' "$T/a.info" > "$T/a.cmp"; grep -v '^TN:' "$T/b.info" > "$T/b.cmp"
cmp -s "$T/a.cmp" "$T/b.cmp" || { echo "    a second run wrote a different LCOV"; exit 1; }

# ---------------------------------------------------------------- 5. an export needs no instrumented run
# A failing option must not be swallowed: --lcov with nothing to write to is the script's error.
expect_exit 12 scripts/rust-coverage --report --from "$T/no-such-export.json" --lcov "$T/c.info"
expect_grep "the measurement could not be trusted"
[ ! -e "$T/c.info" ] || { echo "    an LCOV was written for a measurement that could not be trusted"; exit 1; }
