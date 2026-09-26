import unittest

from ledgerlite import tax


class TaxTest(unittest.TestCase):
    def test_rate(self):
        self.assertEqual(tax.rate_for("CZ"), 2100)

    def test_unknown_country(self):
        with self.assertRaises(ValueError):
            tax.rate_for("XX")

    def test_line_tax_rounds_half_even(self):
        # 250 * 20 % = 50.0 exactly; 1025 * 20 % = 205.0; 1234 * 21 % = 259.14 -> 259
        self.assertEqual(tax.line_tax(250, 2000), 50)
        self.assertEqual(tax.line_tax(1234, 2100), 259)
        # 50 * 21 % = 10.5 -> 10 (ties to even)
        self.assertEqual(tax.line_tax(50, 2100), 10)


if __name__ == "__main__":
    unittest.main()
