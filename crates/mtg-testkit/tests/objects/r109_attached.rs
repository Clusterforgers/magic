//! Goal: cards that enter attached to a chosen object or player, at 2 to 4 seats: an Equipment that enters attached to a
//! creature its controller chooses (or none), a creature that returns as an Aura on a Forest whose granted static gives
//! the Forest mana abilities in turn, a creature that returns transformed as a Curse on an opponent, and an Aura of
//! opponents that moves to a player who attacks you, and the Auras and Equipment of a dead creature put onto another
//! (Cass); each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::ability_reader::abilities_of;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, forest, free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, enchanting, ready};
use rstest::rstest;

use crate::common::copying::{replays, settle};
use crate::common::{names, on_battlefield, pid};

/// Answers the questions of the stack until it is empty: the host question of an entering Equipment by `host` (none
/// when `None`), a may by `yes`, and every other question passively.
fn resolve_all(driver: &mut Driver, host: Option<&str>, yes: bool) -> Result<bool, ScriptError> {
    let mut asked = false;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::ChooseCards(options)
                if options.text.starts_with("You may choose a creature") =>
            {
                asked = true;
                assert_eq!(decision.player, pid(0));
                let chosen = host.map(|name| driver.id_of(name)).transpose()?;
                if let Some(id) = chosen {
                    assert!(options.from.contains(&id));
                }
                assert!(options.from.iter().all(|id| {
                    driver
                        .state()
                        .object(*id)
                        .is_some_and(|o| o.controller() == pid(0))
                }));
                driver.answer(Answer::ChooseCards(CardsChoice(
                    chosen.into_iter().collect(),
                )))?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The id of the permanent `name`'s attachment is attached to, if any.
fn host_of(driver: &Driver, name: &str) -> Option<mtg_core::ids::ObjectId> {
    let id = on_battlefield(driver.state(), name)?;
    driver
        .state()
        .object(id)?
        .permanent()?
        .attached_to()
        .map(|host| host.id)
}

#[rstest]
#[case::onto_my_creature(&["Mine"], Some("Mine"), true)]
#[case::onto_the_second_of_two(&["Mine", "Other"], Some("Other"), true)]
#[case::declined(&["Mine"], None, true)]
#[case::with_no_creature_nothing_is_asked(&[], None, false)]
fn an_equipment_enters_attached_to_a_chosen_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] mine: &[&str],
    #[case] host: Option<&str>,
    #[case] asked: bool,
) -> Result<(), ScriptError> {
    let table = || {
        let mut permanents = vec![ready(mountain()), ready(mountain()), ready(mountain())];
        permanents.extend(mine.iter().map(|name| ready(free_creature(name, 2, 2))));
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [card("Grifter's Blade").unwrap()])
            .battlefield(0, permanents)
            .battlefield(1, [ready(free_creature("Theirs", 2, 2))])
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Grifter's Blade")?;
    assert_eq!(resolve_all(&mut driver, host, true)?, asked);
    let expected = host.map(|name| driver.id_of(name)).transpose()?;
    assert_eq!(host_of(&driver, "Grifter's Blade"), expected);
    if let Some(id) = expected {
        assert_eq!(characteristics(driver.state(), id).unwrap().power, Some(3));
    }
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::harold_on_my_forest("Harold and Bob, First Numens", true, 1)]
#[case::troll_on_my_forest("Old-Growth Troll", true, 2)]
#[case::troll_with_no_forest("Old-Growth Troll", false, 0)]
fn a_dead_creature_returns_as_an_aura_that_grants_mana_abilities(
    #[values(2, 3, 4)] seats: usize,
    #[case] dying: &str,
    #[case] with_forest: bool,
    #[case] granted: usize,
) -> Result<(), ScriptError> {
    let table = || {
        let mut mine = vec![ready(card(dying).unwrap()), ready(mountain())];
        if with_forest {
            mine.push(ready(forest()));
        }
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [burn("Bolt", 10)])
            .battlefield(0, mine)
            .battlefield(1, [ready(forest())])
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    let forest_id = with_forest.then(|| driver.id_of("Forest")).transpose()?;
    let before = forest_id.map_or(0, |id| abilities_of(driver.state(), id).len());
    let target = driver.target_named(dying)?;
    driver.cast_at("Bolt", &[target])?;
    settle(&mut driver)?;
    let state = driver.state();
    let Some(forest_id) = forest_id else {
        assert!(on_battlefield(state, dying).is_none());
        assert!(names(state, Zone::Graveyard(pid(0))).contains(&dying.to_owned()));
        replays(&driver, &setup);
        return Ok(());
    };
    let aura = on_battlefield(state, dying).expect("it returned");
    assert_eq!(host_of(&driver, dying), Some(forest_id));
    assert!(!characteristics(state, aura).unwrap().is_creature());
    // The Aura has only the static it was given; the Forest gains what that static grants.
    assert_eq!(abilities_of(state, aura).len(), 1);
    assert_eq!(abilities_of(state, forest_id).len(), before + granted);
    let theirs = state
        .objects()
        .find(|object| object.zone() == Zone::Battlefield && object.controller() == pid(1))
        .map(mtg_engine::object::GameObject::id)
        .unwrap();
    assert_eq!(abilities_of(state, theirs).len(), before);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_dead_witch_returns_as_a_curse_on_an_opponent(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [burn("Bolt", 10)])
            .battlefield(
                0,
                [
                    ready(card("Accursed Witch // Infectious Curse").unwrap()),
                    ready(mountain()),
                ],
            )
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    let target = driver.target_named("Accursed Witch")?;
    driver.cast_at("Bolt", &[target])?;
    resolve_all(&mut driver, None, true)?;
    let state = driver.state();
    let curse = state
        .objects()
        .find(|object| {
            object.zone() == Zone::Battlefield
                && object
                    .permanent()
                    .is_some_and(|p| p.attached_player().is_some())
        })
        .expect("the Curse is on the battlefield");
    let enchanted = curse.permanent().unwrap().attached_player().unwrap();
    assert_ne!(enchanted, pid(0));
    assert_eq!(curse.controller(), pid(0));
    assert!(!characteristics(state, curse.id()).unwrap().is_creature());
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::moved_to_the_attacker(true)]
#[case::kept_where_it_is(false)]
fn an_aura_of_opponents_moves_to_a_player_who_attacks_you(
    #[values(3, 4)] seats: usize,
    #[case] yes: bool,
) -> Result<(), ScriptError> {
    // Seat 0's Archnemesis enchants the last seat; seat 1 attacks seat 0.
    let last = seats - 1;
    let table = || {
        Scenario::new(seats)
            .at(2, 1, Step::DeclareAttackers)
            .battlefield(0, [enchanting(ready(card("Archnemesis").unwrap()), last)])
            .battlefield(1, [ready(free_creature("Raider", 2, 2))])
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Raider", 0)])?;
    resolve_all(&mut driver, None, yes)?;
    let id = on_battlefield(driver.state(), "Archnemesis").unwrap();
    let enchanted = driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .attached_player();
    assert_eq!(enchanted, Some(pid(if yes { 1 } else { last })));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn attacking_the_enchanted_player_drains_and_draws(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .library(0, [free_creature("Card", 1, 1)])
            .battlefield(
                0,
                [
                    enchanting(ready(card("Archnemesis").unwrap()), 1),
                    ready(free_creature("Raider", 2, 2)),
                ],
            )
    };
    let setup = table().setup().unwrap();
    let mut driver = Driver::from_scenario(table())?;
    let life = |driver: &Driver, seat: usize| driver.state().player(pid(seat)).unwrap().life();
    let (mine, theirs) = (life(&driver, 0), life(&driver, 1));
    let hand = names(driver.state(), Zone::Hand(pid(0))).len();
    driver.attack(&[("Raider", 1)])?;
    resolve_all(&mut driver, None, true)?;
    assert_eq!(life(&driver, 1), theirs - 2);
    assert_eq!(life(&driver, 0), mine + 2);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), hand + 1);
    replays(&driver, &setup);
    Ok(())
}

/// Seat 0 controls Cass, "Mine" (enchanted by a Pacifism and equipped with a Bonesplitter when `attachments`), "Host"
/// and a Mountain; seat 1 controls a creature.
fn cass_table(seats: usize, attachments: bool) -> Scenario {
    let mut mine = vec![
        ready(card("Cass, Hand of Vengeance").unwrap()),
        ready(free_creature("Mine", 2, 2)),
        ready(free_creature("Host", 2, 2)),
        ready(mountain()),
    ];
    if attachments {
        mine.push(attached_to(ready(card("Pacifism").unwrap()), 0, 1));
        mine.push(attached_to(ready(card("Bonesplitter").unwrap()), 0, 1));
    }
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Bolt", 10)])
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("Theirs", 2, 2))])
}

#[rstest]
#[case::everything_returns(true, true)]
#[case::nothing_chosen(true, false)]
#[case::an_unattached_creature_triggers_nothing(false, true)]
fn cass_returns_the_auras_and_moves_the_equipment_of_a_dead_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] attachments: bool,
    #[case] all: bool,
) -> Result<(), ScriptError> {
    let setup = cass_table(seats, attachments).setup().unwrap();
    let mut driver = Driver::from_scenario(cass_table(seats, attachments))?;
    let host = driver.id_of("Host")?;
    let target = driver.target_named("Mine")?;
    driver.cast_at("Bolt", &[target])?;
    let mut triggered = false;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::ChooseTargets(_) => {
                triggered = true;
                let host = driver.target_named("Host")?;
                driver.choose_targets(&[host])?;
            }
            DecisionKind::ChooseCards(options) => {
                let picked = if all { options.from.clone() } else { Vec::new() };
                driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert_eq!(triggered, attachments);
    let state = driver.state();
    if attachments {
        let expected = all.then_some(host);
        assert_eq!(host_of(&driver, "Pacifism"), expected);
        assert_eq!(host_of(&driver, "Bonesplitter"), expected);
        assert_eq!(on_battlefield(state, "Pacifism").is_some(), all);
        // The Bonesplitter stays on the battlefield either way, unattached when it is not chosen.
        assert!(on_battlefield(state, "Bonesplitter").is_some());
    }
    replays(&driver, &setup);
    Ok(())
}
