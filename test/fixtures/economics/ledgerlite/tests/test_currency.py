import unittest

from ledgerlite.currency import format_amount
from ledgerlite.money import Money


class FormatTest(unittest.TestCase):
    def test_eur(self):
        self.assertEqual(format_amount(Money(123450, "EUR")), "1.234,50 €")

    def test_usd(self):
        self.assertEqual(format_amount(Money(123450, "USD")), "$1,234.50")

    def test_jpy_has_no_minor_digits(self):
        self.assertEqual(format_amount(Money(1235, "JPY")), "¥1,235")

    def test_negative(self):
        self.assertEqual(format_amount(Money(-5, "EUR")), "-0,05 €")

    def test_unknown(self):
        with self.assertRaises(ValueError):
            format_amount(Money(1, "XXX"))


if __name__ == "__main__":
    unittest.main()
