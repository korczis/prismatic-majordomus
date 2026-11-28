//! Protocol versions, and the negotiation that picks one per connection.
//!
//! A client says the range it speaks, the host the range it speaks, and the connection
//! runs at the highest version both do. A range with nothing in common is
//! [`RefusalCode::ProtocolUnsupported`], with both ranges in the message, and the connection
//! ends. Within a version, optional behaviour is a named feature: a client lists what it
//! would use and what it cannot do without; the host keeps the ones it offers and ignores
//! the rest, and refuses only a *required* feature it does not offer. An unknown optional
//! feature is therefore never an error, which is what lets a newer client talk to an older
//! host.
//!
//! ```
//! use majordomus_cli::fabric::{ProtocolRange, ProtocolVersion};
//!
//! let client = ProtocolRange::new(1, 3);
//! let host = ProtocolRange::new(1, 2);
//! assert_eq!(client.highest_common(&host), Some(ProtocolVersion(2)));
//! assert_eq!(ProtocolRange::new(3, 4).highest_common(&host), None);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One protocol version. Versions are whole numbers counted from 1; a change that an
/// older peer would misread is a new version, and anything else is a feature.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ProtocolVersion(pub u16);

/// The versions one side speaks, both ends inclusive. A range whose `min` is over its
/// `max` speaks nothing and overlaps nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProtocolRange {
    /// The oldest version spoken.
    pub min: ProtocolVersion,
    /// The newest version spoken.
    pub max: ProtocolVersion,
}

/// The versions this build speaks.
pub const PROTOCOL: ProtocolRange = ProtocolRange::new(1, 1);

impl ProtocolRange {
    /// The range from `min` to `max`.
    pub const fn new(min: u16, max: u16) -> Self {
        ProtocolRange {
            min: ProtocolVersion(min),
            max: ProtocolVersion(max),
        }
    }

    /// The newest version in both ranges.
    pub fn highest_common(&self, other: &ProtocolRange) -> Option<ProtocolVersion> {
        let low = self.min.max(other.min);
        let high = self.max.min(other.max);
        (low <= high).then_some(high)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn an_inverted_range_overlaps_nothing() {
        assert_eq!(
            ProtocolRange::new(2, 1).highest_common(&ProtocolRange::new(1, 9)),
            None
        );
        assert_eq!(PROTOCOL.highest_common(&PROTOCOL), Some(ProtocolVersion(1)));
        let wire = serde_json::to_value(PROTOCOL).unwrap();
        assert_eq!(wire, serde_json::json!({ "min": 1, "max": 1 }));
        assert!(serde_json::from_value::<ProtocolRange>(serde_json::json!({ "min": 1 })).is_err());
    }

    proptest! {
        #[test]
        fn the_chosen_version_is_in_both_ranges_and_the_newest_such(
            a in 0u16..8, b in 0u16..8, c in 0u16..8, d in 0u16..8,
        ) {
            let x = ProtocolRange::new(a, b);
            let y = ProtocolRange::new(c, d);
            let chosen = x.highest_common(&y);
            prop_assert_eq!(chosen, y.highest_common(&x), "symmetric");
            let both = |v: u16| a <= v && v <= b && c <= v && v <= d;
            match chosen {
                Some(ProtocolVersion(v)) => {
                    prop_assert!(both(v));
                    prop_assert!(!(v + 1..8).any(both));
                }
                None => prop_assert!(!(0..8).any(both)),
            }
        }
    }
}
