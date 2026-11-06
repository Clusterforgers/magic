//! Goal: a merge keeps every ability a clause stands for, also when one clause stands for several ("I, II — ..."), over the Sagas of the design corpus; a delayed trigger a spell sets up is a line of the spell, and an ability word may end in a number ("Descend 4").
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::compile::{ClauseKind, classify_clause};
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

#[rstest::rstest]
#[case::a_pact(
    "At the beginning of your next upkeep, pay {2}{B}. If you don't, you lose the game.",
    true,
    ClauseKind::Spell
)]
#[case::a_trigger_this_turn(
    "Whenever a creature blocks this turn, it gets +0/+1 until end of turn.",
    true,
    ClauseKind::Spell
)]
#[case::when_you_next_cast(
    "When you next cast an instant spell this turn, copy that spell.",
    true,
    ClauseKind::Spell
)]
#[case::the_same_on_a_permanent(
    "Whenever a creature blocks this turn, it gets +0/+1 until end of turn.",
    false,
    ClauseKind::Triggered
)]
#[case::a_spell_trigger_of_its_own(
    "When you cycle this card, draw a card.",
    true,
    ClauseKind::Triggered
)]
#[case::this_turn_past_the_event(
    "Whenever a creature dealt damage by enchanted creature this turn dies, put a +1/+1 counter on that creature.",
    true,
    ClauseKind::Triggered
)]
#[case::an_ability_word_with_a_number(
    "Descend 4 \u{2014} Whenever this creature attacks, if there are four or more permanent cards in your graveyard, put a +1/+1 counter on target creature.",
    false,
    ClauseKind::Triggered
)]
#[case::an_ability_word_with_a_number_on_an_activation(
    "Descend 8 \u{2014} {4}{U}{B}: Return this card from your graveyard to the battlefield.",
    false,
    ClauseKind::Activated
)]
fn a_delayed_trigger_of_a_spell_is_its_effect(
    #[case] text: &str,
    #[case] spell: bool,
    #[case] kind: ClauseKind,
) {
    assert_eq!(classify_clause(text, spell), kind);
}
