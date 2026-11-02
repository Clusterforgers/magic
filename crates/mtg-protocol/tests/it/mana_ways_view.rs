//! Goal: each mana ability in `ObjectView.activated` carries what it adds as data (the mana counted now for its controller, the mana a use costs, the sacrifice, the untapped use), the same for every viewer who reads the card, and nothing reaches a viewer who may not read it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::compile::compile;
use mtg_cards::support::{Caps, PrintedCard, PrintedFace};
use mtg_cards::{CardDefinition, assemble};
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;
use mtg_protocol::{ManaWayView, ObjectView, StateView};
use mtg_testkit::cards::forest;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// An artifact with the rules the compiler reads from its oracle text, as the server loads it.
fn artifact(name: &str, text: &str) -> Arc<CardDefinition> {
    let printed = PrintedCard {
        oracle_id: format!("oracle-{name}"),
        name: name.to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: "{2}".to_owned(),
            type_line: "Artifact".to_owned(),
            oracle_text: text.to_owned(),
            ..PrintedFace::default()
        }],
        ..PrintedCard::default()
    };
    let rules = compile(&printed).card;
    Arc::new(assemble(&printed, rules.as_ref(), &Caps::everything()).definition)
}

/// Seat 0 has `card` and six Forests in play (or `card` in hand); every seat has a library.
fn table(seats: usize, card: &Arc<CardDefinition>, in_hand: bool) -> Driver {
    let mut field: Vec<_> = (0..6).map(|_| ready(forest())).collect();
    if !in_hand {
        field.push(ready(Arc::clone(card)));
    }
    let mut scenario = Scenario::new(seats)
        .library(0, vec![forest(); 6])
        .battlefield(0, field);
    if in_hand {
        scenario = scenario.hand(0, [Arc::clone(card)]);
    }
    for other in 1..seats {
        scenario = scenario.library(other, vec![forest(); 6]);
    }
    Driver::from_scenario(scenario).unwrap()
}

fn named(view: &StateView, name: &str) -> Option<ObjectView> {
    view.objects.iter().find(|o| o.name == name).cloned()
}

/// Each way in short: the mana as letters, then `+cost`, `sac` and `free` (untapped).
fn short(ways: &[ManaWayView]) -> Vec<String> {
    ways.iter()
        .map(|way| {
            let mut text: String = way
                .mana
                .iter()
                .map(|mana| match mana {
                    ManaType::White => 'W',
                    ManaType::Blue => 'U',
                    ManaType::Black => 'B',
                    ManaType::Red => 'R',
                    ManaType::Green => 'G',
                    ManaType::Colorless => 'C',
                })
                .collect();
            if !way.cost.symbols().is_empty() {
                text.push_str(" +");
                text.push_str(&way.cost.symbols().len().to_string());
            }
            if way.sacrifice {
                text.push_str(" sac");
            }
            if way.tapless {
                text.push_str(" free");
            }
            text
        })
        .collect()
}

#[rstest]
#[case::plain("{T}: Add {C}.", "C")]
#[case::two_at_once("{T}: Add {C}{C}.", "CC")]
#[case::signet("{1}, {T}: Add {W}{U}.", "WU +1")]
#[case::a_choice("{T}: Add {R} or {G}.", "R/G")]
#[case::any_colour_and_sacrifice(
    "{T}, Sacrifice this artifact: Add one mana of any color.",
    "W sac/U sac/B sac/R sac/G sac"
)]
#[case::counted_now("{T}: Add {G} for each land you control.", "GGGGGG")]
#[case::untapped("Sacrifice this artifact: Add {R}.", "R sac free")]
#[case::painland(
    "{T}: Add {C}.\n{T}: Add {B}. This artifact deals 1 damage to you.",
    "C | B"
)]
fn a_mana_ability_carries_what_it_adds(
    #[case] text: &str,
    #[case] expected: &str,
    #[values(2, 3)] seats: usize,
) {
    let card = artifact("Rock", text);
    let driver = table(seats, &card, false);
    for viewer in (0..=seats).map(PlayerId::from_index) {
        let view = StateView::for_player(driver.state(), viewer);
        let object = named(&view, "Rock").unwrap();
        // The ways of one ability between slashes, the abilities between bars.
        let ways = object
            .activated
            .iter()
            .filter(|a| a.mana)
            .map(|a| short(&a.ways).join("/"))
            .collect::<Vec<_>>()
            .join(" | ");
        assert_eq!(ways, expected, "{viewer:?} {:?}", object.manual);
    }
}

#[rstest]
fn another_ability_carries_no_ways(#[values(2, 3)] seats: usize) {
    let card = artifact("Rock", "{T}: Add {C}.\n{1}, {T}: You gain 1 life.");
    let driver = table(seats, &card, false);
    let view = StateView::for_player(driver.state(), PlayerId::from_index(0));
    let object = named(&view, "Rock").unwrap();
    assert!(object.activated.iter().all(|a| a.mana != a.ways.is_empty()));
}

#[rstest]
fn the_ways_of_a_card_in_hand_reach_only_its_owner(#[values(2, 3, 4)] seats: usize) {
    let card = artifact("Rock", "{1}, {T}: Add {W}{U}.");
    let driver = table(seats, &card, true);
    for viewer in (0..=seats).map(PlayerId::from_index) {
        let view = StateView::for_player(driver.state(), viewer);
        let owner = viewer == PlayerId::from_index(0);
        let ways = named(&view, "Rock").map(|o| o.activated.iter().any(|a| !a.ways.is_empty()));
        assert_eq!(ways == Some(true), owner, "{viewer:?}");
        // Nothing else on the table is blue: the Rock's way is the only blue mana in the view.
        let json = serde_json::to_string(&view).unwrap();
        assert_eq!(json.contains("\"Blue\""), owner, "{viewer:?}");
    }
}
