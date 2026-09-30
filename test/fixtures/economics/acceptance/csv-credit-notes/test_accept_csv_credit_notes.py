import datetime
import unittest

from ledgerlite.csv_export import decimal, export
from ledgerlite.invoice import Invoice


class AcceptCreditNotes(unittest.TestCase):
    def test_negative_decimal(self):
        self.assertEqual(decimal(-1050, "EUR"), "-10.50")
        self.assertEqual(decimal(-5, "EUR"), "-0.05")
        self.assertEqual(decimal(-100, "EUR"), "-1.00")
        self.assertEqual(decimal(-5, "JPY"), "-5")

    def test_positive_unchanged(self):
        self.assertEqual(decimal(1050, "EUR"), "10.50")
        self.assertEqual(decimal(0, "EUR"), "0.00")

    def test_credit_note_row(self):
        inv = Invoice("CN-1", "ACME", "DE", datetime.date(2026, 6, 1))
        inv.add("refund", 1, -1050)
        self.assertIn("CN-1,2026-06-01,ACME,EUR,-10.50,", export([inv]))


if __name__ == "__main__":
    unittest.main()
