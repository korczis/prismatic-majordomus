//! Canonical presentation order.
//!
//! One collection has one order, and every interface that shows it shows that order. A
//! sort written beside a renderer is a second opinion about the same collection, and two
//! opinions drift: the command line alphabetises, the Cockpit groups, the generated index
//! keeps whatever the walk produced, and a reader who compares them concludes that the
//! surfaces disagree about the repository rather than about sorting.
//!
//! The order is a total order over four parts, most significant first:
//!
//! 1. **group** — the semantic bucket the item is filed under, itself compared naturally;
//!    an item with no group sorts after every grouped item, because an ungrouped tail is
//!    legible and an ungrouped head hides the groups.
//! 2. **rank** — an explicit position inside the group, ascending, for the collections
//!    whose domain genuinely has one (a lifecycle runs setup before conclude, whatever
//!    those words sort as). It defaults to [`OrderKey::UNRANKED`], so a collection that
//!    has no such semantics never mentions it.
//! 3. **label** — what a person reads, compared naturally: digit runs by value, so
//!    `item-2` precedes `item-10`, and letters case-insensitively, so `Alpha` and `alpha`
//!    stay adjacent.
//! 4. **identity** — the canonical id, compared naturally, as the final tie-breaker.
//!
//! The fourth part is what makes the order total rather than merely tidy. Two items with
//! the same label must not exchange places because an unrelated item was added, because a
//! different iterator was used, or because one machine's filesystem enumerates differently
//! from another's; identities are unique, so the comparison always ends there.
//!
//! ```
//! use majordomus_cli::order::{canonical, OrderKey, Ordered};
//!
//! struct Recipe {
//!     group: &'static str,
//!     name: &'static str,
//! }
//!
//! impl Ordered for Recipe {
//!     fn order_key(&self) -> OrderKey<'_> {
//!         OrderKey::grouped(self.group, self.name, self.name)
//!     }
//! }
//!
//! let mut recipes = vec![
//!     Recipe { group: "quality", name: "test-10" },
//!     Recipe { group: "development", name: "serve" },
//!     Recipe { group: "quality", name: "test-2" },
//! ];
//! canonical(&mut recipes);
//! let order: Vec<_> = recipes.iter().map(|r| r.name).collect();
//! assert_eq!(order, ["serve", "test-2", "test-10"]);
//! ```

use std::cmp::Ordering;

/// What a collection's items answer so that the collection can be put in canonical order.
///
/// Implemented on the domain type, once, next to the type — not on a view of it beside a
/// renderer. Every projection then sorts by calling [`canonical`], and none of them holds
/// an opinion of its own about the sequence.
pub trait Ordered {
    /// The four parts this item is ordered by. Borrowed from the item: building a key
    /// allocates nothing, so sorting a large collection costs the comparisons and no more.
    fn order_key(&self) -> OrderKey<'_>;
}

/// A reference orders as the item it points at, so a projection can put a borrowed view of
/// a collection in canonical order without cloning every item into a vector first.
impl<T: Ordered + ?Sized> Ordered for &T {
    fn order_key(&self) -> OrderKey<'_> {
        (**self).order_key()
    }
}

/// The four parts of the canonical order, most significant first. See the module
/// documentation for what each part means and why the last one is not optional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderKey<'a> {
    /// The semantic bucket, or `None` for an item that is not filed under one. `None`
    /// sorts after every named group.
    pub group: Option<&'a str>,
    /// The explicit position inside the group, ascending. [`OrderKey::UNRANKED`] when the
    /// collection has no such semantics, which is most of them.
    pub rank: i64,
    /// What a person reads.
    pub label: &'a str,
    /// The canonical id: unique, and therefore the tie-breaker that makes the order total.
    pub identity: &'a str,
}

impl<'a> OrderKey<'a> {
    /// The rank of an item whose collection has no explicit ordering semantics. Zero, so
    /// that a collection where only some items are ranked puts those first, which is what
    /// an explicit rank is for.
    pub const UNRANKED: i64 = 0;

    /// An item with no group and no explicit rank: ordered by its label, then its identity.
    pub fn plain(label: &'a str, identity: &'a str) -> Self {
        Self {
            group: None,
            rank: Self::UNRANKED,
            label,
            identity,
        }
    }

    /// An item filed under a group, ordered inside it by its label, then its identity.
    pub fn grouped(group: &'a str, label: &'a str, identity: &'a str) -> Self {
        Self {
            group: Some(group),
            rank: Self::UNRANKED,
            label,
            identity,
        }
    }

    /// The same key with an explicit position inside its group.
    pub fn ranked(self, rank: i64) -> Self {
        Self { rank, ..self }
    }
}

impl Ord for OrderKey<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        group_cmp(self.group, other.group)
            .then_with(|| self.rank.cmp(&other.rank))
            .then_with(|| natural_cmp(self.label, other.label))
            .then_with(|| natural_cmp(self.identity, other.identity))
    }
}

impl PartialOrd for OrderKey<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Put a collection in canonical order, in place.
///
/// The sort is stable, but nothing relies on that: the identity part of the key makes the
/// order total, so the result does not depend on the order the items arrived in. That is
/// the property [`is_canonical`] checks and the permutation tests below prove.
pub fn canonical<T: Ordered>(items: &mut [T]) {
    items.sort_by(|a, b| a.order_key().cmp(&b.order_key()));
}

/// Put a collection of plain strings in canonical order, in place.
///
/// A `Vec<String>` has no group, no rank and no identity apart from the string itself, so it
/// cannot implement [`Ordered`] without a newtype nobody wants. It still has an order, and
/// before this existed every caller picked one of two: `.sort()`, which is `Ord for str` and
/// puts `case-107` before `case-9`, or a hand-written `sort_by(|a, b| natural_cmp(a, b))`,
/// which is this function spelled out again at the call site. The second is the duplication
/// `project.canonical-order` exists to prevent, and the first is a different order from the
/// one every other collection in the crate is shown in.
///
/// ```
/// use majordomus_cli::order::canonical_strings;
///
/// let mut cases = vec!["case-107".to_string(), "case-9".to_string(), "case-10".to_string()];
/// canonical_strings(&mut cases);
/// assert_eq!(cases, ["case-9", "case-10", "case-107"]);
/// ```
pub fn canonical_strings<S: AsRef<str>>(items: &mut [S]) {
    items.sort_by(|a, b| natural_cmp(a.as_ref(), b.as_ref()));
}

/// Is this collection already in canonical order? What a validator asks of a projection it
/// did not build, and what a test asks of an interface's output.
pub fn is_canonical<T: Ordered>(items: &[T]) -> bool {
    items
        .windows(2)
        .all(|pair| pair[0].order_key() <= pair[1].order_key())
}

/// A group compares naturally against another group; the absence of a group sorts last.
fn group_cmp(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => natural_cmp(a, b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Compare two human-facing strings the way a person reads them.
///
/// A run of ASCII digits compares by value, so `item-2` precedes `item-10` instead of
/// following it. Everything else compares by Unicode scalar, case-insensitively for ASCII
/// letters first so that `Alpha` and `alpha` sort together, and case-sensitively after, so
/// that two strings differing only in case still have a fixed order rather than comparing
/// equal and letting the caller's input order decide.
///
/// Separators are not normalised. `a-b` and `a_b` are different strings and stay different:
/// folding them together would make two distinct identities compare equal, which is exactly
/// the tie this order exists to break.
///
/// ```
/// use majordomus_cli::order::natural_cmp;
/// use std::cmp::Ordering;
///
/// assert_eq!(natural_cmp("item-2", "item-10"), Ordering::Less);
/// assert_eq!(natural_cmp("item-002", "item-2"), Ordering::Less);
/// assert_eq!(natural_cmp("Alpha", "beta"), Ordering::Less);
/// ```
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let folded = fold_cmp(a, b);
    if folded != Ordering::Equal {
        return folded;
    }
    // Same word to a reader; still two different strings. Compare the raw scalars so that
    // the pair has one fixed order on every machine.
    exact_cmp(a, b)
}

/// The reader's comparison: digit runs by value, letters folded to lower case.
fn fold_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                if x.is_ascii_digit() && y.is_ascii_digit() {
                    let ordering = digits_cmp(&mut a, &mut b);
                    if ordering != Ordering::Equal {
                        return ordering;
                    }
                    continue;
                }
                let ordering = fold(x).cmp(&fold(y));
                if ordering != Ordering::Equal {
                    return ordering;
                }
                a.next();
                b.next();
            }
        }
    }
}

/// Compare the digit run at the head of each iterator by the number it spells, consuming
/// both runs. A longer run of significant digits is the larger number; equal numbers are
/// separated by how they were written, so that `002` and `2` have a fixed order.
fn digits_cmp<I, J>(a: &mut std::iter::Peekable<I>, b: &mut std::iter::Peekable<J>) -> Ordering
where
    I: Iterator<Item = char>,
    J: Iterator<Item = char>,
{
    let left = take_digits(a);
    let right = take_digits(b);
    let (lt, rt) = (left.trim_start_matches('0'), right.trim_start_matches('0'));
    lt.len()
        .cmp(&rt.len())
        .then_with(|| lt.cmp(rt))
        // Equal value, different spelling: the one written with more leading zeros first.
        // Arbitrary, and fixed — which is the whole requirement.
        .then_with(|| right.len().cmp(&left.len()))
}

fn take_digits<I: Iterator<Item = char>>(chars: &mut std::iter::Peekable<I>) -> String {
    let mut run = String::new();
    while let Some(c) = chars.peek().copied() {
        if !c.is_ascii_digit() {
            break;
        }
        run.push(c);
        chars.next();
    }
    run
}

/// Scalar-by-scalar comparison, used only to separate two strings a reader would call the
/// same word.
fn exact_cmp(a: &str, b: &str) -> Ordering {
    a.cmp(b)
}

fn fold(c: char) -> char {
    c.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The keys of a fixture, in the order canonical() puts them in.
    fn ordered(mut items: Vec<Item>) -> Vec<&'static str> {
        canonical(&mut items);
        items.iter().map(|i| i.identity).collect()
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Item {
        group: Option<&'static str>,
        rank: i64,
        label: &'static str,
        identity: &'static str,
    }

    impl Item {
        const fn new(
            group: Option<&'static str>,
            rank: i64,
            label: &'static str,
            identity: &'static str,
        ) -> Self {
            Self {
                group,
                rank,
                label,
                identity,
            }
        }
    }

    impl Ordered for Item {
        fn order_key(&self) -> OrderKey<'_> {
            OrderKey {
                group: self.group,
                rank: self.rank,
                label: self.label,
                identity: self.identity,
            }
        }
    }

    const FIXTURE: [Item; 7] = [
        Item::new(Some("governance"), 0, "rules", "governance.rules"),
        Item::new(Some("governance"), 0, "doctrines", "governance.doctrines"),
        Item::new(Some("ai"), 0, "providers", "ai.providers"),
        Item::new(None, 0, "orphan", "orphan"),
        Item::new(Some("ai"), 0, "sessions", "ai.sessions"),
        Item::new(Some("knowledge"), 0, "adr-10", "knowledge.adr-10"),
        Item::new(Some("knowledge"), 0, "adr-2", "knowledge.adr-2"),
    ];

    #[test]
    fn groups_come_first_then_labels_then_the_ungrouped_tail() {
        assert_eq!(
            ordered(FIXTURE.to_vec()),
            [
                "ai.providers",
                "ai.sessions",
                "governance.doctrines",
                "governance.rules",
                "knowledge.adr-2",
                "knowledge.adr-10",
                "orphan",
            ]
        );
    }

    #[test]
    fn every_permutation_of_the_fixture_yields_the_same_order() {
        let expected = ordered(FIXTURE.to_vec());
        for permutation in permutations(&FIXTURE) {
            assert_eq!(
                ordered(permutation),
                expected,
                "a permutation ordered differently"
            );
        }
    }

    #[test]
    fn an_explicit_rank_beats_the_label_inside_its_group() {
        let lifecycle = vec![
            Item::new(Some("task"), 3, "conclude", "task.conclude"),
            Item::new(Some("task"), 1, "setup", "task.setup"),
            Item::new(Some("task"), 2, "work", "task.work"),
        ];
        assert_eq!(
            ordered(lifecycle),
            ["task.setup", "task.work", "task.conclude"]
        );
    }

    #[test]
    fn identical_labels_are_separated_by_identity() {
        let twins = vec![
            Item::new(Some("g"), 0, "same", "g.second"),
            Item::new(Some("g"), 0, "same", "g.first"),
        ];
        assert_eq!(ordered(twins), ["g.first", "g.second"]);
    }

    #[test]
    fn digit_runs_compare_by_value() {
        assert_eq!(natural_cmp("item-2", "item-10"), Ordering::Less);
        assert_eq!(natural_cmp("item-10", "item-9"), Ordering::Greater);
        assert_eq!(natural_cmp("v1.9.0", "v1.10.0"), Ordering::Less);
        assert_eq!(natural_cmp("adr-0009", "adr-0010"), Ordering::Less);
    }

    #[test]
    fn leading_zeros_do_not_change_the_value_but_do_fix_the_order() {
        assert_eq!(natural_cmp("item-02", "item-2"), Ordering::Less);
        assert_eq!(natural_cmp("item-2", "item-02"), Ordering::Greater);
        assert_eq!(natural_cmp("item-2", "item-2"), Ordering::Equal);
    }

    #[test]
    fn case_folds_first_and_separates_after() {
        assert_eq!(natural_cmp("Alpha", "beta"), Ordering::Less);
        assert_eq!(natural_cmp("alpha", "Beta"), Ordering::Less);
        assert_ne!(natural_cmp("Alpha", "alpha"), Ordering::Equal);
        assert_eq!(natural_cmp("alpha", "alpha"), Ordering::Equal);
    }

    #[test]
    fn separators_are_not_normalised_away() {
        assert_ne!(natural_cmp("a-b", "a_b"), Ordering::Equal);
        assert_ne!(natural_cmp("a b", "a-b"), Ordering::Equal);
    }

    #[test]
    fn a_prefix_precedes_the_string_that_extends_it() {
        assert_eq!(natural_cmp("serve", "serve-http"), Ordering::Less);
        assert_eq!(natural_cmp("", "a"), Ordering::Less);
        assert_eq!(natural_cmp("", ""), Ordering::Equal);
    }

    #[test]
    fn is_canonical_agrees_with_canonical() {
        let mut items = FIXTURE.to_vec();
        assert!(
            !is_canonical(&items),
            "the fixture is deliberately out of order"
        );
        canonical(&mut items);
        assert!(is_canonical(&items));
    }

    #[test]
    fn the_order_is_a_total_order_over_the_fixture() {
        // Antisymmetry and transitivity over every pair and triple: a comparator that fails
        // either can make sort() produce a different answer for a different input order.
        for a in FIXTURE.iter() {
            for b in FIXTURE.iter() {
                let (ab, ba) = (
                    a.order_key().cmp(&b.order_key()),
                    b.order_key().cmp(&a.order_key()),
                );
                assert_eq!(ab, ba.reverse(), "not antisymmetric: {a:?} vs {b:?}");
                assert_eq!(
                    ab == Ordering::Equal,
                    a == b,
                    "two distinct items compared equal"
                );
                for c in FIXTURE.iter() {
                    let bc = b.order_key().cmp(&c.order_key());
                    if ab == bc && ab != Ordering::Equal {
                        assert_eq!(a.order_key().cmp(&c.order_key()), ab, "not transitive");
                    }
                }
            }
        }
    }

    /// Every permutation of a slice, smallest first. Seven items is 5040 orders, which is
    /// cheap and exhaustive; a random shuffle would prove less and flake more.
    fn permutations<T: Copy>(items: &[T]) -> Vec<Vec<T>> {
        let mut out = Vec::new();
        let mut current: Vec<T> = items.to_vec();
        let n = current.len();
        let mut counters = vec![0usize; n];
        out.push(current.clone());
        let mut i = 0;
        while i < n {
            if counters[i] < i {
                let j = if i % 2 == 0 { 0 } else { counters[i] };
                current.swap(j, i);
                out.push(current.clone());
                counters[i] += 1;
                i = 0;
            } else {
                counters[i] = 0;
                i += 1;
            }
        }
        out
    }
}
