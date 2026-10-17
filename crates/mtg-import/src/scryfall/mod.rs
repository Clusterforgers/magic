//! Scryfall bulk data: raw serde models, normalized records and streaming readers.

pub mod array_stream;
pub mod line_stream;
mod normalize;
pub mod raw;
pub mod record;
pub mod stream;

pub use normalize::InvalidRecord;
pub use record::{
    CardFace, CardRecord, ImageUris, Layout, Legality, ProducedMana, Rarity, RulingRecord,
    RulingSource,
};
pub use stream::{
    BulkStream, CardStream, JsonlCardStream, JsonlRulingStream, RulingStream, ScryfallError, cards,
    cards_jsonl, rulings, rulings_jsonl,
};
