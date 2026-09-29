import contextlib
import io
import tempfile
import unittest
from pathlib import Path

from ledgerlite import cli


def run(*argv) -> str:
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        code = cli.main(list(argv))
    assert code == 0, code
    return out.getvalue()


class CliTest(unittest.TestCase):
    def test_create_add_total(self):
        with tempfile.TemporaryDirectory() as d:
            f = str(Path(d) / "inv.json")
            run("create", f, "2026-001", "--customer", "ACME", "--country", "DE", "--issued", "2026-01-15")
            run("add-line", f, "2026-001", "consulting", "3", "10000")
            out = run("total", f, "2026-001")
        self.assertIn("gross 357,00 €", out)

    def test_export(self):
        with tempfile.TemporaryDirectory() as d:
            f = str(Path(d) / "inv.json")
            run("create", f, "2026-001", "--customer", "ACME", "--country", "DE", "--issued", "2026-01-15")
            run("add-line", f, "2026-001", "consulting", "3", "10000")
            out = run("export", f)
        self.assertIn("2026-001,2026-01-15,ACME,EUR,300.00,57.00,357.00", out)


if __name__ == "__main__":
    unittest.main()
