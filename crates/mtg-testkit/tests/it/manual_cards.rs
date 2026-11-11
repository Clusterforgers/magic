//! Goal: a card the engine does not run whole is cast and resolves without effects for the clauses it leaves to the table (a spell goes to the graveyard, a permanent enters with the abilities the engine does run), the clauses stay on the card for every seat to read, an Aura or an Equipment of that kind waits on the battlefield until its controller attaches it by hand, and every corpus card that is not Verified and has a mana cost can be cast and played this way at a table with lands, a creature, an artifact and an enchantment of its own.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::keywords::keywords_of;
use mtg_engine::manual::ManualAction;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::manual_cards::{
    manual_aura, manual_creature, manual_equipment, manual_instant, manual_sorcery,
    unverified_corpus_cards,
};
use mtg_testkit::{Driver, RandomBot, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::resolve;
use crate::common::{name_of, pid};

/// Three lands of each basic type: enough for a spell of up to three symbols of one color.
fn lands() -> Vec<PresetPermanent> {
    [forest, plains, island, swamp, mountain]
        .into_iter()
        .flat_map(|land| (0..3).map(move |_| ready(land())))
        .collect()
}

/// Seat 0 holds the cards given and lands of every color in its first main phase; seat 1 has a creature.
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

/// Starts casting the named spell, answers each choice before the payment (the face of a card with two faces, its
/// targets) with a seeded random legal choice and pays automatically.
fn cast_choosing_first(driver: &mut Driver, name: &str) -> Result<Vec<Event>, ScriptError> {
    driver.begin_cast(name)?;
    let mut bot = RandomBot::new(7, 0);
    let mut events = Vec::new();
    for _ in 0..8 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::PayMana(_)) {
            break;
        }
        events.extend(driver.answer(bot.choose(&decision))?);
    }
    events.extend(driver.answer(Answer::PayMana(PayChoice::Auto))?);
    Ok(events)
}

/// The events of a cast and its resolution.
fn cast_and_resolve(driver: &mut Driver, name: &str) -> Vec<Event> {
    let mut events = cast_choosing_first(driver, name)
        .unwrap_or_else(|error| panic!("{name} is not cast: {error:?}"));
    let before = driver.engine().events().len();
    resolve(driver).unwrap_or_else(|error| panic!("{name} does not resolve: {error:?}"));
    // A choice as it resolves or enters ("As ~ enters, choose a color") takes the passive answer.
    for _ in 0..8 {
        if matches!(
            driver.decision().map(|d| &d.kind),
            Ok(DecisionKind::Priority(_))
        ) {
            break;
        }
        driver
            .answer_passively()
            .unwrap_or_else(|error| panic!("{name} does not resolve: {error:?}"));
    }
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
        // The card plays under the name of its front face, a Room under the name of the door cast; a card with two
        // faces may leave its clauses on the back.
        let name = card.name.clone();
        let names: Vec<String> = std::iter::once(name.clone())
            .chain(card.faces.iter().map(|face| face.name.clone()))
            .collect();
        assert!(
            !card.manual.is_empty() || card.faces.iter().any(|face| !face.manual.is_empty()),
            "{name} shows no clause to the table"
        );
        let land = card.types.contains(&CardType::Land);
        // A card without a mana cost (CR 202.1b) is cast only by another rule (suspend, splice) or not at all.
        if !land
            && card.mana_cost.is_none()
            && card.faces.iter().all(|face| face.mana_cost.is_none())
        {
            continue;
        }
        let mut driver = table(vec![card]);
        // A land is played, every other card cast; what it puts on the stack (a storm copy, a door's trigger) resolves,
        // and an enters trigger may exile the card itself.
        if land {
            driver
                .play_land(&name)
                .unwrap_or_else(|error| panic!("{name} is not played: {error:?}"));
        } else {
            cast_and_resolve(&mut driver, &name);
        }
        settle(&mut driver, &name);
        for _ in 0..8 {
            if driver.state().zone(Zone::Stack).is_empty() {
                break;
            }
            resolve(&mut driver)
                .unwrap_or_else(|error| panic!("{name} does not resolve: {error:?}"));
            settle(&mut driver, &name);
        }
        let played = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .chain(driver.state().zone(Zone::Graveyard(pid(0))))
            .chain(driver.state().zone(Zone::Exile))
            .any(|id| names.contains(&name_of(driver.state(), *id)));
        assert!(played, "{name} was cast and did not resolve");
    }
}

/// Four lands of each basic type: enough for a spell of up to four symbols of one color.
fn more_lands() -> Vec<PresetPermanent> {
    [forest, plains, island, swamp, mountain]
        .into_iter()
        .flat_map(|land| (0..4).map(move |_| ready(land())))
        .collect()
}

/// Seat 0 holds the cards given, lands of every color, a creature, an artifact and an enchantment, and a legendary
/// creature card in its graveyard (something for each Aura and target to name); seat 1 has a creature.
fn table(hand: Vec<Arc<CardDefinition>>) -> Driver {
    let corpus = |name: &str| mtg_testkit::corpus::card(name).unwrap();
    let mut own = more_lands();
    own.extend(["Grizzly Bears", "Sol Ring", "Glorious Anthem"].map(|name| ready(corpus(name))));
    Driver::from_scenario(
        Scenario::new(2)
            .hand(0, hand)
            .library(0, vec![forest(); 8])
            .library(1, vec![forest(); 8])
            .graveyard(0, [corpus("Kongming, \"Sleeping Dragon\"")])
            .battlefield(0, own)
            .battlefield(1, [ready(free_creature("Theirs", 3, 3))]),
    )
    .unwrap()
}

/// Answers the choices a permanent asks for as it enters ("As this land enters, choose a basic land type") passively.
fn settle(driver: &mut Driver, name: &str) {
    for _ in 0..8 {
        if matches!(
            driver.decision().map(|d| &d.kind),
            Ok(DecisionKind::Priority(_))
        ) {
            break;
        }
        driver
            .answer_passively()
            .unwrap_or_else(|error| panic!("{name} does not settle: {error:?}"));
    }
}
