//! Goal: a card the engine does not run whole is cast and resolves without effects for the clauses it leaves to the table (a spell goes to the graveyard, a permanent enters with the abilities the engine does run), the clauses stay on the card for every seat to read, an Aura or an Equipment of that kind waits on the battlefield until its controller attaches it by hand, and every corpus card that is not Verified can be cast and played this way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::keywords::keywords_of;
use mtg_engine::manual::ManualAction;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::{forest, free_creature, island, plains, swamp};
use mtg_testkit::manual_cards::{
    manual_aura, manual_creature, manual_equipment, manual_instant, manual_sorcery,
    unverified_corpus_cards,
};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::casting::resolve;
use crate::common::{name_of, pid};

fn lands() -> Vec<PresetPermanent> {
    vec![
        ready(forest()),
        ready(forest()),
        ready(plains()),
        ready(plains()),
        ready(island()),
        ready(island()),
        ready(swamp()),
    ]
}

/// Seat 0 holds the cards given and six lands in its first main phase; seat 1 has a creature.
fn game(hand: Vec<Arc<CardDefinition>>) -> Driver {
    Driver::from_scenario(
        Scenario::new(2)
            .hand(0, hand)
            .library(0, vec![forest(); 8])
            .library(1, vec![forest(); 8])
            .battlefield(0, lands())
            .battlefield(1, [ready(free_creature("Theirs", 3, 3))]),
    )
    .unwrap()
}

fn find(driver: &Driver, zone: Zone, name: &str) -> ObjectId {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .find(|id| name_of(driver.state(), *id) == name)
        .unwrap_or_else(|| panic!("{name} is not in {zone:?}"))
}

fn zone_of(driver: &Driver, id: ObjectId) -> Zone {
    driver.state().object(id).unwrap().zone()
}

/// The events of a cast and its resolution.
fn cast_and_resolve(driver: &mut Driver, name: &str) -> Vec<Event> {
    let mut events = driver.cast(name).unwrap();
    let before = driver.engine().events().len();
    resolve(driver).unwrap();
    events.extend_from_slice(&driver.engine().events()[before..]);
    events
}

#[rstest]
#[case::a_sorcery(manual_sorcery("Wave"))]
#[case::an_instant(manual_instant("Zap"))]
fn a_spell_the_engine_does_not_run_goes_to_the_graveyard_doing_nothing(
    #[case] card: Arc<CardDefinition>,
) {
    let name = card.name.clone();
    let mut driver = game(vec![card]);
    let life = [
        driver.state().player(pid(0)).unwrap().life(),
        driver.state().player(pid(1)).unwrap().life(),
    ];
    let events = cast_and_resolve(&mut driver, &name);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::SpellResolved { .. })),
        "{events:?}"
    );
    let graveyard = driver.state().zone(Zone::Graveyard(pid(0)));
    assert_eq!(graveyard.len(), 1, "the spell is in its owner's graveyard");
    assert_eq!(name_of(driver.state(), graveyard[0]), name);
    assert_eq!(
        [
            driver.state().player(pid(0)).unwrap().life(),
            driver.state().player(pid(1)).unwrap().life()
        ],
        life
    );
    assert_eq!(
        driver.state().zone(Zone::Battlefield).len(),
        lands().len() + 1
    );
}

#[test]
fn a_permanent_the_engine_does_not_run_enters_with_the_abilities_it_does() {
    let mut driver = game(vec![manual_creature("Drake")]);
    cast_and_resolve(&mut driver, "Drake");
    let drake = find(&driver, Zone::Battlefield, "Drake");
    assert!(
        keywords_of(driver.state(), drake)
            .iter()
            .any(|keyword| **keyword == Keyword::Flying)
    );
    let clauses = &driver.state().object(drake).unwrap().card().manual;
    assert_eq!(clauses.len(), 1);
    assert_eq!(
        clauses[0].text,
        "Whenever this creature dies, each opponent loses 1 life."
    );
}

#[test]
fn an_aura_the_engine_does_not_run_waits_on_the_battlefield_until_it_is_attached_by_hand() {
    let mut driver = game(vec![manual_aura("Charm")]);
    cast_and_resolve(&mut driver, "Charm");
    let charm = find(&driver, Zone::Battlefield, "Charm");
    assert_eq!(driver.state().host_of(charm), None);
    assert_eq!(zone_of(&driver, charm), Zone::Battlefield);
    let theirs = find(&driver, Zone::Battlefield, "Theirs");
    driver
        .manual(ManualAction::Attach {
            object: charm,
            to: theirs,
        })
        .unwrap();
    assert_eq!(driver.state().host_of(charm), Some(theirs));
    assert_eq!(zone_of(&driver, charm), Zone::Battlefield);
}

#[test]
fn an_equipment_the_engine_does_not_run_is_attached_by_hand() {
    let mut driver = game(vec![manual_equipment("Blade")]);
    cast_and_resolve(&mut driver, "Blade");
    let blade = find(&driver, Zone::Battlefield, "Blade");
    let theirs = find(&driver, Zone::Battlefield, "Theirs");
    driver
        .manual(ManualAction::Attach {
            object: blade,
            to: theirs,
        })
        .unwrap();
    assert_eq!(driver.state().host_of(blade), Some(theirs));
}

#[test]
fn every_corpus_card_that_is_not_verified_is_cast_and_resolves_with_its_clauses_left_to_the_table()
{
    let names = unverified_corpus_cards().unwrap();
    assert!(
        !names.is_empty(),
        "the corpus keeps a card the engine does not play whole"
    );
    for full_name in names {
        let card = mtg_testkit::corpus::card(&full_name).unwrap();
        // The card plays under the name of its front face.
        let name = card.name.clone();
        assert!(
            !card.manual.is_empty(),
            "{name} shows no clause to the table"
        );
        let mut driver = game(vec![card]);
        cast_and_resolve(&mut driver, &name);
        let played = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .chain(driver.state().zone(Zone::Graveyard(pid(0))))
            .any(|id| name_of(driver.state(), *id) == name);
        assert!(played, "{name} was cast and did not resolve");
    }
}
