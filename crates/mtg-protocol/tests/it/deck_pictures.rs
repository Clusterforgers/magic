//! Goal: the pictures a viewer may load ahead name its own deck whole and of every other seat only its commanders and companion, so a hidden list never reaches a client; each card comes once, with a picture per face only for a double-faced card.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::definition::Layout;
use mtg_cards::{CardDefinition, CardIdentity};
use mtg_core::ids::PlayerId;
use mtg_engine::{GameSetup, PresetPermanent};
use mtg_protocol::{PictureCard, deck_pictures};
use mtg_testkit::fuzz_setup;
use rstest::rstest;

fn card(oracle: &str, printing: Option<&str>) -> Arc<CardDefinition> {
    let mut def = CardDefinition::new(oracle);
    def.identity = Some(CardIdentity {
        oracle_id: oracle.to_owned(),
        printing_id: printing.map(str::to_owned),
    });
    Arc::new(def)
}

fn shaped(oracle: &str, layout: Layout) -> Arc<CardDefinition> {
    let mut def = (*card(oracle, None)).clone();
    def.layout = layout;
    def.faces = vec![CardDefinition::new("front"), CardDefinition::new("back")];
    Arc::new(def)
}

/// Two seats whose every card is named for its seat and place: `s0-lib-a`, `s1-cmd`, and so on.
fn setup() -> GameSetup {
    let mut setup = fuzz_setup(2, 7);
    for (at, seat) in setup.players.iter_mut().enumerate() {
        let name = |what: &str| format!("s{at}-{what}");
        seat.library = vec![
            card(&name("lib-a"), Some("p1")),
            card(&name("lib-a"), Some("p1")),
            card(&name("lib-a"), Some("p2")),
            card(&name("lib-b"), None),
            shaped(&name("dfc"), Layout::Transform),
            shaped(&name("split"), Layout::Split),
            Arc::new(CardDefinition::new("No identity")),
        ];
        seat.hand = vec![card(&name("hand"), None)];
        seat.graveyard = vec![card(&name("yard"), None)];
        seat.battlefield = vec![PresetPermanent::new(card(&name("field"), None))];
        seat.commanders = vec![card(&name("cmd"), Some("pc"))];
        seat.companion = Some(card(&name("companion"), None));
        seat.satchel = vec![card(&name("satchel"), None)];
    }
    setup
}

fn names(cards: &[PictureCard]) -> Vec<String> {
    cards
        .iter()
        .map(|card| match &card.identity.printing_id {
            Some(printing) => format!("{}@{printing}", card.identity.oracle_id),
            None => card.identity.oracle_id.clone(),
        })
        .collect()
}

const OWN_0: &[&str] = &[
    "s0-cmd@pc",
    "s0-companion",
    "s0-lib-a@p1",
    "s0-lib-a@p2",
    "s0-lib-b",
    "s0-dfc",
    "s0-split",
    "s0-hand",
    "s0-field",
    "s0-yard",
    "s0-satchel",
];
const OWN_1: &[&str] = &[
    "s1-cmd@pc",
    "s1-companion",
    "s1-lib-a@p1",
    "s1-lib-a@p2",
    "s1-lib-b",
    "s1-dfc",
    "s1-split",
    "s1-hand",
    "s1-field",
    "s1-yard",
    "s1-satchel",
];

const PUBLIC_0: &[&str] = &["s0-cmd@pc", "s0-companion"];
const PUBLIC_1: &[&str] = &["s1-cmd@pc", "s1-companion"];

#[rstest]
#[case::seat_0(0, &[OWN_0, PUBLIC_1])]
#[case::seat_1(1, &[OWN_1, PUBLIC_0])]
#[case::spectator(2, &[PUBLIC_0, PUBLIC_1])]
fn a_viewer_gets_its_own_deck_and_the_public_cards_of_the_others(
    #[case] viewer: usize,
    #[case] expected: &[&[&str]],
) {
    let cards = deck_pictures(&setup(), PlayerId::from_index(viewer));
    let expected: Vec<String> = expected.concat().iter().map(|&n| n.to_owned()).collect();
    assert_eq!(names(&cards), expected);
}

#[rstest]
#[case::transform("s0-dfc", 2)]
#[case::split("s0-split", 1)]
#[case::normal("s0-lib-b", 1)]
fn a_double_faced_card_alone_has_two_pictures(#[case] oracle: &str, #[case] faces: u8) {
    let cards = deck_pictures(&setup(), PlayerId::from_index(0));
    let card = cards
        .iter()
        .find(|card| card.identity.oracle_id == oracle)
        .unwrap();
    assert_eq!(card.faces, faces);
}

#[test]
fn no_other_seats_hidden_card_is_named_to_any_viewer() {
    let setup = setup();
    for viewer in 0..=2 {
        for card in deck_pictures(&setup, PlayerId::from_index(viewer)) {
            let oracle = &card.identity.oracle_id;
            let public = oracle.ends_with("-cmd") || oracle.ends_with("-companion");
            assert!(
                public || oracle.starts_with(&format!("s{viewer}-")),
                "viewer {viewer} is told {oracle}"
            );
        }
    }
}
