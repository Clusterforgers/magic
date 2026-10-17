//! Identifier newtypes for imported card data.

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! uuid_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// The identifier as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

uuid_id!(
    /// Scryfall oracle id: identifies a card across all of its printings.
    OracleId
);
uuid_id!(
    /// Scryfall card id: identifies a single printing.
    PrintingId
);

#[cfg(test)]
mod tests {
    //! Goal: ids serialize as bare strings so stored JSON and API payloads stay stable.
    use super::*;

    #[test]
    fn ids_are_transparent_strings() {
        let id = OracleId("abc".into());
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"abc\"");
        assert_eq!(
            serde_json::from_str::<PrintingId>("\"xyz\"")
                .unwrap()
                .as_str(),
            "xyz"
        );
        assert_eq!(id.to_string(), "abc");
    }
}
