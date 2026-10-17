//! Strongly typed identifiers.

use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl $name {
            /// Raw numeric value.
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

id_type!(
    /// A player seat in a game.
    PlayerId
);
id_type!(
    /// A game object (card, token, spell, ability on the stack).
    ObjectId
);

#[cfg(test)]
mod tests {
    //! Goal: ids serialize as bare numbers so the wire format stays stable.
    use super::*;

    #[test]
    fn ids_serialize_transparently() {
        assert_eq!(serde_json::to_string(&PlayerId(7)).unwrap(), "7");
        assert_eq!(
            serde_json::from_str::<ObjectId>("42").unwrap(),
            ObjectId(42)
        );
    }
}
