//! Postgres schema and repositories for card data: oracle cards, printings, rulings, precon decks, import runs, the compiled rules of cards, the decks accounts store and the cards they mark as important to fix.

pub mod card_rules;
pub mod cards;
pub mod codec;
pub mod colors;
pub mod database;
pub mod decks;
pub mod error;
pub mod fix_marks;
pub mod import;
pub mod model;
pub mod precons;
mod rows;
pub mod runs;

pub use card_rules::{CardRulesRepo, NewRules, RuleCounts, RuleStamp, RulesVersion, StoredRules};
pub use cards::{
    CardPage, CardRepo, CardSearch, ColorFilter, ColorMatch, IndexVersion, NameSuggestion, Page,
    PrintingPictures, SortKey, TokenPictures,
};
pub use database::{Database, RowCounts};
pub use decks::{CreateDeckError, DeckDraft, DeckId, DeckLine, DeckRepo, DeckZone, StoredDeck};
pub use error::DbError;
pub use fix_marks::{FixMark, FixMarkRepo, MAX_NOTE_CHARS, RankedMark};
pub use import::{
    CheckReport, ImportError, Importer, PrintingImportReport, check_cards, check_precons,
    check_printings, check_rulings,
};
pub use model::{Card, CardDetail, ImplStatus, Printing, PrintingImages, Ruling, StoredFace};
pub use precons::{
    DeckBoard, PreconDeckDetail, PreconDeckId, PreconDeckSummary, PreconEntry, PreconRepo,
    PreconVersion,
};
pub use runs::{ImportRun, ImportRuns, ImportSource, RunId, RunStatus, STALE_AFTER};
