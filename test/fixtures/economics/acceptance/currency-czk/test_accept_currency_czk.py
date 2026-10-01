import unittest

from ledgerlite.csv_export import decimal
from ledgerlite.currency import format_amount
from ledgerlite.money import Money


class AcceptCzk(unittest.TestCase):
    def test_format(self):
        self.assertEqual(format_amount(Money(123450, "CZK")), "1 234,50 Kč")

    def test_small_and_negative(self):
        self.assertEqual(format_amount(Money(-5, "CZK")), "-0,05 Kč")
        self.assertEqual(format_amount(Money(123456789, "CZK")), "1 234 567,89 Kč")

    def test_export_decimal(self):
        self.assertEqual(decimal(123450, "CZK"), "1234.50")

    def test_others_unchanged(self):
        self.assertEqual(format_amount(Money(123450, "EUR")), "1.234,50 €")


if __name__ == "__main__":
    unittest.main()
