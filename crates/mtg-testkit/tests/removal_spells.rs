//! Goal: Doom Blade, Swords to Plowshares, Counterspell and Wrath of God play whole, as their cards say, at tables of two, three and four seats: Doom Blade destroys a nonblack creature and is offered no black one, Swords to Plowshares exiles any creature, indestructible or not, and its controller gains the power it had at that moment and nothing for a power of 0 or less, Counterspell takes a spell off the stack for its owner's graveyard so that nothing of it happens, and Wrath of God destroys every creature but the indestructible ones in one step; each move carries its cause, and a spell is offered only while it has a target.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::removal::{
    corpus, gravedigger, moves, owner_of, pass_to, resolve_the_stack, sorted, strings, table, titan,
};
use common::{name_of, names, pid};
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::events::{ResolutionOutcome, ZoneCause};
use mtg_engine::zones::Zone;
use mtg_engine::{EngineError, Event};
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// What the pending target decision offers, as sorted card names.
fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::ChooseTargets(options) = &driver.decision().unwrap().kind else {
        panic!("a target is asked");
    };
    let mut found: Vec<String> = options
        .legal
        .iter()
        .filter_map(|target| match target {
            Target::Object(object) => Some(name_of(driver.state(), object.id)),
            Target::Player(_) => None,
        })
        .collect();
    found.sort();
    found
}

// Doom Blade

fn blade(seats: usize) -> Scenario {
    table(seats, &["Doom Blade"], &[swamp(), swamp()], &[])
}

#[rstest]
fn doom_blade_destroys_the_target_creature_and_offers_no_black_one(
    #[values((2, "Grizzly Bears"), (2, "Wolf"), (3, "Serra Angel"), (4, "Llanowar Elves"), (4, "Grizzly Bears"))]
    case: (usize, &str),
) -> Result<(), ScriptError> {
    let (seats, victim) = case;
    let mut driver = Driver::from_scenario(blade(seats))?;
    let owner = owner_of(&driver, victim);
    let before = names(driver.state(), Zone::Battlefield);
    let begun = driver.engine().events().len();
    driver.begin_cast("Doom Blade")?;

    let mut expected = vec!["Grizzly Bears", "Wolf"];
    expected.extend(
        ["Serra Angel", "Llanowar Elves"]
            .into_iter()
            .take(seats.saturating_sub(2)),
    );
    assert_eq!(
        offered(&driver),
        sorted(strings(&expected)),
        "no Gravedigger"
    );
    let target = driver.target_named(victim)?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    resolve_the_stack(&mut driver)?;

    assert_eq!(
        moves(&driver, begun),
        [(
            victim.to_owned(),
            Zone::Battlefield,
            Zone::Graveyard(pid(owner)),
            ZoneCause::Destroyed
        )],
        "one permanent moved, destroyed, to its owner's graveyard"
    );
    assert!(
        driver.engine().events()[begun..]
            .iter()
            .all(|event| !matches!(event, Event::CreatureDied { .. }))
    );
    assert!(!names(driver.state(), Zone::Battlefield).contains(&victim.to_owned()));
    assert!(names(driver.state(), Zone::Graveyard(pid(owner))).contains(&victim.to_owned()));
    assert!(
        names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Doom Blade".to_owned()),
        "the spell is spent"
    );
    let mut remaining = before;
    remaining.retain(|name| name != victim);
    assert_eq!(
        names(driver.state(), Zone::Battlefield),
        remaining,
        "nothing else left the battlefield"
    );
    assert_eq!(driver.state().stack(), []);
    for seat in 0..seats {
        driver.expect_life(seat, 20)?;
    }
    Ok(())
}

#[test]
fn a_black_creature_cannot_be_chosen_for_doom_blade() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(blade(2))?;
    driver.begin_cast("Doom Blade")?;
    let digger = driver.target_named("Gravedigger")?;
    let before = (driver.engine().state_hash(), driver.engine().events().len());
    let refused = driver
        .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![digger])))
        .unwrap_err();
    assert_eq!(
        refused,
        ScriptError::Engine(EngineError::Illegal(IllegalAnswer::NotOffered("target")))
    );
    assert_eq!(
        (driver.engine().state_hash(), driver.engine().events().len()),
        before,
        "a refusal changes nothing"
    );
    Ok(())
}

#[rstest]
#[case::no_creature_at_all(&[], false)]
#[case::only_a_black_one(&[("Gravedigger", true)], false)]
#[case::a_black_one_and_a_green_one(&[("Gravedigger", true), ("Grizzly Bears", false)], true)]
#[case::a_colorless_one(&[("Wolf", false)], true)]
fn doom_blade_is_offered_only_with_a_nonblack_creature_to_target(
    #[case] creatures: &[(&str, bool)],
    #[case] castable: bool,
) -> Result<(), ScriptError> {
    let board = creatures.iter().map(|(name, black)| match (*name, *black) {
        ("Gravedigger", _) => ready(gravedigger()),
        ("Wolf", _) => ready(free_creature("Wolf", 3, 3)),
        (other, _) => ready(corpus(other)),
    });
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [corpus("Doom Blade")])
            .battlefield(0, [ready(swamp()), ready(swamp())])
            .battlefield(1, board),
    )?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority");
    };
    let offered_cards: Vec<String> = options
        .castable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    assert_eq!(offered_cards.contains(&"Doom Blade".to_owned()), castable);
    Ok(())
}

#[rstest]
fn an_indestructible_creature_survives_doom_blade_which_still_resolves(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        &["Doom Blade"],
        &[swamp(), swamp()],
        &[(seats - 1, titan())],
    ))?;
    let begun = driver.engine().events().len();
    let target = driver.target_named("Titan")?;
    driver.cast_at("Doom Blade", &[target])?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(moves(&driver, begun), [], "nothing moved");
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Titan".to_owned()));
    let events = &driver.engine().events()[begun..];
    assert!(events.iter().any(|event| matches!(
        event,
        Event::ResolutionEnded {
            outcome: ResolutionOutcome::Resolved,
            ..
        }
    )));
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Doom Blade"],
        "the spell is spent"
    );
    Ok(())
}

// Swords to Plowshares

fn swords(seats: usize) -> Scenario {
    table(
        seats,
        &["Swords to Plowshares"],
        &[plains()],
        &[(seats - 1, titan())],
    )
}

/// The power each creature of the tables has.
fn power(name: &str) -> i32 {
    match name {
        "Wolf" => 3,
        "Grizzly Bears" | "Gravedigger" => 2,
        "Serra Angel" => 4,
        "Llanowar Elves" => 1,
        "Titan" => 5,
        other => panic!("no power for {other}"),
    }
}

#[rstest]
fn swords_exiles_the_creature_and_its_controller_gains_the_power_it_had(
    #[values(
        (2, "Grizzly Bears", 0),
        (2, "Wolf", 0),
        (3, "Serra Angel", 0),
        (3, "Serra Angel", 2),
        (4, "Llanowar Elves", 0),
        (4, "Llanowar Elves", 3),
        (4, "Titan", 0),
        (4, "Grizzly Bears", -1)
    )]
    case: (usize, &str, i32),
) -> Result<(), ScriptError> {
    let (seats, victim, bonus) = case;
    let mut driver = Driver::from_scenario(swords(seats))?;
    let owner = owner_of(&driver, victim);
    let id = driver.id_of(victim)?;
    let begun = driver.engine().events().len();
    let target = driver.target_named(victim)?;
    driver.cast_at("Swords to Plowshares", &[target])?;
    if bonus != 0 {
        // The creature is bigger or smaller than its card when it is exiled, and its controller gains what it was.
        driver.add_effect(
            Selector::Object(id),
            Modification::ModifyPowerToughness {
                power: bonus,
                toughness: bonus.max(0),
            },
            Duration::UntilEndOfTurn,
        )?;
    }
    resolve_the_stack(&mut driver)?;

    let played = driver.engine().events()[begun..].to_vec();
    assert_eq!(
        moves(&driver, begun),
        [(
            victim.to_owned(),
            Zone::Battlefield,
            Zone::Exile,
            ZoneCause::Exiled
        )]
    );
    let gain = power(victim) + bonus;
    let gains: Vec<(usize, u32)> = played
        .iter()
        .filter_map(|event| match event {
            Event::LifeGained { player, amount } => Some((player.index(), *amount)),
            _ => None,
        })
        .collect();
    assert_eq!(
        gains,
        [(owner, u32::try_from(gain).unwrap())],
        "the controller gains its power as it was"
    );
    let exile = played
        .iter()
        .position(|event| {
            matches!(
                event,
                Event::ZoneChanged {
                    to: Zone::Exile,
                    ..
                }
            )
        })
        .unwrap();
    let life = played
        .iter()
        .position(|event| matches!(event, Event::LifeGained { .. }))
        .unwrap();
    assert!(exile < life, "the creature is exiled first");
    assert_eq!(names(driver.state(), Zone::Exile), [victim]);
    for seat in 0..seats {
        let expected = 20 + if seat == owner { gain } else { 0 };
        driver.expect_life(seat, expected)?;
    }
    Ok(())
}

#[rstest]
#[case::no_power(0)]
#[case::negative_power(-5)]
fn swords_gains_nothing_for_a_creature_without_power(
    #[case] set_power: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(swords(3))?;
    let id = driver.id_of("Serra Angel")?;
    let target = driver.target_named("Serra Angel")?;
    driver.cast_at("Swords to Plowshares", &[target])?;
    driver.add_effect(
        Selector::Object(id),
        Modification::SetPowerToughness {
            power: set_power,
            toughness: 4,
        },
        Duration::UntilEndOfTurn,
    )?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Serra Angel"]);
    for seat in 0..3 {
        driver.expect_life(seat, 20)?;
    }
    driver.expect_no_event("life gained or lost", |event| {
        matches!(event, Event::LifeGained { .. } | Event::LifeLost { .. })
    })?;
    Ok(())
}

// Counterspell

fn counterspell(seats: usize, countering: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [corpus("Lightning Bolt"), corpus("Grizzly Bears")])
        .battlefield(0, [ready(mountain()), ready(forest()), ready(forest())]);
    for seat in 1..seats {
        scenario = if seat == countering {
            scenario
                .hand(seat, [corpus("Counterspell")])
                .battlefield(seat, [ready(island()), ready(island())])
        } else {
            scenario.battlefield(seat, [ready(free_creature("Bystander", 1, 1))])
        };
    }
    scenario
}

#[rstest]
fn counterspell_takes_a_spell_off_the_stack_and_nothing_of_it_happens(
    #[values((2, 1, 1), (3, 2, 1), (3, 1, 2), (4, 3, 2), (4, 2, 3))] seating: (usize, usize, usize),
    #[values("Lightning Bolt", "Grizzly Bears")] spell: &str,
) -> Result<(), ScriptError> {
    let (seats, countering, aimed) = seating;
    let mut driver = Driver::from_scenario(counterspell(seats, countering))?;
    let begun = driver.engine().events().len();
    if spell == "Lightning Bolt" {
        driver.cast_at(spell, &[driver.target_player(aimed)])?;
    } else {
        driver.cast(spell)?;
    }
    let cast = *driver.state().stack().last().unwrap();
    let before = driver.state().object(cast).unwrap().counter();
    pass_to(&mut driver, countering)?;
    let target = Target::Object(driver.state().object(cast).unwrap().reference());
    let events = driver.cast_at("Counterspell", &[target])?;
    assert!(events.iter().any(|e| matches!(e, Event::SpellCast { .. })));
    resolve_the_stack(&mut driver)?;

    let played = &driver.engine().events()[begun..];
    let ended: Vec<(ObjectId, ResolutionOutcome)> = played
        .iter()
        .filter_map(|event| match event {
            Event::ResolutionEnded { object, outcome } => Some((*object, *outcome)),
            _ => None,
        })
        .collect();
    let counter = played
        .iter()
        .find_map(|event| match event {
            Event::ResolutionBegan { object } => Some(*object),
            _ => None,
        })
        .unwrap();
    assert_eq!(name_of(driver.state(), counter), "Counterspell");
    assert_eq!(
        ended,
        [
            (cast, ResolutionOutcome::Countered),
            (counter, ResolutionOutcome::Resolved)
        ],
        "the spell is countered by the step, then Counterspell ends"
    );
    assert_eq!(
        played
            .iter()
            .filter(|e| matches!(e, Event::ResolutionBegan { .. }))
            .count(),
        1,
        "the countered spell never began to resolve"
    );
    assert!(
        !played
            .iter()
            .any(|event| matches!(event, Event::DamageDealt { .. })),
        "the Bolt dealt no damage"
    );
    assert!(
        !played
            .iter()
            .any(|event| matches!(event, Event::SpellResolved { object } if *object == cast)),
        "the countered spell did not resolve"
    );
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), [spell]);
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(countering))),
        ["Counterspell"]
    );
    assert!(!names(driver.state(), Zone::Battlefield).contains(&"Grizzly Bears".to_owned()));
    assert!(driver.state().object(cast).unwrap().counter() > before);
    assert_eq!(driver.state().stack(), []);
    for seat in 0..seats {
        driver.expect_life(seat, 20)?;
    }
    Ok(())
}

#[test]
fn counterspell_is_offered_only_while_a_spell_is_on_the_stack() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [corpus("Counterspell"), corpus("Lightning Bolt")])
            .battlefield(0, [ready(island()), ready(island()), ready(mountain())]),
    )?;
    let offered_now = |driver: &Driver| -> Vec<String> {
        let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
            panic!("priority");
        };
        options
            .castable
            .iter()
            .map(|id| name_of(driver.state(), *id))
            .collect()
    };
    assert_eq!(
        offered_now(&driver),
        ["Lightning Bolt"],
        "nothing to counter"
    );
    driver.cast_at("Lightning Bolt", &[driver.target_player(1)])?;
    assert_eq!(
        offered_now(&driver),
        ["Counterspell"],
        "the Bolt is a spell to counter, and the lands left pay for the Counterspell"
    );
    Ok(())
}

// Wrath of God

fn wrath(seats: usize) -> Scenario {
    table(
        seats,
        &["Wrath of God"],
        &[plains(), plains(), plains(), plains()],
        &[(seats - 1, titan()), (0, corpus("Sol Ring")), (1, forest())],
    )
}

#[rstest]
fn wrath_of_god_destroys_every_creature_but_the_indestructible_ones_at_once(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(wrath(seats))?;
    let creatures: Vec<(ObjectId, String, usize)> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|o| o.card().power.is_some() && o.card().name != "Titan")
        .map(|o| (o.id(), o.card().name.clone(), o.owner().index()))
        .collect();
    let non_creatures: Vec<Vec<String>> = (0..seats)
        .map(|seat| {
            driver
                .state()
                .zone(Zone::Battlefield)
                .iter()
                .filter_map(|id| driver.state().object(*id))
                .filter(|o| o.controller() == pid(seat) && o.card().power.is_none())
                .map(|o| o.card().name.clone())
                .collect()
        })
        .collect();
    assert!(creatures.len() >= 2 + seats - 1);
    let begun = driver.engine().events().len();
    driver.cast("Wrath of God")?;
    resolve_the_stack(&mut driver)?;

    // Every destruction is one move with its cause, and they all come in the one step of the spell.
    let played = &driver.engine().events()[begun..];
    let step_ends: Vec<usize> = played
        .iter()
        .enumerate()
        .filter(|(_, event)| matches!(event, Event::StepDone { .. }))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(step_ends.len(), 1, "the spell has one step");
    let destroyed: Vec<(usize, ObjectId)> = played
        .iter()
        .enumerate()
        .filter_map(|(at, event)| match event {
            Event::ZoneChanged {
                object,
                from: Zone::Battlefield,
                cause: ZoneCause::Destroyed,
                ..
            } => Some((at, *object)),
            _ => None,
        })
        .collect();
    assert!(destroyed.iter().all(|(at, _)| *at < step_ends[0]));
    let mut gone: Vec<ObjectId> = destroyed.iter().map(|(_, object)| *object).collect();
    gone.sort();
    let mut expected: Vec<ObjectId> = creatures.iter().map(|(id, ..)| *id).collect();
    expected.sort();
    assert_eq!(gone, expected, "every creature but the Titan");
    assert!(
        played
            .iter()
            .all(|event| !matches!(event, Event::CreatureDied { .. }))
    );

    for (id, name, owner) in &creatures {
        assert_eq!(
            driver.state().object(*id).unwrap().zone(),
            Zone::Graveyard(pid(*owner)),
            "{name} is in its owner's graveyard"
        );
    }
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Titan".to_owned()));
    for (seat, before) in non_creatures.iter().enumerate() {
        let now: Vec<String> = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| driver.state().object(*id))
            .filter(|o| o.controller() == pid(seat) && o.card().power.is_none())
            .map(|o| o.card().name.clone())
            .collect();
        assert_eq!(&now, before, "lands and artifacts stay");
    }
    assert_eq!(driver.state().stack(), []);
    for seat in 0..seats {
        driver.expect_life(seat, 20)?;
    }
    Ok(())
}

#[test]
fn wrath_of_god_with_no_creature_on_the_table_destroys_nothing() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [corpus("Wrath of God")])
            .battlefield(0, [plains(), plains(), plains(), plains()].map(ready)),
    )?;
    let begun = driver.engine().events().len();
    driver.cast("Wrath of God")?;
    resolve_the_stack(&mut driver)?;
    assert_eq!(moves(&driver, begun), [], "no permanent moved");
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Wrath of God"]
    );
    assert_eq!(names(driver.state(), Zone::Battlefield).len(), 4);
    Ok(())
}
