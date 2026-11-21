//! Goal: scratch.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use crate::common;
#[test]
fn show() {
    let texts = [
        "Create X 1/1 white Soldier creature tokens.",
        "Destroy all other creatures.",
        "If X is 5 or more, destroy all creatures.",
        "If X is 5 or more, destroy all other creatures.",
        "Create X 1/1 white Soldier creature tokens. Destroy all other creatures.",
        "Create X 1/1 white Soldier creature tokens. If X is 5 or more, destroy all creatures.",
        "Create X 1/1 white Soldier creature tokens. If X is 5 or more, destroy all other creatures.",
        "You gain X life. Create X 1/1 colorless Phyrexian Mite artifact creature tokens with toxic 1 and \"This token can't block.\"",
        "Create a 2/2 colorless Wizard Soldier creature token named Cadet, then put a +1/+1 counter on each other Wizard token you control.",
    ];
    for card in common::printed() {
        if card.name == "Martial Coup" {
            for text in texts {
                let mut card = card.clone();
                card.faces[0].oracle_text = text.to_owned();
                let c = mtg_cards::compile::compile(&card);
                println!("{text}\n  => {}", serde_json::to_string(&c.card.map(|c| c.faces[0].abilities.clone())).unwrap());
            }
        }
    }
}
