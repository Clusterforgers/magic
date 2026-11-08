//! Goal: a face-down exile a player may look at, and "discard any number of cards", translate to the engine and play: the cards lie face down in exile linked to the source, the discard takes as many as the player picks, and a shape the engine cannot say stays a gap named after the key.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::{abilities, gaps};

fn sorcery(line: &str) -> String {
    format!("Name:Subject\nManaCost:R\nTypes:Sorcery\n{line}\n")
}

fn card(text: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Subject")
        .with_type(CardType::Sorcery)
        .with_mana_cost(cost(0, &[Color::Red]));
    for value in abilities(text) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn cast(text: &str, hand: usize) -> Driver {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .library(0, vec![forest(); 12])
            .library(1, vec![forest(); 12])
            .hand(
                0,
                std::iter::once(card(text)).chain((0..hand).map(|_| mountain())),
            )
            .battlefield(0, vec![ready(mountain())]),
    )
    .unwrap();
    driver.cast_tapping("Subject", &["Mountain"]).unwrap();
    driver
}

fn settle(driver: &mut Driver) {
    for _ in 0..40 {
        if matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
        {
            return;
        }
        let decision = driver.decision().unwrap().clone();
        driver.answer(Driver::passive_answer(&decision)).unwrap();
    }
}

const DIG_ALL: &str = "A:SP$ Dig | DigNum$ 3 | ChangeNum$ All | DestinationZone$ Exile | ExileFaceDown$ True | WithMayLook$ True | RememberChanged$ True";
const DIG_ONE: &str = "A:SP$ Dig | DigNum$ 4 | ChangeNum$ 1 | DestinationZone$ Exile | ExileFaceDown$ True | WithMayLook$ True";

#[rstest]
#[case::all_of_them(DIG_ALL, 3, 9)]
#[case::one_of_four(DIG_ONE, 1, 11)]
fn the_cards_taken_lie_face_down_in_exile(
    #[case] line: &str,
    #[case] exiled: usize,
    #[case] library: usize,
) {
    let text = sorcery(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    let mut driver = cast(&text, 0);
    settle(&mut driver);
    let exile = driver.state().zone(Zone::Exile).to_vec();
    assert_eq!(exile.len(), exiled);
    for id in exile {
        assert!(
            driver.state().object(id).unwrap().face_down().is_some(),
            "{id:?} lies face up"
        );
    }
    let left = driver
        .state()
        .zone(Zone::Library(PlayerId::from_index(0)))
        .len();
    assert_eq!(left, library);
}

#[rstest]
#[case::revealed_face_down(
    "A:SP$ Dig | DigNum$ 3 | ChangeNum$ All | DestinationZone$ Exile | ExileFaceDown$ True | Reveal$ True",
    "key:Dig.ExileFaceDown"
)]
#[case::rest_face_down(
    "A:SP$ Dig | DigNum$ 3 | ChangeNum$ 1 | DestinationZone$ Exile | DestinationZone2$ Exile | ExileFaceDown$ True | WithMayLook$ True",
    "key:Dig.ExileFaceDown-destination"
)]
#[case::to_hand(
    "A:SP$ Dig | DigNum$ 3 | ChangeNum$ 1 | ExileFaceDown$ True | WithMayLook$ True",
    "key:Dig.ExileFaceDown-destination"
)]
#[case::look_alone(
    "A:SP$ Dig | DigNum$ 3 | ChangeNum$ 1 | WithMayLook$ True",
    "key:Dig.WithMayLook"
)]
fn a_face_down_shape_the_engine_cannot_say_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    let found = gaps(&sorcery(line));
    assert!(found.iter().any(|found| found == gap), "{found:?}");
}

#[rstest]
#[case::two_of_three(3, 2, 1)]
#[case::none(3, 0, 3)]
#[case::all(3, 3, 0)]
fn any_number_of_cards_are_discarded(
    #[case] hand: usize,
    #[case] take: usize,
    #[case] left: usize,
) {
    let text = sorcery(
        "A:SP$ Discard | Defined$ You | AnyNumber$ True | Optional$ True | Mode$ TgtChoose",
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let mut driver = cast(&text, hand);
    for _ in 0..10 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseCards(options) => {
                assert_eq!(options.max as usize, hand);
                let picked = options.from.iter().copied().take(take).collect();
                driver
                    .answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(picked)))
                    .unwrap();
                break;
            }
            _ => {
                driver.answer(Driver::passive_answer(&decision)).unwrap();
            }
        }
    }
    settle(&mut driver);
    let in_hand = driver
        .state()
        .zone(Zone::Hand(PlayerId::from_index(0)))
        .len();
    assert_eq!(in_hand, left);
}

#[rstest]
#[case::exactly_n(
    "A:SP$ Discard | Defined$ You | AnyNumber$ True | NumCards$ 2 | Optional$ True",
    "key:Discard.AnyNumber"
)]
#[case::not_optional(
    "A:SP$ Discard | Defined$ You | AnyNumber$ True",
    "key:Discard.AnyNumber"
)]
fn a_discard_of_any_number_needs_the_optional_choice(#[case] line: &str, #[case] gap: &str) {
    let found = gaps(&sorcery(line));
    assert!(found.iter().any(|found| found == gap), "{found:?}");
}
