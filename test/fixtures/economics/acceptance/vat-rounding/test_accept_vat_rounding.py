import datetime
import unittest

from ledgerlite.invoice import Invoice
from ledgerlite.money import Money


class AcceptVatRounding(unittest.TestCase):
    def test_vat_is_the_sum_of_rounded_lines(self):
        # CZ 21 %: each line 50 -> 10.5 -> 10 (half to even); the total rounded once would be 21.
        inv = Invoice("1", "A", "CZ", datetime.date(2026, 3, 1))
        inv.add("a", 1, 50)
        inv.add("b", 1, 50)
        self.assertEqual(inv.vat(), Money(20))
        self.assertEqual(inv.gross(), Money(120))

    def test_half_even_per_line(self):
        # DE 19 %: 250 -> 47.5 -> 48 (even); 150 -> 28.5 -> 28 (even)
        inv = Invoice("2", "A", "DE", datetime.date(2026, 3, 1))
        inv.add("a", 1, 250)
        inv.add("b", 1, 150)
        self.assertEqual(inv.vat(), Money(76))

    def test_single_line_unchanged(self):
        inv = Invoice("3", "A", "DE", datetime.date(2026, 3, 1))
        inv.add("x", 3, 10000)
        self.assertEqual(inv.vat(), Money(5700))


if __name__ == "__main__":
    unittest.main()
