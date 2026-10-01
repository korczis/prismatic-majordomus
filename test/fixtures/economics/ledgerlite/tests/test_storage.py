import datetime
import json
import tempfile
import unittest
from pathlib import Path

from ledgerlite import storage
from ledgerlite.invoice import Invoice


class StorageTest(unittest.TestCase):
    def test_round_trip(self):
        inv = Invoice("2026-001", "ACME", "DE", datetime.date(2026, 1, 15))
        inv.add("consulting", 3, 10000)
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "inv.json"
            storage.save(p, [inv])
            (back,) = storage.load(p)
        self.assertEqual(back.number, "2026-001")
        self.assertEqual(back.gross(), inv.gross())

    def test_written_version(self):
        doc = json.loads(storage.dumps([]))
        self.assertEqual(doc["version"], storage.VERSION)

    def test_unknown_version_refused(self):
        with self.assertRaises(ValueError):
            storage.loads('{"version": 99, "invoices": []}')


if __name__ == "__main__":
    unittest.main()
