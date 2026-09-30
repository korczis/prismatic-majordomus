Invoice lines can now carry a discount. Add an optional discount to `Line` as a field
`discount_bp` (default 0, in basis points) and accept it in `Invoice.add(...,
discount_bp=0)`. A line's net amount is the undiscounted amount minus the discount, the
discount rounded half to even to the minor unit; VAT is computed on the discounted net. The
discount must survive saving and loading (files written before this change must still
load), and the CSV export and the monthly report must show discounted amounts. Add tests.
