//! Goal: the seat that has priority applies every kind of manual action to what it owns or controls, to any permanent for counters, tapping and untapping, and to its own life and counters, each one logged first by an event that names the seat and the action and followed by the events of the change; an action that is not allowed (an opponent's card or life, a card in a library or on the stack, a zone the game lacks, a number out of range, counters that are not there) is refused by name and leaves the log, the state and the question as they were; the seat keeps priority, the round starts again, and a rebuilt engine reaches the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::error::IllegalAnswer;
use mtg_engine::manual::{
    CommonToken, CustomToken, ManualAction, ManualRefusal, ManualToken, ManualZone,
};
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, EngineError, Event, characteristics};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::manual_cards::{manual_aura, manual_equipment, manual_instant, manual_sorcery};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped, with_counters};
use rstest::rstest;

use crate::common::{name_of, pid};

/// The objects the tests name.
struct Ids {
    hand_creature: ObjectId,
    zap: ObjectId,
    wave: ObjectId,
    mine: ObjectId,
    counted: ObjectId,
    tapper: ObjectId,
    charm: ObjectId,
    blade: ObjectId,
    ghoul: ObjectId,
    top: ObjectId,
    theirs: ObjectId,
    foe_card: ObjectId,
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

/// Seat 0 holds priority in its first main phase with a card of each kind around it; seat 1 has a creature and a card.
fn game() -> (Driver, Ids) {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(
                0,
                [
                    free_creature("Hand creature", 2, 2),
                    manual_instant("Zap"),
                    manual_sorcery("Wave"),
                ],
            )
            .library(
                0,
                [
                    free_creature("Top", 1, 1),
                    free_creature("Next", 1, 1),
                    free_creature("Third", 1, 1),
                    forest(),
                    forest(),
                ],
            )
            .battlefield(
                0,
                [
                    ready(free_creature("Mine", 2, 2)),
                    with_counters(
                        ready(free_creature("Counted", 2, 2)),
                        CounterKind::Charge,
                        3,
                    ),
                    tapped(free_creature("Tapper", 1, 1)),
                    ready(manual_aura("Charm")),
                    ready(manual_equipment("Blade")),
                    ready(forest()),
                ],
            )
            .graveyard(0, [free_creature("Ghoul", 1, 1)])
            .hand(1, [free_creature("Foe card", 1, 1)])
            .library(1, vec![forest(); 5])
            .battlefield(1, [ready(free_creature("Theirs", 3, 3))]),
    )
    .unwrap();
    let ids = Ids {
        hand_creature: find(&driver, Zone::Hand(pid(0)), "Hand creature"),
        zap: find(&driver, Zone::Hand(pid(0)), "Zap"),
        wave: find(&driver, Zone::Hand(pid(0)), "Wave"),
        mine: find(&driver, Zone::Battlefield, "Mine"),
        counted: find(&driver, Zone::Battlefield, "Counted"),
        tapper: find(&driver, Zone::Battlefield, "Tapper"),
        charm: find(&driver, Zone::Battlefield, "Charm"),
        blade: find(&driver, Zone::Battlefield, "Blade"),
        ghoul: find(&driver, Zone::Graveyard(pid(0)), "Ghoul"),
        top: find(&driver, Zone::Library(pid(0)), "Top"),
        theirs: find(&driver, Zone::Battlefield, "Theirs"),
        foe_card: find(&driver, Zone::Hand(pid(1)), "Foe card"),
    };
    (driver, ids)
}

/// The zone of an object.
fn zone(driver: &Driver, id: ObjectId) -> Zone {
    driver.state().object(id).unwrap().zone()
}

/// How many counters of `kind` the permanent holds.
fn counters(driver: &Driver, id: ObjectId, kind: &CounterKind) -> u32 {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(kind)
}

fn tapped_now(driver: &Driver, id: ObjectId) -> bool {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

fn me(driver: &Driver) -> &mtg_engine::player::Player {
    driver.state().player(pid(0)).unwrap()
}

const fn mv(object: ObjectId, to: ManualZone) -> ManualAction {
    ManualAction::Move { object, to }
}

/// Takes the action as seat 0 and checks what every action does: it is logged first, the seat keeps priority, and a
/// rebuilt engine is in the same state.
fn take(driver: &mut Driver, action: ManualAction) {
    let expected = action.clone();
    let events = driver.manual(action).unwrap();
    assert!(
        matches!(
            events.first(),
            Some(Event::ManualAction { player, action: logged })
                if *player == pid(0) && **logged == expected
        ),
        "{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::PriorityPassed { .. }))
    );
    let decision = driver.decision().unwrap();
    assert_eq!(decision.player, pid(0));
    assert!(matches!(decision.kind, DecisionKind::Priority(_)));
    let rebuilt = Engine::resume(
        driver.engine().setup().clone(),
        driver.engine().events().to_vec(),
    )
    .unwrap();
    assert_eq!(rebuilt.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::hand_to_the_graveyard(|i: &Ids| mv(i.hand_creature, ManualZone::Graveyard),
    |d: &Driver, i: &Ids| assert_eq!(zone(d, i.hand_creature), Zone::Graveyard(pid(0))))]
#[case::hand_to_the_battlefield(|i: &Ids| mv(i.hand_creature, ManualZone::Battlefield),
    |d: &Driver, i: &Ids| assert_eq!(zone(d, i.hand_creature), Zone::Battlefield))]
#[case::hand_to_exile(|i: &Ids| mv(i.hand_creature, ManualZone::Exile),
    |d: &Driver, i: &Ids| assert_eq!(zone(d, i.hand_creature), Zone::Exile))]
#[case::hand_to_the_top_of_the_library(|i: &Ids| mv(i.hand_creature, ManualZone::LibraryTop),
    |d: &Driver, i: &Ids| assert_eq!(d.state().zone(Zone::Library(pid(0))).last(), Some(&i.hand_creature)))]
#[case::hand_to_the_bottom_of_the_library(|i: &Ids| mv(i.hand_creature, ManualZone::LibraryBottom),
    |d: &Driver, i: &Ids| assert_eq!(d.state().zone(Zone::Library(pid(0))).first(), Some(&i.hand_creature)))]
#[case::the_battlefield_to_the_hand(|i: &Ids| mv(i.mine, ManualZone::Hand),
    |d: &Driver, i: &Ids| assert_eq!(zone(d, i.mine), Zone::Hand(pid(0))))]
#[case::the_battlefield_to_the_graveyard(|i: &Ids| mv(i.mine, ManualZone::Graveyard),
    |d: &Driver, i: &Ids| assert_eq!(zone(d, i.mine), Zone::Graveyard(pid(0))))]
#[case::the_battlefield_to_exile(|i: &Ids| mv(i.mine, ManualZone::Exile),
    |d: &Driver, i: &Ids| assert_eq!(zone(d, i.mine), Zone::Exile))]
#[case::the_graveyard_to_the_battlefield(|i: &Ids| mv(i.ghoul, ManualZone::Battlefield),
    |d: &Driver, i: &Ids| assert_eq!(zone(d, i.ghoul), Zone::Battlefield))]
#[case::the_graveyard_to_the_hand(|i: &Ids| mv(i.ghoul, ManualZone::Hand),
    |d: &Driver, i: &Ids| assert_eq!(zone(d, i.ghoul), Zone::Hand(pid(0))))]
#[case::the_top_two_cards_to_the_graveyard(
    |_: &Ids| ManualAction::MoveTop { count: 2, to: ManualZone::Graveyard },
    |d: &Driver, i: &Ids| {
        assert_eq!(zone(d, i.top), Zone::Graveyard(pid(0)));
        assert_eq!(d.state().zone(Zone::Library(pid(0))).len(), 3);
    })]
#[case::the_top_card_to_exile(
    |_: &Ids| ManualAction::MoveTop { count: 1, to: ManualZone::Exile },
    |d: &Driver, i: &Ids| assert_eq!(zone(d, i.top), Zone::Exile))]
#[case::the_top_card_to_the_hand(
    |_: &Ids| ManualAction::MoveTop { count: 1, to: ManualZone::Hand },
    |d: &Driver, i: &Ids| assert_eq!(zone(d, i.top), Zone::Hand(pid(0))))]
#[case::the_top_two_cards_to_the_bottom_in_order(
    |_: &Ids| ManualAction::MoveTop { count: 2, to: ManualZone::LibraryBottom },
    |d: &Driver, i: &Ids| {
        let library = d.state().zone(Zone::Library(pid(0)));
        assert_eq!(library.len(), 5);
        assert_eq!(library[1], i.top, "the first of the two is above the second");
        assert_eq!(name_of(d.state(), library[0]), "Next");
        assert_eq!(name_of(d.state(), library[4]), "Third");
    })]
#[case::counters_on_a_permanent(
    |i: &Ids| ManualAction::Counters { object: i.mine, counter: CounterKind::PlusOnePlusOne, delta: 2 },
    |d: &Driver, i: &Ids| {
        assert_eq!(counters(d, i.mine, &CounterKind::PlusOnePlusOne), 2);
        assert_eq!(characteristics(d.state(), i.mine).unwrap().power, Some(4));
    })]
#[case::counters_taken_off(
    |i: &Ids| ManualAction::Counters { object: i.counted, counter: CounterKind::Charge, delta: -2 },
    |d: &Driver, i: &Ids| assert_eq!(counters(d, i.counted, &CounterKind::Charge), 1))]
#[case::counters_on_an_opponents_permanent(
    |i: &Ids| ManualAction::Counters { object: i.theirs, counter: CounterKind::Other("quest".into()), delta: 1 },
    |d: &Driver, i: &Ids| assert_eq!(counters(d, i.theirs, &CounterKind::Other("quest".into())), 1))]
#[case::poison_on_the_seat(
    |_: &Ids| ManualAction::PlayerCounters { player: pid(0), counter: CounterKind::Poison, delta: 2 },
    |d: &Driver, _: &Ids| assert_eq!(me(d).counters_of(&CounterKind::Poison), 2))]
#[case::energy_on_the_seat(
    |_: &Ids| ManualAction::PlayerCounters { player: pid(0), counter: CounterKind::Other("energy".into()), delta: 3 },
    |d: &Driver, _: &Ids| assert_eq!(me(d).counters_of(&CounterKind::Other("energy".into())), 3))]
#[case::life_gained(
    |_: &Ids| ManualAction::Life { player: pid(0), delta: 3 },
    |d: &Driver, _: &Ids| assert_eq!(me(d).life(), 23))]
#[case::life_lost(
    |_: &Ids| ManualAction::Life { player: pid(0), delta: -4 },
    |d: &Driver, _: &Ids| assert_eq!(me(d).life(), 16))]
#[case::two_treasures(
    |_: &Ids| ManualAction::CreateToken { token: ManualToken::Common(CommonToken::Treasure), count: 2 },
    |d: &Driver, _: &Ids| {
        let made = tokens_named(d, "Treasure");
        assert_eq!(made.len(), 2);
        assert!(made.iter().all(|id| d.state().object(*id).unwrap().controller() == pid(0)));
    })]
#[case::a_soldier(
    |_: &Ids| ManualAction::CreateToken { token: ManualToken::Common(CommonToken::Soldier), count: 1 },
    |d: &Driver, _: &Ids| {
        let [soldier] = tokens_named(d, "Soldier")[..] else { panic!("one Soldier") };
        let chars = characteristics(d.state(), soldier).unwrap();
        assert_eq!((chars.power, chars.toughness), (Some(1), Some(1)));
        assert!(chars.colors.contains(Color::White));
    })]
#[case::a_custom_token(
    |_: &Ids| ManualAction::CreateToken {
        token: ManualToken::Custom(Box::new(CustomToken {
            name: "Imp".into(),
            colors: vec![Color::Black],
            types: vec![CardType::Creature],
            subtypes: vec!["Imp".into()],
            power: Some(2),
            toughness: Some(3),
        })),
        count: 1,
    },
    |d: &Driver, _: &Ids| {
        let [imp] = tokens_named(d, "Imp")[..] else { panic!("one Imp") };
        let chars = characteristics(d.state(), imp).unwrap();
        assert_eq!((chars.power, chars.toughness), (Some(2), Some(3)));
        assert!(chars.colors.contains(Color::Black));
        assert!(chars.is_creature());
    })]
#[case::tapping(|i: &Ids| ManualAction::Tap { object: i.mine },
    |d: &Driver, i: &Ids| assert!(tapped_now(d, i.mine)))]
#[case::tapping_an_opponents_permanent(|i: &Ids| ManualAction::Tap { object: i.theirs },
    |d: &Driver, i: &Ids| assert!(tapped_now(d, i.theirs)))]
#[case::untapping(|i: &Ids| ManualAction::Untap { object: i.tapper },
    |d: &Driver, i: &Ids| assert!(!tapped_now(d, i.tapper)))]
#[case::drawing_two(|_: &Ids| ManualAction::Draw { count: 2 },
    |d: &Driver, i: &Ids| {
        assert_eq!(zone(d, i.top), Zone::Hand(pid(0)));
        assert_eq!(d.state().zone(Zone::Hand(pid(0))).len(), 5);
    })]
#[case::looking_at_the_top_two(|_: &Ids| ManualAction::LookTop { count: 2 },
    |d: &Driver, i: &Ids| {
        assert!(d.state().is_shown_to(pid(0), i.top));
        assert!(!d.state().is_shown_to(pid(1), i.top));
        assert_eq!(mtg_engine::sight::known_top(d.state(), pid(0)).len(), 2);
    })]
#[case::looking_at_more_than_the_library_holds(|_: &Ids| ManualAction::LookTop { count: 10 },
    |d: &Driver, _: &Ids| assert_eq!(mtg_engine::sight::known_top(d.state(), pid(0)).len(), 5))]
#[case::shuffling(|_: &Ids| ManualAction::Shuffle {},
    |d: &Driver, _: &Ids| assert_eq!(d.state().zone(Zone::Library(pid(0))).len(), 5))]
#[case::discarding(|i: &Ids| ManualAction::Discard { object: i.hand_creature },
    |d: &Driver, i: &Ids| assert_eq!(zone(d, i.hand_creature), Zone::Graveyard(pid(0))))]
#[case::attaching(|i: &Ids| ManualAction::Attach { object: i.blade, to: i.mine },
    |d: &Driver, i: &Ids| assert_eq!(d.state().host_of(i.blade), Some(i.mine)))]
#[case::attaching_an_aura_the_engine_cannot_aim(|i: &Ids| ManualAction::Attach { object: i.charm, to: i.theirs },
    |d: &Driver, i: &Ids| assert_eq!(d.state().host_of(i.charm), Some(i.theirs)))]
fn an_action_changes_what_it_names_and_is_logged_first(
    #[case] make: fn(&Ids) -> ManualAction,
    #[case] holds: fn(&Driver, &Ids),
) {
    let (mut driver, ids) = game();
    take(&mut driver, make(&ids));
    holds(&driver, &ids);
}

/// The tokens of a name on the battlefield.
fn tokens_named(driver: &Driver, name: &str) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| {
            let object = driver.state().object(*id).unwrap();
            object.kind() == mtg_engine::object::ObjectKind::Token && object.card().name == name
        })
        .collect()
}

#[test]
fn a_token_moved_by_hand_off_the_battlefield_ceases_to_exist() {
    let (mut driver, _) = game();
    take(
        &mut driver,
        ManualAction::CreateToken {
            token: ManualToken::Common(CommonToken::Zombie),
            count: 1,
        },
    );
    let [zombie] = tokens_named(&driver, "Zombie")[..] else {
        panic!("one Zombie")
    };
    take(&mut driver, mv(zombie, ManualZone::Hand));
    assert!(driver.state().object(zombie).is_none());
    assert!(!driver.state().zone(Zone::Hand(pid(0))).contains(&zombie));
}

#[test]
fn an_attachment_is_taken_off_by_hand() {
    let (mut driver, ids) = game();
    take(
        &mut driver,
        ManualAction::Attach {
            object: ids.blade,
            to: ids.mine,
        },
    );
    take(&mut driver, ManualAction::Unattach { object: ids.blade });
    assert_eq!(driver.state().host_of(ids.blade), None);
}

/// Takes the action as seat 0 and returns why it was refused, after checking that nothing changed.
fn refused(driver: &mut Driver, action: ManualAction) -> ManualRefusal {
    let (log, hash) = (driver.engine().events().len(), driver.state().state_hash());
    let decision = driver.decision().unwrap().clone();
    let error = driver.manual(action).unwrap_err();
    let ScriptError::Engine(EngineError::Illegal(IllegalAnswer::Manual(refusal))) = error else {
        panic!("not a manual refusal: {error:?}");
    };
    assert_eq!(driver.engine().events().len(), log);
    assert_eq!(driver.state().state_hash(), hash);
    assert_eq!(driver.decision().unwrap(), &decision);
    refusal
}

fn custom(name: &str, types: Vec<CardType>, stats: Option<(i32, i32)>) -> ManualAction {
    ManualAction::CreateToken {
        token: ManualToken::Custom(Box::new(CustomToken {
            name: name.into(),
            colors: Vec::new(),
            types,
            subtypes: Vec::new(),
            power: stats.map(|(power, _)| power),
            toughness: stats.map(|(_, toughness)| toughness),
        })),
        count: 1,
    }
}

#[rstest]
#[case::an_opponents_permanent(|i: &Ids| mv(i.theirs, ManualZone::Graveyard), |_: &Ids| ManualRefusal::NotYours)]
#[case::an_opponents_card_in_hand(|i: &Ids| mv(i.foe_card, ManualZone::Graveyard), |_: &Ids| ManualRefusal::NotYours)]
#[case::an_opponents_life(|_: &Ids| ManualAction::Life { player: pid(1), delta: -3 }, |_: &Ids| ManualRefusal::NotYours)]
#[case::an_opponents_counters(
    |_: &Ids| ManualAction::PlayerCounters { player: pid(1), counter: CounterKind::Poison, delta: 1 },
    |_: &Ids| ManualRefusal::NotYours)]
#[case::an_opponents_hand_discarded(|i: &Ids| ManualAction::Discard { object: i.foe_card }, |_: &Ids| ManualRefusal::NotYours)]
#[case::a_card_in_a_library(|i: &Ids| mv(i.top, ManualZone::Hand),
    |i: &Ids| ManualRefusal::WrongZone { object: i.top, zone: Zone::Library(pid(0)) })]
#[case::an_object_that_is_not_there(|_: &Ids| mv(ObjectId(9999), ManualZone::Hand), |_: &Ids| ManualRefusal::Unknown(ObjectId(9999)))]
#[case::a_command_zone_the_game_lacks(|i: &Ids| mv(i.hand_creature, ManualZone::Command),
    |_: &Ids| ManualRefusal::NoDestination(ManualZone::Command))]
#[case::an_instant_onto_the_battlefield(|i: &Ids| mv(i.zap, ManualZone::Battlefield),
    |_: &Ids| ManualRefusal::NoDestination(ManualZone::Battlefield))]
#[case::a_card_to_the_zone_it_is_in(|i: &Ids| mv(i.ghoul, ManualZone::Graveyard), |_: &Ids| ManualRefusal::NoChange)]
#[case::the_top_of_the_library_to_the_battlefield(
    |_: &Ids| ManualAction::MoveTop { count: 1, to: ManualZone::Battlefield },
    |_: &Ids| ManualRefusal::NoDestination(ManualZone::Battlefield))]
#[case::more_cards_than_the_library_holds(
    |_: &Ids| ManualAction::MoveTop { count: 10, to: ManualZone::Graveyard },
    |_: &Ids| ManualRefusal::NotEnough("cards in the library"))]
#[case::no_cards(
    |_: &Ids| ManualAction::MoveTop { count: 0, to: ManualZone::Graveyard },
    |_: &Ids| ManualRefusal::OutOfRange("the number of cards"))]
#[case::looking_at_no_cards(|_: &Ids| ManualAction::LookTop { count: 0 },
    |_: &Ids| ManualRefusal::OutOfRange("the number of cards"))]
#[case::looking_at_eleven_cards(|_: &Ids| ManualAction::LookTop { count: 11 },
    |_: &Ids| ManualRefusal::OutOfRange("the number of cards"))]
#[case::arranging_cards_not_looked_at(
    |i: &Ids| ManualAction::ArrangeTop { top: Vec::new(), bottom: vec![i.top], graveyard: Vec::new() },
    |_: &Ids| ManualRefusal::NotLookedAt)]
#[case::arranging_nothing(
    |_: &Ids| ManualAction::ArrangeTop { top: Vec::new(), bottom: Vec::new(), graveyard: Vec::new() },
    |_: &Ids| ManualRefusal::OutOfRange("the number of cards"))]
#[case::discarding_from_the_battlefield(|i: &Ids| ManualAction::Discard { object: i.mine },
    |i: &Ids| ManualRefusal::WrongZone { object: i.mine, zone: Zone::Battlefield })]
#[case::tapping_a_tapped_permanent(|i: &Ids| ManualAction::Tap { object: i.tapper }, |_: &Ids| ManualRefusal::NoChange)]
#[case::untapping_an_untapped_permanent(|i: &Ids| ManualAction::Untap { object: i.mine }, |_: &Ids| ManualRefusal::NoChange)]
#[case::tapping_a_card_in_hand(|i: &Ids| ManualAction::Tap { object: i.hand_creature },
    |i: &Ids| ManualRefusal::WrongZone { object: i.hand_creature, zone: Zone::Hand(pid(0)) })]
#[case::counters_on_a_card_in_hand(
    |i: &Ids| ManualAction::Counters { object: i.hand_creature, counter: CounterKind::Charge, delta: 1 },
    |i: &Ids| ManualRefusal::WrongZone { object: i.hand_creature, zone: Zone::Hand(pid(0)) })]
#[case::no_counters(
    |i: &Ids| ManualAction::Counters { object: i.mine, counter: CounterKind::Charge, delta: 0 },
    |_: &Ids| ManualRefusal::OutOfRange("the number of counters"))]
#[case::too_many_counters(
    |i: &Ids| ManualAction::Counters { object: i.mine, counter: CounterKind::Charge, delta: 100 },
    |_: &Ids| ManualRefusal::OutOfRange("the number of counters"))]
#[case::more_counters_than_there_are(
    |i: &Ids| ManualAction::Counters { object: i.counted, counter: CounterKind::Charge, delta: -4 },
    |_: &Ids| ManualRefusal::NotEnough("counters there"))]
#[case::a_counter_without_a_name(
    |i: &Ids| ManualAction::Counters { object: i.mine, counter: CounterKind::Other(String::new()), delta: 1 },
    |_: &Ids| ManualRefusal::BadToken("a name of letters"))]
#[case::no_life(|_: &Ids| ManualAction::Life { player: pid(0), delta: 0 }, |_: &Ids| ManualRefusal::OutOfRange("the life"))]
#[case::too_much_life(|_: &Ids| ManualAction::Life { player: pid(0), delta: 1001 }, |_: &Ids| ManualRefusal::OutOfRange("the life"))]
#[case::no_tokens(
    |_: &Ids| ManualAction::CreateToken { token: ManualToken::Common(CommonToken::Soldier), count: 0 },
    |_: &Ids| ManualRefusal::OutOfRange("the number of tokens"))]
#[case::too_many_tokens(
    |_: &Ids| ManualAction::CreateToken { token: ManualToken::Common(CommonToken::Soldier), count: 11 },
    |_: &Ids| ManualRefusal::OutOfRange("the number of tokens"))]
#[case::a_token_without_a_name(|_: &Ids| custom("  ", vec![CardType::Artifact], None),
    |_: &Ids| ManualRefusal::BadToken("a name of 1 to 40 characters"))]
#[case::a_token_that_is_an_instant(|_: &Ids| custom("Spell", vec![CardType::Instant], None),
    |_: &Ids| ManualRefusal::BadToken("creature, artifact, enchantment or land as its types"))]
#[case::a_creature_token_without_a_size(|_: &Ids| custom("Imp", vec![CardType::Creature], None),
    |_: &Ids| ManualRefusal::BadToken("power and toughness from 0 to 99 on a creature only"))]
#[case::an_artifact_token_with_a_size(|_: &Ids| custom("Gem", vec![CardType::Artifact], Some((1, 1))),
    |_: &Ids| ManualRefusal::BadToken("power and toughness from 0 to 99 on a creature only"))]
#[case::a_huge_token(|_: &Ids| custom("Imp", vec![CardType::Creature], Some((100, 1))),
    |_: &Ids| ManualRefusal::BadToken("power and toughness from 0 to 99 on a creature only"))]
#[case::no_cards_drawn(|_: &Ids| ManualAction::Draw { count: 0 }, |_: &Ids| ManualRefusal::OutOfRange("the number of cards"))]
#[case::too_many_cards_drawn(|_: &Ids| ManualAction::Draw { count: 11 }, |_: &Ids| ManualRefusal::OutOfRange("the number of cards"))]
#[case::attaching_a_creature(|i: &Ids| ManualAction::Attach { object: i.mine, to: i.theirs },
    |i: &Ids| ManualRefusal::NotAttachable(i.mine))]
#[case::attaching_to_itself(|i: &Ids| ManualAction::Attach { object: i.blade, to: i.blade },
    |i: &Ids| ManualRefusal::NotAttachable(i.blade))]
#[case::attaching_to_a_card_in_hand(|i: &Ids| ManualAction::Attach { object: i.blade, to: i.hand_creature },
    |i: &Ids| ManualRefusal::WrongZone { object: i.hand_creature, zone: Zone::Hand(pid(0)) })]
#[case::taking_off_what_is_not_attached(|i: &Ids| ManualAction::Unattach { object: i.blade }, |_: &Ids| ManualRefusal::NoChange)]
fn an_action_that_is_not_allowed_is_refused_by_name_and_changes_nothing(
    #[case] make: fn(&Ids) -> ManualAction,
    #[case] why: fn(&Ids) -> ManualRefusal,
) {
    let (mut driver, ids) = game();
    assert_eq!(refused(&mut driver, make(&ids)), why(&ids));
}

#[test]
fn a_spell_on_the_stack_cannot_be_moved_by_hand() {
    let (mut driver, ids) = game();
    driver.cast("Wave").unwrap();
    assert_eq!(zone(&driver, ids.wave), Zone::Stack);
    assert_eq!(
        refused(&mut driver, mv(ids.wave, ManualZone::Hand)),
        ManualRefusal::WrongZone {
            object: ids.wave,
            zone: Zone::Stack
        }
    );
}

#[test]
fn the_seat_that_has_priority_acts_with_a_spell_on_the_stack_and_on_another_seats_turn() {
    let (mut driver, _) = game();
    driver.cast("Wave").unwrap();
    take(
        &mut driver,
        ManualAction::Life {
            player: pid(0),
            delta: 1,
        },
    );
    // The round started again, so seat 1 is asked after seat 0 passes, and it may act too.
    driver.pass().unwrap();
    assert_eq!(driver.decision().unwrap().player, pid(1));
    let events = driver
        .manual(ManualAction::Life {
            player: pid(1),
            delta: -2,
        })
        .unwrap();
    assert!(matches!(
        events.first(),
        Some(Event::ManualAction { player, .. }) if *player == pid(1)
    ));
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 18);
    assert_eq!(driver.decision().unwrap().player, pid(1));
}

#[test]
fn an_action_starts_the_round_again_so_the_other_seats_may_answer_it() {
    let (mut driver, _) = game();
    driver.pass().unwrap();
    assert_eq!(driver.decision().unwrap().player, pid(1));
    // Seat 1 acts having been passed to: seat 0, who passed, is asked again before the step can end.
    driver
        .manual(ManualAction::Life {
            player: pid(1),
            delta: 1,
        })
        .unwrap();
    driver.pass().unwrap();
    assert_eq!(driver.decision().unwrap().player, pid(0));
}

#[test]
fn a_planeswalker_put_onto_the_battlefield_by_hand_enters_with_its_loyalty() {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [mtg_testkit::cards::walker("Walker", 0)])
            .library(0, vec![forest(); 5])
            .library(1, vec![forest(); 5]),
    )
    .unwrap();
    let walker = find(&driver, Zone::Hand(pid(0)), "Walker");
    take(&mut driver, mv(walker, ManualZone::Battlefield));
    assert!(counters(&driver, walker, &CounterKind::Loyalty) > 0);
}

/// An Aura attached to nothing goes to the graveyard, unless its enchant ability is a clause the table follows: its
/// controller attaches it by hand, and the invariant checker, which judges a state at every decision, accepts it waiting.
#[rstest]
#[case::a_plain_aura(Arc::new(CardDefinition::new("Plain aura").with_type(CardType::Enchantment).with_subtype("Aura")), false)]
#[case::an_aura_the_engine_does_not_run(manual_aura("Charm"), true)]
fn an_aura_attached_to_nothing_waits_only_when_the_table_attaches_it(
    #[case] aura: Arc<CardDefinition>,
    #[case] waits: bool,
) {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(aura)])
            .library(0, vec![forest(); 5])
            .library(1, vec![forest(); 5]),
    )
    .unwrap();
    assert_eq!(
        driver.state().zone(Zone::Battlefield).len(),
        usize::from(waits)
    );
    mtg_testkit::check_settled(driver.state()).unwrap();
}
