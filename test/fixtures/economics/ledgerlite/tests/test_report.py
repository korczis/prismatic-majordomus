import datetime
import unittest

from ledgerlite import report
from ledgerlite.invoice import Invoice
from ledgerlite.money import Money


class ReportTest(unittest.TestCase):
    def test_monthly_totals(self):
        a = Invoice("1", "A", "DE", datetime.date(2026, 1, 5))
        a.add("x", 1, 10000)
        b = Invoice("2", "B", "DE", datetime.date(2026, 1, 20))
        b.add("y", 2, 5000)
        c = Invoice("3", "C", "DE", datetime.date(2026, 2, 1))
        c.add("z", 1, 1000)
        months = report.monthly_totals([c, a, b])
        self.assertEqual([m.month for m in months], ["2026-01", "2026-02"])
        self.assertEqual(months[0].invoices, 2)
        self.assertEqual(months[0].net, Money(20000))


if __name__ == "__main__":
    unittest.main()
