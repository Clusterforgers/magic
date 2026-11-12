//! Goal: bestow (CR 702.103) is an alternative cost from the hand that casts the creature card as an Aura spell with enchant creature: it is offered beside the mana cost only with a creature to enchant and the mana to pay, the spell is no creature spell while it is on the stack, the permanent enters attached to its target as an Aura that is no creature and gives its bonus to the host, a target that is gone as the spell resolves makes it resolve as a creature, a host that leaves makes the permanent a creature again, a copy of the creature is no bestowed Aura and the Aura is not copied; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::{choose_way, pay, ways};
use crate::common::{name_of, names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::keywords::has_keyword;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, GameSetup, GameState, characteristics, replay};
use mtg_testkit::cards::{bear, cost, forest, free_creature, island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;
use std::sync::Arc;

/// Counters creature spells and nothing else.
fn essence_scatter() -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(
        r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Type":"Creature"}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Essence Scatter")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(1, &[Color::Blue]))
            .with_rules(spell),
    )
}

/// Seat 0 holds `hand` with `forests` Forests and `plains` Plains, and `islands` Islands; `creatures` puts a Bear with
/// seat 0 and an Ogre with seat 1. Seat 1 holds Doom Blade and Essence Scatter with the lands for them.
fn table(
    seats: usize,
    hand: &[&str],
    (forests, white, islands): (usize, usize, usize),
    creatures: bool,
) -> Scenario {
    let mut mine: Vec<_> = (0..forests).map(|_| ready(forest())).collect();
    mine.extend((0..white).map(|_| ready(plains())));
    mine.extend((0..islands).map(|_| ready(island())));
    let mut theirs = vec![
        ready(swamp()),
        ready(swamp()),
        ready(island()),
        ready(island()),
    ];
    if creatures {
        mine.push(ready(bear()));
        theirs.push(ready(free_creature("Ogre", 3, 4)));
    }
    let scenario = Scenario::new(seats)
        .hand(0, hand.iter().map(|name| card(name).unwrap()))
        .hand(1, [card("Doom Blade").unwrap(), essence_scatter()])
        .battlefield(0, mine)
        .battlefield(1, theirs);
    (2..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    })
}

/// Answers whatever is asked passively until the stack is empty and nothing resolves.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 casts `name` for its bestow cost at `host` and pays, leaving the spell on the stack.
fn cast_bestowed(driver: &mut Driver, name: &str, host: &str) -> Result<(), ScriptError> {
    cast_bestowed_with(driver, name, host, &[])
}

/// [`cast_bestowed`] paying by tapping the named lands, or whichever when there are none named.
fn cast_bestowed_with(
    driver: &mut Driver,
    name: &str,
    host: &str,
    lands: &[&str],
) -> Result<(), ScriptError> {
    let target = driver.target_named(host)?;
    driver.begin_cast(name)?;
    choose_way(driver, "Bestow")?;
    driver.choose_targets(&[target])?;
    if lands.is_empty() {
        pay(driver)?;
    } else {
        driver.pay_tapping(lands, 0)?;
    }
    Ok(())
}

/// Passes until `seat` has priority.
fn until_priority_of(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    while driver.decision()?.player != pid(seat) {
        driver.pass()?;
    }
    Ok(())
}

fn id_of(driver: &Driver, name: &str) -> ObjectId {
    driver.id_of(name).unwrap()
}

fn stats(state: &GameState, id: ObjectId) -> (i32, i32) {
    let chars = characteristics(state, id).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

fn is_creature(state: &GameState, id: ObjectId) -> bool {
    characteristics(state, id).unwrap().is_creature()
}

fn is_aura(state: &GameState, id: ObjectId) -> bool {
    let chars = characteristics(state, id).unwrap();
    chars.types.contains(CardType::Enchantment)
        && chars.has_subtype(
            &state.object(id).unwrap().card().subtypes,
            &Subtype("Aura".into()),
        )
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn count(events: &[Event], pattern: impl Fn(&Event) -> bool) -> usize {
    events.iter().filter(|event| pattern(event)).count()
}

#[rstest]
#[case::both_ways("Boon Satyr", (5, 0, 0), true, &["Cast for {1}{G}{G}", "Bestow {3}{G}{G}"])]
#[case::both_ways_by_a_white_card("Celestial Archon", (0, 7, 0), true, &["Cast for {3}{W}{W}", "Bestow {5}{W}{W}"])]
#[case::too_little_mana_to_bestow("Boon Satyr", (4, 0, 0), true, &[])]
#[case::no_creature_to_enchant("Boon Satyr", (5, 0, 0), false, &[])]
fn bestow_is_offered_beside_the_mana_cost_only_with_a_creature_and_the_mana(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] lands: (usize, usize, usize),
    #[case] creatures: bool,
    #[case] offered: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &[name], lands, creatures))?;
    driver.begin_cast(name)?;
    if offered.is_empty() {
        // Only the mana cost is left, so nothing is asked but the payment.
        assert!(matches!(driver.decision()?.kind, DecisionKind::PayMana(_)));
    } else {
        assert_eq!(ways(&driver)?, offered);
    }
    Ok(())
}

#[rstest]
#[case::own_creature_with_a_pump("Boon Satyr", (5, 0, 0), "Grizzly Bears", (6, 4))]
#[case::opponents_creature_with_a_pump("Boon Satyr", (5, 0, 0), "Ogre", (7, 6))]
#[case::a_pump_and_two_keywords("Celestial Archon", (0, 7, 0), "Grizzly Bears", (6, 6))]
fn a_bestowed_creature_enters_attached_as_an_aura_that_is_no_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] name: &str,
    #[case] lands: (usize, usize, usize),
    #[case] host: &str,
    #[case] pumped: (i32, i32),
) -> Result<(), ScriptError> {
    let setup = table(seats, &[name], lands, true).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &[name], lands, true))?;
    let before = names(driver.state(), Zone::Battlefield).len();
    cast_bestowed(&mut driver, name, host)?;
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    assert!(matches!(
        events
            .iter()
            .skip_while(|event| !matches!(event, Event::SpellResolved { .. }))
            .nth(1),
        Some(Event::Attached { .. })
    ));
    let aura = id_of(&driver, name);
    let host_id = id_of(&driver, host);
    let state = driver.state();
    assert_eq!(names(state, Zone::Battlefield).len(), before + 1);
    assert_eq!(state.host_of(aura), Some(host_id));
    assert!(is_aura(state, aura) && !is_creature(state, aura));
    assert!(state.object(aura).unwrap().bestowed());
    assert_eq!(stats(state, host_id), pumped);
    if name == "Celestial Archon" {
        assert!(has_keyword(
            state,
            host_id,
            &mtg_cards::keyword::Keyword::Flying
        ));
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_bestowed_spell_is_no_creature_spell_while_it_is_on_the_stack(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] bestowed: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Boon Satyr"], (5, 0, 0), true))?;
    if bestowed {
        cast_bestowed(&mut driver, "Boon Satyr", "Ogre")?;
    } else {
        driver.begin_cast("Boon Satyr")?;
        choose_way(&mut driver, "Cast for")?;
        pay(&mut driver)?;
    }
    let spell = *driver.state().stack().last().unwrap();
    assert_eq!(is_creature(driver.state(), spell), !bestowed);
    assert_eq!(is_aura(driver.state(), spell), bestowed);
    // Seat 1 may counter a creature spell with Essence Scatter, and no Aura spell.
    until_priority_of(&mut driver, 1)?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority");
    };
    let castable = options
        .castable
        .iter()
        .any(|id| name_of(driver.state(), *id) == "Essence Scatter");
    assert_eq!(castable, !bestowed);
    Ok(())
}

#[rstest]
fn a_target_that_is_gone_as_the_spell_resolves_makes_it_resolve_as_a_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, &["Boon Satyr"], (5, 0, 0), true)
        .setup()
        .unwrap();
    let mut driver = Driver::from_scenario(table(seats, &["Boon Satyr"], (5, 0, 0), true))?;
    cast_bestowed(&mut driver, "Boon Satyr", "Ogre")?;
    // Seat 1 destroys the creature the spell is aimed at.
    until_priority_of(&mut driver, 1)?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Doom Blade", &[ogre])?;
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    assert_eq!(count(events, |e| matches!(e, Event::Attached { .. })), 0);
    let satyr = id_of(&driver, "Boon Satyr");
    let state = driver.state();
    assert!(is_creature(state, satyr) && !is_aura(state, satyr));
    assert!(!state.object(satyr).unwrap().bestowed());
    assert_eq!(state.host_of(satyr), None);
    assert_eq!(stats(state, satyr), (4, 2));
    assert_eq!(names(state, Zone::Graveyard(pid(0))), Vec::<String>::new());
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_host_that_dies_makes_the_bestowed_aura_a_creature_again(
    #[values(2, 3, 4)] seats: usize,
    #[values("Boon Satyr", "Celestial Archon")] name: &str,
) -> Result<(), ScriptError> {
    let lands = if name == "Boon Satyr" {
        (5, 0, 0)
    } else {
        (0, 7, 0)
    };
    let setup = table(seats, &[name], lands, true).setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, &[name], lands, true))?;
    cast_bestowed(&mut driver, name, "Ogre")?;
    settle(&mut driver)?;
    let satyr = id_of(&driver, name);
    assert!(driver.state().object(satyr).unwrap().bestowed());
    until_priority_of(&mut driver, 1)?;
    let ogre = driver.target_named("Ogre")?;
    driver.cast_at("Doom Blade", &[ogre])?;
    let begun = driver.engine().events().len();
    settle(&mut driver)?;
    let events = &driver.engine().events()[begun..];
    assert_eq!(count(events, |e| matches!(e, Event::Unattached { .. })), 1);
    let state = driver.state();
    assert_eq!(state.object(satyr).unwrap().zone(), Zone::Battlefield);
    assert!(is_creature(state, satyr) && !is_aura(state, satyr));
    assert!(!state.object(satyr).unwrap().bestowed());
    assert_eq!(state.host_of(satyr), None);
    let printed = state.object(satyr).unwrap().card();
    assert_eq!(
        stats(state, satyr),
        (printed.power.unwrap(), printed.toughness.unwrap())
    );
    assert_eq!(names(state, Zone::Graveyard(pid(0))), Vec::<String>::new());
    let mut theirs = names(state, Zone::Graveyard(pid(1)));
    theirs.sort();
    assert_eq!(theirs, ["Doom Blade", "Ogre"]);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_clone_copies_the_creature_and_never_the_bestowed_aura(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] bestowed: bool,
) -> Result<(), ScriptError> {
    let build = || table(seats, &["Boon Satyr", "Clone"], (5, 0, 4), true);
    let setup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    // The Islands stay untapped for the Clone.
    if bestowed {
        cast_bestowed_with(&mut driver, "Boon Satyr", "Grizzly Bears", &["Forest"; 5])?;
    } else {
        driver.begin_cast("Boon Satyr")?;
        choose_way(&mut driver, "Cast for")?;
        driver.pay_tapping(&["Forest"; 3], 0)?;
    }
    settle(&mut driver)?;
    let satyr = id_of(&driver, "Boon Satyr");
    driver.cast_tapping("Clone", &["Island", "Island", "Island", "Island"])?;
    let mut options = None;
    for _ in 0..50 {
        if let Some(DecisionKind::ChooseCards(asked)) =
            driver.engine().decision().map(|d| d.kind.clone())
        {
            options = Some(asked.from);
            break;
        }
        driver.answer_passively()?;
    }
    let options = options.expect("Clone asks what to copy");
    assert_eq!(options.contains(&satyr), !bestowed);
    assert!(options.contains(&id_of(&driver, "Grizzly Bears")));
    if !bestowed {
        driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(vec![
            satyr,
        ])))?;
        settle(&mut driver)?;
        let copies: Vec<ObjectId> = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .filter(|id| name_of(driver.state(), *id) == "Boon Satyr" && *id != satyr)
            .collect();
        let [copy] = copies.as_slice() else {
            panic!("one copy: {copies:?}");
        };
        let state = driver.state();
        assert!(is_creature(state, *copy) && !is_aura(state, *copy));
        assert!(!state.object(*copy).unwrap().bestowed());
        assert_eq!(state.host_of(*copy), None);
        assert_eq!(stats(state, *copy), (4, 2));
        replays(&driver, &setup);
    }
    Ok(())
}

#[rstest]
#[case::enters_as_an_aura(false, (6, 4))]
#[case::resolves_as_a_creature(true, (3, 3))]
fn a_creature_entering_triggers_cathars_crusade_only_when_bestow_ends_in_one(
    #[values(2, 3, 4)] seats: usize,
    #[case] target_gone: bool,
    #[case] bear_stats: (i32, i32),
) -> Result<(), ScriptError> {
    let build = || {
        let mut mine: Vec<_> = (0..5).map(|_| ready(forest())).collect();
        mine.extend([ready(card("Cathars' Crusade").unwrap()), ready(bear())]);
        let theirs = [
            ready(swamp()),
            ready(swamp()),
            ready(free_creature("Ogre", 3, 4)),
        ];
        (2..seats).fold(
            Scenario::new(seats)
                .hand(0, [card("Boon Satyr").unwrap()])
                .hand(1, [card("Doom Blade").unwrap()])
                .battlefield(0, mine)
                .battlefield(1, theirs),
            |scenario, seat| scenario.library(seat, (0..8).map(|_| forest())),
        )
    };
    let setup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    let host = if target_gone { "Ogre" } else { "Grizzly Bears" };
    cast_bestowed(&mut driver, "Boon Satyr", host)?;
    if target_gone {
        until_priority_of(&mut driver, 1)?;
        let ogre = driver.target_named("Ogre")?;
        driver.cast_at("Doom Blade", &[ogre])?;
    }
    settle(&mut driver)?;
    let bear_id = id_of(&driver, "Grizzly Bears");
    assert_eq!(stats(driver.state(), bear_id), bear_stats);
    replays(&driver, &setup);
    Ok(())
}
