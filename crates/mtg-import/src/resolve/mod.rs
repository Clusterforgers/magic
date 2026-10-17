//! Resolving deck-list names to cards: normalized exact matching plus fuzzy suggestions.

mod deck;
mod distance;
mod index;
mod normalize;

pub use deck::{
    Resolution, ResolveError, ResolvedEntry, SUGGESTION_LIMIT, UnresolvedEntry, resolve_deck,
};
pub use index::{CardIndex, IndexedCard, Lookup, Suggestion};
pub use normalize::normalize_name;
