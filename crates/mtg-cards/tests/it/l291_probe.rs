//! Goal: a probe for card work, ignored by default: `L291=<slug,...>` prints what the compiler reads of each corpus card and the checker's report on its rules file.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;
use crate::common::checks::Corpus;

use mtg_cards::compile::compile;
use mtg_cards::load::slug;
use mtg_cards::proposal::check;
use mtg_cards::support::Caps;

#[test]
#[ignore = "probe"]
fn l291_probe() {
    let Ok(wanted) = std::env::var("L291") else {
        return;
    };
    let corpus = Corpus::repo();
    let library = corpus.library();
    let names = BTreeSet::new();
    for card in common::printed() {
        let s = slug(&card.name);
        if !wanted.split(',').any(|w| w == s) {
            continue;
        }
        let compiled = compile(&card);
        println!("== {} compiled:", card.name);
        for face in compiled.card.iter().flat_map(|c| c.faces.iter()) {
            for ability in &face.abilities {
                println!("  {}", serde_json::to_string(ability).unwrap());
            }
        }
        let path = common::cards_dir().join(format!("{s}.json"));
        if let Ok(text) = std::fs::read_to_string(&path) {
            let report = check(&text, &card, &library, &names, &Caps::everything());
            println!("{}", report.render(true));
        }
    }
}
