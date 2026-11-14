//! Goal: print what the compiler makes of the cards named in `R6405` (scratch aid, ignored by default).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::{Corpus, printed};
use mtg_cards::compile::compile;

#[test]
#[ignore = "scratch probe"]
fn probe() {
    let want = std::env::var("R6405").unwrap_or_default();
    let _ = Corpus::repo();
    for card in printed() {
        if !want.split('|').any(|n| n == card.name) {
            continue;
        }
        let out = compile(&card);
        println!("=== {}", card.name);
        for (i, face) in card.faces.iter().enumerate() {
            println!("[{i}] {}", face.oracle_text.replace('\n', "\n    "));
        }
        println!("status: {:?}", out.card.as_ref().map(|c| c.impl_status));
        for c in out.clauses.iter().filter(|c| !c.read) {
            println!("  UNREAD[{}] {}", c.face, c.text);
        }
    }
}
