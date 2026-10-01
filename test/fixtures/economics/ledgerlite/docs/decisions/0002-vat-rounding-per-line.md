# 0002 VAT is rounded per line, half to even

The tax authorities we file with require VAT to be computed for each invoice line and
rounded to the minor unit on that line; the invoice's VAT is the sum of the rounded line
amounts. Rounding is half to even ("banker's rounding"): 0.5 minor units rounds to the
nearest even unit. Rounding once on the invoice total produces a different figure on
multi-line invoices and was the cause of rejected filings in the past.

`tax.line_tax(net_minor, rate_bp)` is the one place this rounding happens.
