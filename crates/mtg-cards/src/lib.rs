//! Card rules data: the DSL types, their loader and validation, and support records.
//!
//! Design record: `docs/cards-dsl.md`. A card's printed side comes from the imported record; the
//! files in `cards/` hold only rules, keyed by oracle id.

pub mod ability;
pub mod assemble;
pub mod builtin;
pub mod card;
pub mod cost;
pub mod definition;
pub mod effect;
pub mod keyword;
pub mod load;
pub mod mana;
pub mod modify;
pub mod pretty;
pub mod refs;
pub mod support;
pub mod trigger;
pub mod validate;
pub mod value;
pub mod vanilla;

pub use assemble::{Assembled, assemble};
pub use definition::{AbilityDef, CardDefinition, ManaAbility};
pub use support::{Caps, PrintedCard, PrintedFace};
