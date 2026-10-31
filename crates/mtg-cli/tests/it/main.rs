//! Goal: fetching, caching and ingesting card data against local mock servers and a database per test; one test binary so the crate's integration tests share one link and run together on the thread pool.

mod audit;
mod c11i_propose_local;
mod check_rules;
mod commands;
mod common;
mod compile;
mod coverage;
mod coverage_all;
mod draft_store;
mod drafts;
mod drafts_smoke;
mod fetch_decks;
mod fetch_printings;
mod fetch_scryfall;
mod fix_marks;
mod forge_check;
mod forge_hint;
mod gaps;
mod ingest;
mod ingest_printings;
mod promote_draft;
mod propose;
mod propose_http;
mod t18a_propose_speed;
mod w2a_promote;
mod w3a_propose_schema;
