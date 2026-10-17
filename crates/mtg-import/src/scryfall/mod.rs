//! Scryfall bulk data: raw serde models, normalized records and streaming readers.

pub mod array_stream;
mod normalize;
pub mod raw;
pub mod record;
pub mod stream;

pub use normalize::InvalidRecord;
pub use record::{
    CardFace, CardRecord, ImageUris, Layout, Legality, ProducedMana, Rarity, RulingRecord,
    RulingSource,
};
pub use stream::{BulkStream, CardStream, RulingStream, ScryfallError, cards, rulings};
