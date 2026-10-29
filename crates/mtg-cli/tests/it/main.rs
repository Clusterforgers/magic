//! Goal: fetching, caching and ingesting card data against local mock servers and a database per test; one test binary so the crate's integration tests share one link and run together on the thread pool.

mod audit;
mod c11i_propose_local;
mod check_rules;
mod commands;
mod common;
mod compile;
mod coverage;
mod coverage_all;
mod fetch_decks;
mod fetch_printings;
mod fetch_scryfall;
mod forge_check;
mod ingest;
mod ingest_printings;
mod propose;
mod propose_http;
mod w2a_promote;
mod w3a_propose_schema;
