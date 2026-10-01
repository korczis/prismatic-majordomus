import unittest

from ledgerlite.money import CurrencyMismatch, Money, round_half_even, round_half_up


class MoneyTest(unittest.TestCase):
    def test_add_and_subtract(self):
        self.assertEqual(Money(150) + Money(250), Money(400))
        self.assertEqual(Money(150) - Money(250), Money(-100))

    def test_currency_mismatch(self):
        with self.assertRaises(CurrencyMismatch):
            Money(1, "EUR") + Money(1, "USD")

    def test_minor_must_be_int(self):
        with self.assertRaises(TypeError):
            Money(1.5)

    def test_total(self):
        self.assertEqual(Money.total([Money(1), Money(2), Money(3)]), Money(6))

    def test_round_half_even(self):
        self.assertEqual(round_half_even(5, 2), 2)
        self.assertEqual(round_half_even(15, 10), 2)
        self.assertEqual(round_half_even(25, 10), 2)
        self.assertEqual(round_half_even(26, 10), 3)

    def test_round_half_up(self):
        self.assertEqual(round_half_up(25, 10), 3)
        self.assertEqual(round_half_up(-25, 10), -3)


if __name__ == "__main__":
    unittest.main()
