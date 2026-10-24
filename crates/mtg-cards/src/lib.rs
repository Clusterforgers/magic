//! Card rules data: the DSL types, their loader and validation, support records, and the assembly
//! of a card definition from an imported record and its rules.
//!
//! Design record: `docs/cards-dsl.md`. A card's printed side comes from the imported record; the
//! files in `cards/` hold only rules, keyed by oracle id.

pub mod ability;
pub mod assemble;
pub mod builtin;
pub mod card;
pub mod companion;
pub mod compile;
pub mod cost;
pub mod coverage;
pub mod deck;
pub mod definition;
pub mod effect;
pub mod generated;
mod identity;
pub mod keyword;
pub mod load;
pub mod mana;
pub mod mana_trigger;
pub mod modify;
pub mod pretty;
pub mod proposal;
pub mod reference;
pub mod refs;
pub mod spend;
pub mod support;
pub mod trigger;
pub mod validate;
pub mod value;
pub mod vanilla;

pub use assemble::{Assembled, assemble};
pub use definition::{AbilityDef, CardDefinition, ManaAbility, ManualClause};
pub use support::{Caps, PrintedCard, PrintedFace};
