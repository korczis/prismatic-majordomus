# 0001 Money is integer minor units

Floating point cannot represent 0.10 exactly, and invoices that do not add up to the cent
are refused by accountants. An amount is an `int` of the currency's minor unit (cents for
EUR, haléře for CZK) paired with an ISO 4217 code. Conversion to a decimal string happens
only when formatting for display (`currency.format_amount`).
