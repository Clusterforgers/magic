//! Card rules data: the DSL types, their loader and validation, and support records.
//!
//! Design record: `docs/cards-dsl.md`. A card's printed side comes from the imported record; the
//! files in `cards/` hold only rules, keyed by oracle id.

pub mod ability;
pub mod card;
pub mod cost;
pub mod effect;
pub mod keyword;
pub mod load;
pub mod mana;
pub mod modify;
pub mod refs;
pub mod support;
pub mod trigger;
pub mod validate;
pub mod value;
pub mod vanilla;
