//! Goal: an object's activated abilities reach the viewers who may read its rules in the words of its card (cost, effect, target phrases, loyalty cost, mana ability, keyword line) with the address `PriorityOptions.abilities` offers, for abilities of every kind and from every zone they work in, and never for a card in another seat's hand or library; a clause the compiler cannot read stays a manual clause.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::compile::compile;
use mtg_cards::support::{Caps, PrintedCard, PrintedFace};
use mtg_cards::{CardDefinition, assemble};
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Commander;
use mtg_protocol::{ActivatedView, ObjectView, StateView};
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A card with the rules the compiler reads from its oracle text, as the server loads it.
fn card(name: &str, type_line: &str, cost: &str, text: &str) -> Arc<CardDefinition> {
    let printed = PrintedCard {
        oracle_id: format!("oracle-{name}"),
        name: name.to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: cost.to_owned(),
            type_line: type_line.to_owned(),
            oracle_text: text.to_owned(),
            power: type_line.contains("Creature").then(|| "2".to_owned()),
            toughness: type_line.contains("Creature").then(|| "2".to_owned()),
            loyalty: type_line.contains("Planeswalker").then(|| "4".to_owned()),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    };
    let rules = compile(&printed).card;
    Arc::new(assemble(&printed, rules.as_ref(), &Caps::everything()).definition)
}

/// Where the card lies at the start of the game.
#[derive(Debug, Clone, Copy)]
enum Place {
    Battlefield,
    Hand,
    Graveyard,
    Command,
}

/// Seat 0 has `card` in `place`, six Forests in play and a library; every other seat has a library.
fn table(seats: usize, card: &Arc<CardDefinition>, place: Place) -> Driver {
    let mut field: Vec<_> = (0..6).map(|_| ready(forest())).collect();
    if matches!(place, Place::Battlefield) {
        field.push(ready(Arc::clone(card)));
    }
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 6])
        .battlefield(0, field);
    scenario = match place {
        Place::Battlefield => scenario,
        Place::Hand => scenario.hand(0, [Arc::clone(card)]),
        Place::Graveyard => scenario.graveyard(0, [Arc::clone(card)]),
        Place::Command => scenario
            .format(Arc::new(Commander))
            .commanders(0, [Arc::clone(card)]),
    };
    for other in 1..seats {
        scenario = scenario.library(other, vec![forest(); 6]);
    }
    Driver::from_scenario(scenario).unwrap()
}

fn named(view: &StateView, name: &str) -> Option<ObjectView> {
    view.objects.iter().find(|o| o.name == name).cloned()
}

/// The words of each ability: `cost: effect`, then `[targets]`, the loyalty and mana flags and the keyword line.
fn words(abilities: &[ActivatedView]) -> Vec<String> {
    abilities
        .iter()
        .map(|a| {
            let mut text = format!("{}: {}", a.cost, a.effect);
            if !a.targets.is_empty() {
                text = format!("{text} [{}]", a.targets.join("; "));
            }
            if a.mana {
                text.push_str(" (mana)");
            }
            if let Some(keyword) = &a.keyword {
                text = format!("{text} <{keyword}>");
            }
            if a.unworded {
                text.push_str(" (unworded)");
            }
            text
        })
        .collect()
}

#[rstest]
#[case::tap_and_sacrifice(
    "Mind Stone", "Artifact", "{2}", "{T}: Add {C}.\n{1}, {T}, Sacrifice Mind Stone: Draw a card.", Place::Battlefield,
    &["{T}: Add {C}. (mana)", "{1}, {T}, Sacrifice Mind Stone: Draw a card."]
)]
#[case::mana_in_rules(
    "Sol Ring", "Artifact", "{1}", "{T}: Add {C}{C}.", Place::Battlefield,
    &["{T}: Add {C}{C}. (mana)"]
)]
#[case::with_a_target(
    "Pyromancer", "Creature — Human Wizard", "{2}{R}", "{T}: Pyromancer deals 1 damage to any target.",
    Place::Battlefield, &["{T}: Pyromancer deals 1 damage to any target. [any target]"]
)]
#[case::with_x(
    "Mill Stone", "Artifact", "{2}", "{X}, {T}: Target player mills X cards.", Place::Battlefield,
    &["{X}, {T}: Target player mills X cards. [Target player]"]
)]
#[case::loyalty(
    "Walker", "Legendary Planeswalker — Ajani", "{2}{W}{W}",
    "+1: You gain 2 life.\n\u{2212}2: Destroy target creature.", Place::Battlefield,
    &["+1: You gain 2 life.", "\u{2212}2: Destroy target creature. [target creature]"]
)]
#[case::cycling_in_hand(
    "Cycler", "Creature — Beast", "{3}{G}", "Cycling {2}", Place::Hand,
    &[": Cycling {2} <Cycling {2}> (unworded)"]
)]
#[case::unearth_in_graveyard(
    "Unearther", "Creature — Zombie", "{1}{B}", "Unearth {B}", Place::Graveyard,
    &["{B}: Unearth {B} <Unearth {B}> (unworded)"]
)]
#[case::flashback_is_a_cast(
    "Flasher", "Sorcery", "{1}{U}", "Draw two cards.\nFlashback {3}{U}", Place::Graveyard,
    &[]
)]
#[case::in_the_command_zone(
    "Commander", "Legendary Creature — Elf", "{G}", "{1}, {T}: You gain 1 life.", Place::Command,
    &["{1}, {T}: You gain 1 life."]
)]
fn the_abilities_are_worded_as_the_card_prints_them(
    #[case] name: &str,
    #[case] type_line: &str,
    #[case] cost: &str,
    #[case] text: &str,
    #[case] place: Place,
    #[case] expected: &[&str],
    #[values(2, 3, 4)] seats: usize,
) {
    let card = card(name, type_line, cost, text);
    let driver = table(seats, &card, place);
    let view = StateView::for_player(driver.state(), PlayerId::from_index(0));
    let object = named(&view, name).unwrap();
    assert_eq!(words(&object.activated), expected, "{:?}", object.manual);
}

#[rstest]
fn an_offered_ability_is_named_by_the_address_of_its_words(#[values(2, 3, 4)] seats: usize) {
    let stone = card(
        "Mind Stone",
        "Artifact",
        "{2}",
        "{T}: Add {C}.\n{1}, {T}, Sacrifice Mind Stone: Draw a card.",
    );
    let driver = table(seats, &stone, Place::Battlefield);
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("seat 0 holds priority");
    };
    let view = StateView::for_player(driver.state(), PlayerId::from_index(0));
    let object = named(&view, "Mind Stone").unwrap();
    let addresses: Vec<_> = object.activated.iter().filter_map(|a| a.ability).collect();
    assert_ne!(options.abilities, Vec::new());
    for offered in &options.abilities {
        assert!(addresses.contains(offered), "{offered:?} is worded");
    }
}

#[rstest]
#[case::unread_effect(
    "Oddity",
    "Artifact",
    "{T}: Each player shuffles their name into a hat.",
    Place::Battlefield
)]
#[case::unread_ability_word(
    "Channeler",
    "Creature — Spirit",
    "Channel — {1}{G}, Discard Channeler: Target creature gets +2/+2 until end of turn.",
    Place::Hand
)]
fn an_unread_ability_stays_a_manual_clause(
    #[case] name: &str,
    #[case] type_line: &str,
    #[case] text: &str,
    #[case] place: Place,
    #[values(2, 4)] seats: usize,
) {
    let card = card(name, type_line, "{2}", text);
    let driver = table(seats, &card, place);
    let view = StateView::for_player(driver.state(), PlayerId::from_index(0));
    let object = named(&view, name).unwrap();
    assert_eq!(object.activated, Vec::new());
    assert!(object.manual.iter().any(|clause| clause.text == text));
}

/// Where the card lies, and whether the viewers that do not own it read its abilities.
#[rstest]
#[case::battlefield(Place::Battlefield, true)]
#[case::graveyard(Place::Graveyard, true)]
#[case::hand(Place::Hand, false)]
fn the_abilities_reach_the_viewers_who_read_the_card(
    #[case] place: Place,
    #[case] others_read: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    let cycler = card("Cycler", "Creature — Beast", "{3}{G}", "Cycling {2}");
    let driver = table(seats, &cycler, place);
    // Every seat and a spectator.
    for viewer in (0..=seats).map(PlayerId::from_index) {
        let view = StateView::for_player(driver.state(), viewer);
        let reads = viewer == PlayerId::from_index(0) || others_read;
        let worded = named(&view, "Cycler").is_some_and(|o| !o.activated.is_empty());
        assert_eq!(worded, reads, "{viewer:?}");
        let json = serde_json::to_string(&view).unwrap();
        assert_eq!(json.contains("Cycling {2}"), reads, "{viewer:?}");
    }
}

#[rstest]
fn a_card_with_no_activated_ability_leaves_the_field_out(#[values(2, 3)] seats: usize) {
    let driver = table(seats, &forest(), Place::Hand);
    let view = StateView::for_player(driver.state(), PlayerId::from_index(1));
    for object in &view.objects {
        let json = serde_json::to_value(object).unwrap();
        assert_eq!(
            json.get("activated").is_some(),
            !object.activated.is_empty()
        );
        assert!(
            object
                .activated
                .iter()
                .all(|a| a.mana && a.ability.is_none())
        );
    }
}
