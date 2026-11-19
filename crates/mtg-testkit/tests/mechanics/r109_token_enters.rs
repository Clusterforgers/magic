//! Goal: an effect over how a permanent enters applies to each token as it is created (CR 111.1, 614.1c, 614.12), a token's own as well (an embalmed Vizier of Many Faces enters as a copy that stays a white Zombie, a cast one does not): tokens a player creates enter with the counters of "creatures you control enter with" (after a token doubler, and counted up by Hardened Scales), a "nontoken" clause, a token of another player and a token of another type get none, "as a historic permanent you control enters, it becomes" turns a Treasure into a 7/7 Dinosaur, at tables of two to four seats, and each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::object::ObjectKind;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::{pid, spell};

const SOLDIER: &str = r#"{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}}"#;

/// A free sorcery: each player creates two 1/1 Soldiers, and its caster a Treasure.
fn r109_muster() -> Arc<CardDefinition> {
    spell(
        "Muster",
        CardType::Sorcery,
        "[]",
        &format!(
            r#"[{{"CreateToken":{{"who":"EachPlayer","token":{SOLDIER},"count":2}}}},{{"CreateToken":{{"token":{{"Named":"treasure"}}}}}}]"#
        ),
    )
}

/// An enchantment with one replacement over the entering of what `what` selects.
fn r109_enchantment(name: &str, what: &str, by: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":{what}}}}},"by":{by}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_rules(rules),
    )
}

/// "Each creature you control enters with two additional +1/+1 counters on it", with the selector `what`.
fn r109_brood(what: &str) -> Arc<CardDefinition> {
    r109_enchantment(
        "Brood",
        what,
        r#"{"WithCounters":{"counter":"+1/+1","count":2}}"#,
    )
}

/// The +1/+1 counters on each token named `name` that `seat` controls.
fn r109_counters(driver: &Driver, seat: usize, name: &str) -> Vec<u32> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| {
            o.kind() == ObjectKind::Token && o.controller() == pid(seat) && o.card().name == name
        })
        .map(|o| {
            o.permanent()
                .unwrap()
                .counters()
                .filter(|(kind, _)| **kind == CounterKind::PlusOnePlusOne)
                .map(|(_, n)| n)
                .sum()
        })
        .collect()
}

/// Seat 0 controls `permanents` and casts Muster; the stack is then resolved.
fn r109_play(seats: usize, permanents: Vec<Arc<CardDefinition>>) -> (Scenario, Driver) {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, permanents.into_iter().map(ready))
        .hand(0, [r109_muster()]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| mtg_testkit::cards::plains()));
    }
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("Muster").unwrap();
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        driver.answer_passively().unwrap();
    }
    (scenario, driver)
}

fn r109_replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
#[case::each_creature(r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#, false, false, 2)]
#[case::nontoken_only(
    r#"[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"}]"#,
    false,
    false,
    0
)]
#[case::doubled_tokens_each_get_them(
    r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
    true,
    false,
    2
)]
#[case::scales_add_one(r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#, false, true, 3)]
fn tokens_enter_with_the_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] what: &str,
    #[case] doubled: bool,
    #[case] scales: bool,
    #[case] each: u32,
) {
    let mut permanents = vec![r109_brood(what)];
    if doubled {
        permanents.push(card("Anointed Procession").unwrap());
    }
    if scales {
        permanents.push(card("Hardened Scales").unwrap());
    }
    let (scenario, driver) = r109_play(seats, permanents);
    let made = if doubled { 4 } else { 2 };
    assert_eq!(r109_counters(&driver, 0, "Soldier"), vec![each; made]);
    // The Treasure is no creature, and the opponents' Soldiers are not the Brood controller's.
    assert_eq!(r109_counters(&driver, 0, "Treasure"), vec![0; made / 2]);
    for seat in 1..seats {
        assert_eq!(r109_counters(&driver, seat, "Soldier"), vec![0, 0]);
    }
    r109_replays(&scenario, &driver);
}

#[rstest]
fn a_historic_token_becomes_a_dinosaur(#[values(2, 3, 4)] seats: usize) {
    let dinosaurs = r109_enchantment(
        "Displacement",
        r#"[{"AnyOf":[{"Type":"Artifact"},{"Supertype":"Legendary"},{"Subtype":"Saga"}]},{"ControlledBy":"You"}]"#,
        r#"{"BecomesChoice":[[{"AddType":"Creature"},{"AddSubtype":"Dinosaur"},{"SetPT":{"power":7,"toughness":7}}]]}"#,
    );
    let (scenario, driver) = r109_play(seats, vec![dinosaurs]);
    let state = driver.state();
    let treasure = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == "Treasure")
        .unwrap();
    let chars = characteristics(state, treasure).unwrap();
    assert!(chars.types.contains(CardType::Artifact) && chars.is_creature());
    assert_eq!((chars.power, chars.toughness), (Some(7), Some(7)));
    // A Soldier is not historic.
    let soldier = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| state.object(*id).unwrap().card().name == "Soldier")
        .unwrap();
    assert_eq!(characteristics(state, soldier).unwrap().power, Some(1));
    r109_replays(&scenario, &driver);
}

/// Seat 0 controls Grizzly Bears and five Islands and holds Vizier of Many Faces in its hand, or in its graveyard when
/// `embalmed`; the Vizier is cast or embalmed, enters as a copy of the Bears, and the stack is resolved.
fn r109_vizier(seats: usize, embalmed: bool) -> (Scenario, Driver) {
    let mut board = vec![ready(card("Grizzly Bears").unwrap())];
    board.extend((0..5).map(|_| ready(mtg_testkit::cards::island())));
    let vizier = card("Vizier of Many Faces").unwrap();
    let mut scenario = Scenario::new(seats).battlefield(0, board);
    scenario = if embalmed {
        scenario.graveyard(0, [vizier])
    } else {
        scenario.hand(0, [vizier])
    };
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| mtg_testkit::cards::plains()));
    }
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    if embalmed {
        driver.begin_activation("Vizier of Many Faces").unwrap();
    } else {
        driver.begin_cast("Vizier of Many Faces").unwrap();
    }
    let bears = driver.id_of("Grizzly Bears").unwrap();
    for _ in 0..40 {
        match driver.decision().unwrap().kind.clone() {
            DecisionKind::ChooseCards(options) if options.from.contains(&bears) => {
                driver
                    .answer(Answer::ChooseCards(CardsChoice(vec![bears])))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                break;
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    (scenario, driver)
}

#[rstest]
#[case::cast_from_hand(false)]
#[case::embalmed_token(true)]
fn vizier_enters_as_a_copy_and_an_embalmed_token_stays_a_white_zombie(
    #[values(2, 3, 4)] seats: usize,
    #[case] embalmed: bool,
) {
    let (scenario, driver) = r109_vizier(seats, embalmed);
    let state = driver.state();
    let copies: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| state.object(*id).unwrap().card().name == "Grizzly Bears")
        .collect();
    assert_eq!(copies.len(), 2, "the Vizier is a Grizzly Bears");
    let copy = copies[1];
    assert_eq!(
        state.object(copy).unwrap().kind() == ObjectKind::Token,
        embalmed
    );
    let chars = characteristics(state, copy).unwrap();
    let printed = &state.object(copy).unwrap().card().subtypes;
    let zombie = chars.has_subtype(printed, &mtg_core::card::Subtype("Zombie".to_owned()));
    assert_eq!(zombie, embalmed);
    assert_eq!(
        chars.colors.contains(mtg_core::color::Color::White),
        embalmed
    );
    r109_replays(&scenario, &driver);
}
