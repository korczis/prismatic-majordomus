import datetime
import unittest

from ledgerlite.invoice import Invoice
from ledgerlite.money import Money


def sample() -> Invoice:
    inv = Invoice("2026-001", "ACME", "DE", datetime.date(2026, 1, 15))
    inv.add("consulting", 3, 10000)
    return inv


class InvoiceTest(unittest.TestCase):
    def test_single_line_totals(self):
        inv = sample()
        self.assertEqual(inv.net(), Money(30000))
        self.assertEqual(inv.vat(), Money(5700))
        self.assertEqual(inv.gross(), Money(35700))

    def test_empty_invoice(self):
        inv = Invoice("x", "c", "DE", datetime.date(2026, 1, 1))
        self.assertEqual(inv.gross(), Money(0))


if __name__ == "__main__":
    unittest.main()
