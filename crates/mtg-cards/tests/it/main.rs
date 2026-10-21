//! Goal: the card DSL compiles, round-trips and reports on the corpus; one test binary so the crate's integration tests share one link and run together on the thread pool.

mod assemble;
mod common;
mod compile;
mod compile_corpus;
mod compile_real;
mod compile_shapes;
mod corpus;
mod deck_report;
mod printed;
mod roundtrip;
