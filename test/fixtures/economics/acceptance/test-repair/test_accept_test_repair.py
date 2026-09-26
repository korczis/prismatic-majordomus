import datetime
import unittest
import warnings

from ledgerlite import report
from ledgerlite.invoice import Invoice


def invoices():
    a = Invoice("1", "A", "DE", datetime.date(2026, 1, 5))
    a.add("x", 1, 10000)
    return [a]


class AcceptRename(unittest.TestCase):
    def test_new_name_works(self):
        (m,) = report.monthly_summary(invoices())
        self.assertEqual(m.month, "2026-01")

    def test_old_name_is_a_deprecated_alias(self):
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            (m,) = report.monthly_totals(invoices())
        self.assertEqual(m.month, "2026-01")
        dep = [w for w in caught if issubclass(w.category, DeprecationWarning)]
        self.assertTrue(dep, "the old name must emit DeprecationWarning")
        self.assertIn("monthly_summary", str(dep[0].message))


if __name__ == "__main__":
    unittest.main()
