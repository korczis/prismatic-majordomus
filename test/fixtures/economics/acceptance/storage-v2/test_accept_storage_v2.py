import datetime
import json
import unittest

from ledgerlite import storage
from ledgerlite.invoice import Invoice
from ledgerlite.money import Money

V1 = {
    "version": 1,
    "invoices": [
        {
            "number": "old-1",
            "customer": "ACME",
            "country": "DE",
            "issued": "2025-12-01",
            "lines": [{"description": "x", "quantity": 2, "unit_price": 500}],
        }
    ],
}


class AcceptStorageV2(unittest.TestCase):
    def test_writes_version_two(self):
        self.assertEqual(storage.VERSION, 2)
        self.assertEqual(json.loads(storage.dumps([]))["version"], 2)

    def test_currency_survives_round_trip(self):
        inv = Invoice("u-1", "ACME", "DE", datetime.date(2026, 5, 1), currency="USD")
        inv.add("x", 1, 1234)
        (back,) = storage.loads(storage.dumps([inv]))
        self.assertEqual(back.currency, "USD")
        self.assertEqual(back.gross(), inv.gross())
        self.assertEqual(back.lines[0].unit_price, Money(1234, "USD"))

    def test_version_one_is_migrated_as_eur(self):
        (back,) = storage.loads(json.dumps(V1))
        self.assertEqual(back.currency, "EUR")
        self.assertEqual(back.net(), Money(1000, "EUR"))

    def test_unknown_version_refused(self):
        with self.assertRaises(ValueError):
            storage.loads('{"version": 99, "invoices": []}')


if __name__ == "__main__":
    unittest.main()
