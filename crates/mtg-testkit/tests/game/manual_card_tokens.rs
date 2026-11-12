//! Goal: the tokens a seat's cards in sight make (never one in its library) (predefined, described, fixed-size; never a copy or a counted size) are offered to that seat alone, by card name and place, and made by hand with every ability of the token: activated, triggered and keyword ones; a card the seat does not own or a place the card has not is refused and changes nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::manual::{ManualAction, ManualRefusal, ManualToken};
use mtg_engine::manual_tokens::card_tokens;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, characteristics};
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::pid;

/// What a token's abilities are, coarsely.
#[derive(Debug, PartialEq, Eq)]
enum Has {
    Activated,
    Triggered,
    Keyword,
    Nothing,
}

/// A token offered: its name, its power and toughness, what its abilities are.
type Offered = (&'static str, Option<(i32, i32)>, Has);

fn has(abilities: &[Ability]) -> Has {
    if abilities
        .iter()
        .any(|a| matches!(a, Ability::Activated { .. }))
    {
        Has::Activated
    } else if abilities
        .iter()
        .any(|a| matches!(a, Ability::Triggered { .. }))
    {
        Has::Triggered
    } else if abilities.iter().any(|a| matches!(a, Ability::Keyword(_))) {
        Has::Keyword
    } else {
        Has::Nothing
    }
}

/// Seat 0 holds `name` in its hand and Descendant of Storms in its library, seat 1 an Angelic Sleuth in hand.
fn driver(name: &str) -> Driver {
    let scenario = Scenario::new(2)
        .hand(0, [card(name).unwrap()])
        .hand(1, [card("Angelic Sleuth").unwrap()])
        .library(
            0,
            [card("Descendant of Storms").unwrap()]
                .into_iter()
                .chain((0..10).map(|_| island()))
                .collect::<Vec<_>>(),
        )
        .library(1, (0..10).map(|_| island()).collect::<Vec<_>>());
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
#[case::a_predefined_clue("Angelic Sleuth", &[("Clue", None, Has::Activated)])]
#[case::a_described_scion("Abstruse Interference", &[("Eldrazi Scion", Some((1, 1)), Has::Activated)])]
#[case::a_described_keyword_token("Baron Bertram Graywater", &[("Vampire Rogue", Some((1, 1)), Has::Keyword)])]
#[case::a_fixed_size_is_that_size("Descendant of Storms", &[("Spirit", Some((1, 1)), Has::Nothing)])]
#[case::a_copy_is_left_out("Blue Sun's Twilight", &[])]
#[case::a_counted_size_is_left_out("Assemble the Entmoot", &[])]
fn each_card_offers_the_tokens_it_makes_and_the_token_is_made_with_its_abilities(
    #[case] name: &str,
    #[case] expected: &[Offered],
) {
    let mut driver = driver(name);
    let offers = card_tokens(driver.state(), pid(0));
    let seen: Vec<_> = offers
        .iter()
        .map(|offer| {
            (
                offer.token.name.as_str(),
                offer.token.power.zip(offer.token.toughness),
                has(&offer.token.abilities),
            )
        })
        .collect();
    assert_eq!(seen.len(), expected.len(), "{seen:?}");
    for (seen, expected) in seen.iter().zip(expected) {
        assert_eq!((seen.0, seen.1), (expected.0, expected.1));
        assert_eq!(seen.2, expected.2);
    }
    assert!(offers.iter().all(|offer| offer.card == name));
    for offer in &offers {
        driver
            .manual(ManualAction::CreateToken {
                token: ManualToken::Card {
                    card: offer.card.clone(),
                    index: offer.index,
                },
                count: 2,
            })
            .unwrap();
        let made: Vec<_> = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .filter(|id| {
                let object = driver.state().object(*id).unwrap();
                object.kind() == ObjectKind::Token && object.card().name == offer.token.name
            })
            .collect();
        assert_eq!(made.len(), 2);
        for id in made {
            let object = driver.state().object(id).unwrap();
            assert_eq!(object.controller(), pid(0));
            let rules: Vec<Ability> = object.card().rules.iter().map(|r| (**r).clone()).collect();
            assert_eq!(has(&rules), has(&offer.token.abilities));
            let chars = characteristics(driver.state(), id).unwrap();
            assert_eq!(
                chars.power.zip(chars.toughness),
                offer.token.power.zip(offer.token.toughness)
            );
        }
    }
}

#[rstest]
#[case::an_opponents_card("Angelic Sleuth", 0)]
#[case::a_place_the_card_has_not("Abstruse Interference", 1)]
#[case::a_card_nobody_owns("Baron Bertram Graywater", 0)]
#[case::a_card_in_the_library("Descendant of Storms", 0)]
fn a_token_of_a_card_the_seat_does_not_own_or_a_place_it_has_not_is_refused(
    #[case] card: &str,
    #[case] index: u32,
) {
    let mut driver = driver("Abstruse Interference");
    assert!(
        card_tokens(driver.state(), pid(0))
            .iter()
            .all(|offer| offer.card == "Abstruse Interference"),
        "the opponent's Angelic Sleuth is not offered"
    );
    let (log, hash) = (driver.engine().events().len(), driver.state().state_hash());
    let error = driver
        .manual(ManualAction::CreateToken {
            token: ManualToken::Card {
                card: card.to_owned(),
                index,
            },
            count: 1,
        })
        .unwrap_err();
    let ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Manual(refusal))) = error else {
        panic!("not a manual refusal: {error:?}");
    };
    assert!(matches!(refusal, ManualRefusal::BadToken(_)));
    assert_eq!(driver.engine().events().len(), log);
    assert_eq!(driver.state().state_hash(), hash);
}
