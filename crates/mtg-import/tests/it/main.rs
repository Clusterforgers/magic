//! Goal: deck lists and Scryfall data import through the corpus, fixtures and real exports; one test binary so the crate's integration tests share one link and run together on the thread pool.

mod corpus;
mod corpus_detect;
mod corpus_export;
mod corpus_parse;
mod decklist_formats;
mod mana_costs;
mod mtgjson_decks;
mod real_data;
mod real_exports;
mod resolve_corpus;
mod resolve_pipeline;
mod scryfall_bulk;
mod support;
