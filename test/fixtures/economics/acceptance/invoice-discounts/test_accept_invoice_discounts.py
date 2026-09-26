import datetime
import json
import unittest

from ledgerlite import report, storage
from ledgerlite.csv_export import export
from ledgerlite.invoice import Invoice, Line
from ledgerlite.money import Money


def inv_with_discount() -> Invoice:
    inv = Invoice("D-1", "ACME", "DE", datetime.date(2026, 4, 2))
    inv.add("licence", 3, 1000, discount_bp=1250)
    return inv


class AcceptDiscounts(unittest.TestCase):
    def test_line_default_is_no_discount(self):
        line = Line("x", 1, Money(1000))
        self.assertEqual(line.discount_bp, 0)
        self.assertEqual(line.net(), Money(1000))

    def test_discounted_net_and_vat(self):
        inv = inv_with_discount()
        self.assertEqual(inv.net(), Money(2625))
        self.assertEqual(inv.vat(), Money(499))  # 2625 * 19 % = 498.75 -> 499
        self.assertEqual(inv.gross(), Money(3124))

    def test_discount_rounds_half_even(self):
        inv = Invoice("D-2", "ACME", "DE", datetime.date(2026, 4, 2))
        inv.add("x", 3, 333, discount_bp=1250)  # 999 - 124.875 -> 999 - 125
        self.assertEqual(inv.net(), Money(874))

    def test_round_trip(self):
        (back,) = storage.loads(storage.dumps([inv_with_discount()]))
        self.assertEqual(back.lines[0].discount_bp, 1250)
        self.assertEqual(back.net(), Money(2625))

    def test_file_without_discount_loads(self):
        # A file written before discounts existed: the version-1 format as it is on disk
        # today, with lines that carry no discount field at all. A reader may version its
        # format for discounts (docs/CONVENTIONS.md #6), but it must still read this file.
        before = {
            "version": 1,
            "invoices": [
                {
                    "number": "D-1",
                    "customer": "ACME",
                    "country": "DE",
                    "issued": "2026-04-02",
                    "lines": [{"description": "licence", "quantity": 3, "unit_price": 1000}],
                }
            ],
        }
        (back,) = storage.loads(json.dumps(before))
        self.assertEqual(back.net(), Money(3000))

    def test_export_and_report_use_discounted_amounts(self):
        inv = inv_with_discount()
        self.assertIn("D-1,2026-04-02,ACME,EUR,26.25,4.99,31.24", export([inv]))
        (m,) = report.monthly_totals([inv])
        self.assertEqual(m.net, Money(2625))


if __name__ == "__main__":
    unittest.main()
