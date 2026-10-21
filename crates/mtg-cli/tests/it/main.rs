//! Goal: fetching, caching and ingesting card data against local mock servers and a database per test; one test binary so the crate's integration tests share one link and run together on the thread pool.

mod commands;
mod common;
mod coverage;
mod fetch_decks;
mod fetch_scryfall;
mod ingest;
