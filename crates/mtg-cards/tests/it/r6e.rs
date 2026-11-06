//! Goal: a merge keeps every ability a clause stands for, also when one clause stands for several ("I, II — ..."), over the Sagas of the design corpus.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::proposal::{check, merge};
use mtg_cards::trigger::TriggerPattern;

use crate::common::{Corpus, printed};

/// The chapter numbers of a card's abilities.
fn chapters<'a>(abilities: impl Iterator<Item = &'a Ability>) -> Vec<u32> {
    abilities
        .filter_map(|ability| match ability {
            Ability::Triggered {
                when: TriggerPattern::Chapter { number },
                ..
            } => Some(*number),
            _ => None,
        })
        .collect()
}

#[test]
fn a_merged_saga_keeps_every_chapter_of_a_line_of_several() {
    let library = Corpus::repo().library();
    let cards = printed();
    let names: BTreeSet<String> = cards.iter().map(|c| c.name.to_lowercase()).collect();
    let caps = Caps::everything();
    let mut tried = 0;
    for printed in &cards {
        let several = printed.faces.iter().any(|face| {
            face.oracle_text
                .lines()
                .any(|line| line.starts_with("I, II"))
        });
        let Some(by_hand) = library.card(&printed.oracle_id) else {
            continue;
        };
        if !several {
            continue;
        }
        let text = serde_json::to_string(by_hand).unwrap();
        let proposal = check(&text, printed, &library, &names, &caps);
        let Some(merged) = merge(&proposal, printed, &library, &names, &caps) else {
            continue;
        };
        let card = merged.rules.unwrap();
        tried += 1;
        assert_eq!(
            chapters(card.abilities().map(|(_, a)| a)),
            chapters(by_hand.abilities().map(|(_, a)| a)),
            "{}",
            printed.name
        );
    }
    assert!(tried >= 1, "no Saga with a line of several chapters merged");
}
