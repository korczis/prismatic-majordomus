import contextlib
import datetime
import io
import tempfile
import unittest
from pathlib import Path

from ledgerlite import cli, storage
from ledgerlite.invoice import Invoice
from ledgerlite.recurring import Schedule, generate

D = datetime.date


class AcceptSchedule(unittest.TestCase):
    def test_monthly_clamps_to_month_end_from_start_day(self):
        s = Schedule(D(2026, 1, 31), "monthly", 4)
        self.assertEqual(s.dates(), [D(2026, 1, 31), D(2026, 2, 28), D(2026, 3, 31), D(2026, 4, 30)])

    def test_quarterly_crosses_year(self):
        s = Schedule(D(2026, 11, 15), "quarterly", 3)
        self.assertEqual(s.dates(), [D(2026, 11, 15), D(2027, 2, 15), D(2027, 5, 15)])

    def test_invalid(self):
        with self.assertRaises(ValueError):
            Schedule(D(2026, 1, 1), "weekly", 2).dates()
        with self.assertRaises(ValueError):
            Schedule(D(2026, 1, 1), "monthly", 0).dates()


class AcceptGenerate(unittest.TestCase):
    def template(self):
        t = Invoice("T-1", "ACME", "CZ", D(2026, 1, 31))
        t.add("hosting", 1, 5000)
        return t

    def test_generate(self):
        t = self.template()
        out = generate(t, Schedule(D(2026, 1, 31), "monthly", 3), 7)
        self.assertEqual([i.number for i in out], ["R-7", "R-8", "R-9"])
        self.assertEqual([i.issued for i in out], [D(2026, 1, 31), D(2026, 2, 28), D(2026, 3, 31)])
        self.assertEqual(out[1].customer, "ACME")
        self.assertEqual(out[1].country, "CZ")
        self.assertEqual(out[1].net(), t.net())
        self.assertEqual(t.number, "T-1")
        out[0].add("extra", 1, 1)
        self.assertEqual(len(t.lines), 1, "the template is not modified")

    def test_cli(self):
        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "inv.json"
            storage.save(f, [self.template()])
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                code = cli.main(
                    ["recur", str(f), "--template", "T-1", "--every", "quarterly", "--count", "2", "--first", "1"]
                )
            self.assertEqual(code or 0, 0)
            self.assertEqual(buf.getvalue().split("\n")[:2], ["R-1 2026-01-31", "R-2 2026-04-30"])
            numbers = [i.number for i in storage.load(f)]
        self.assertEqual(numbers, ["T-1", "R-1", "R-2"])


if __name__ == "__main__":
    unittest.main()
