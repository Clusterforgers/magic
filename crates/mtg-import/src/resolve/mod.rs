//! Resolving deck-list entries to cards: printing first, then normalized names, then other names of printings; fuzzy suggestions only suggest.

mod deck;
mod distance;
mod index;
mod normalize;

pub use deck::{
    MatchKind, Resolution, ResolveError, ResolvedEntry, SUGGESTION_LIMIT, UnresolvedEntry,
    resolve_deck,
};
pub use index::{CardIndex, IndexedCard, Lookup, Suggestion};
pub use normalize::normalize_name;
