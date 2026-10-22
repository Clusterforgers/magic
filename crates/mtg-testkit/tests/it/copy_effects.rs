//! Goal: storm, copies of abilities and a copy that lasts until end of turn play whole, at tables of two, three and four seats: a storm spell copies itself once for each spell cast before it this turn by any player (0, 1 and 3), counted when it is cast so a spell cast in response does not change it, the copies are put on the stack by the trigger and not cast, and each may take new targets; Lithoform Engine copies an activated ability and Strionic Resonator a triggered one, the copy has the choices of the original or new targets with the division moved to them and is neither activated nor triggered; Cytoshape makes a creature a copy of another until the cleanup step of the turn and it is what it was after; and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::{
    ability_copies, casts, game, hand, keep_targets_until_done, lands, life, pass_round, replays,
    run, say, spell_copies,
};
use common::pid;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, characteristics};
use mtg_testkit::cards::{forest, instant, island, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// `prior` instants for `{R}` that do nothing, then Grapeshot (or `storm`), in seat 0's hand, over `prior + 2` Mountains.
fn storm_game(seats: usize, prior: usize, storm: &str) -> Scenario {
    let fillers = (0..prior).map(|i| instant(&format!("Filler {i}"), 0, &[Color::Red]));
    game(seats, &[], 1, prior + 2)
        .hand(0, fillers.chain([card(storm).unwrap()]).collect::<Vec<_>>())
        .library(1, (0..12).map(|_| island()))
}

/// Casts the fillers one after the other and lets each resolve, so a sorcery can be cast after them.
fn cast_fillers(driver: &mut Driver, prior: usize, seats: usize) -> Result<(), ScriptError> {
    for i in 0..prior {
        driver.cast_tapping(&format!("Filler {i}"), &["Mountain"])?;
        run(driver, seats)?;
    }
    Ok(())
}

#[rstest]
fn grapeshot_copies_itself_for_each_spell_cast_before_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] prior: usize,
) -> Result<(), ScriptError> {
    let setup = storm_game(seats, prior, "Grapeshot").setup().unwrap();
    let mut driver = Driver::from_scenario(storm_game(seats, prior, "Grapeshot"))?;
    cast_fillers(&mut driver, prior, seats)?;
    let foe = driver.target_player(1);
    driver.begin_cast("Grapeshot")?;
    driver.choose_targets(&[foe])?;
    driver.pay_tapping(&["Mountain", "Mountain"], 0)?;
    assert_eq!(casts(&driver), prior + 1);
    // The trigger is on the stack above the spell; the copies come when it resolves.
    assert_eq!(driver.state().stack().len(), 2);
    assert_eq!(spell_copies(&driver), Vec::<mtg_core::ids::ObjectId>::new());
    keep_targets_until_done(&mut driver, seats)?;
    assert_eq!(casts(&driver), prior + 1, "a copy is not cast");
    assert_eq!(
        life(&driver, 1),
        20 - i32::try_from(prior + 1).unwrap(),
        "Grapeshot and a copy for each spell before it"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn the_spells_of_every_player_count_for_storm(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        storm_game(seats, 1, "Grapeshot")
            .battlefield(1, lands(0, 1))
            .hand(1, [instant("Their Filler", 0, &[Color::Red])])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    cast_fillers(&mut driver, 1, seats)?;
    driver.pass()?;
    // The next seat to get priority casts a spell of its own.
    for _ in 0..seats {
        if driver.decision()?.player == pid(1) {
            break;
        }
        driver.pass()?;
    }
    driver.cast_tapping("Their Filler", &["Mountain"])?;
    run(&mut driver, seats)?;
    assert_eq!(casts(&driver), 2);
    let foe = driver.target_player(1);
    driver.begin_cast("Grapeshot")?;
    driver.choose_targets(&[foe])?;
    driver.pay_tapping(&["Mountain", "Mountain"], 0)?;
    keep_targets_until_done(&mut driver, seats)?;
    assert_eq!(life(&driver, 1), 17, "Grapeshot and two copies");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_spell_cast_in_response_to_the_storm_trigger_is_not_counted(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = storm_game(seats, 2, "Grapeshot").setup().unwrap();
    let mut driver = Driver::from_scenario(storm_game(seats, 2, "Grapeshot"))?;
    cast_fillers(&mut driver, 1, seats)?;
    let foe = driver.target_player(1);
    driver.begin_cast("Grapeshot")?;
    driver.choose_targets(&[foe])?;
    driver.pay_tapping(&["Mountain", "Mountain"], 0)?;
    driver.cast_tapping("Filler 1", &["Mountain"])?;
    keep_targets_until_done(&mut driver, seats)?;
    assert_eq!(
        life(&driver, 1),
        18,
        "Grapeshot and one copy for the one spell before it"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn each_copy_of_a_storm_spell_may_take_new_targets(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = storm_game(seats, 2, "Grapeshot").setup().unwrap();
    let mut driver = Driver::from_scenario(storm_game(seats, 2, "Grapeshot"))?;
    cast_fillers(&mut driver, 2, seats)?;
    let foe = driver.target_player(1);
    driver.begin_cast("Grapeshot")?;
    driver.choose_targets(&[foe])?;
    driver.pay_tapping(&["Mountain", "Mountain"], 0)?;
    let third = driver.target_player(2);
    let mut asked = 0;
    for _ in 0..100 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                asked += 1;
                // The first copy keeps its target, the second takes another.
                say(&mut driver, asked == 2)?;
            }
            DecisionKind::ChooseTargets(options) => {
                assert_eq!((options.min, options.max), (1, 1));
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![third])))?;
            }
            _ => pass_round(&mut driver, seats)?,
        }
    }
    assert_eq!(asked, 2);
    assert_eq!((life(&driver, 1), life(&driver, 2)), (18, 19));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn brain_freeze_and_tendrils_of_agony_storm_too(
    #[values("Brain Freeze", "Tendrils of Agony")] name: &str,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        Scenario::new(seats)
            .battlefield(
                0,
                lands(2, 5)
                    .into_iter()
                    .chain([ready(swamp()), ready(swamp())]),
            )
            .hand(
                0,
                [
                    instant("Filler 0", 0, &[Color::Red]),
                    instant("Filler 1", 0, &[Color::Red]),
                    card(name).unwrap(),
                ],
            )
            .library(1, (0..12).map(|_| island()))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    cast_fillers(&mut driver, 2, seats)?;
    let foe = driver.target_player(1);
    driver.begin_cast(name)?;
    driver.choose_targets(&[foe])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    keep_targets_until_done(&mut driver, seats)?;
    if name == "Brain Freeze" {
        assert_eq!(driver.state().zone(Zone::Library(pid(1))).len(), 12 - 9);
    } else {
        assert_eq!((life(&driver, 0), life(&driver, 1)), (26, 14));
    }
    replays(&driver, &setup);
    Ok(())
}

fn walker_game(seats: usize) -> Scenario {
    Scenario::new(seats).battlefield(
        0,
        lands(0, 3).into_iter().chain([
            ready(card("Lithoform Engine").unwrap()),
            with_counters(
                ready(card("Hangarback Walker").unwrap()),
                CounterKind::PlusOnePlusOne,
                1,
            ),
        ]),
    )
}

#[rstest]
fn lithoform_engine_copies_an_activated_ability_without_activating_it(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = walker_game(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(walker_game(seats))?;
    driver.begin_ability("Hangarback Walker", 2)?;
    driver.pay_tapping(&["Mountain"], 0)?;
    let walker_ability = *driver.state().stack().last().unwrap();
    let target = Target::Object(driver.state().object(walker_ability).unwrap().reference());
    driver.begin_ability("Lithoform Engine", 0)?;
    driver.choose_targets(&[target])?;
    driver.pay_tapping(&["Mountain", "Mountain"], 0)?;
    pass_round(&mut driver, seats)?;
    let copies = ability_copies(&driver);
    assert_eq!(
        copies.len(),
        1,
        "the copy waits above the ability it copies"
    );
    let copy = driver.state().object(copies[0]).unwrap();
    assert_eq!(copy.copied_from().unwrap().id, walker_ability);
    assert_eq!(
        copy.ability().unwrap().source(),
        driver
            .state()
            .object(walker_ability)
            .unwrap()
            .ability()
            .unwrap()
            .source()
    );
    run(&mut driver, seats)?;
    let walker = driver.id_of("Hangarback Walker")?;
    assert_eq!(
        driver
            .state()
            .object(walker)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne),
        3,
        "the counter it had and one from the Walker and one from the copy"
    );
    let activated = driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, Event::AbilityActivated { .. }))
        .count();
    assert_eq!(
        activated, 2,
        "the Walker's and the Engine's, and not the copy's"
    );
    replays(&driver, &setup);
    Ok(())
}

fn corps_game(seats: usize) -> Scenario {
    Scenario::new(seats)
        .hand(0, [card("Armament Corps").unwrap()])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(swamp()),
                ready(forest()),
                ready(forest()),
                ready(forest()),
                ready(island()),
                ready(island()),
                ready(card("Strionic Resonator").unwrap()),
                ready(card("Grizzly Bears").unwrap()),
            ],
        )
}

#[rstest]
fn strionic_resonator_copies_a_triggered_ability_with_new_targets_and_the_division_moves_to_them(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = corps_game(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(corps_game(seats))?;
    driver.begin_cast("Armament Corps")?;
    driver.pay_tapping(&["Plains", "Swamp", "Forest", "Forest", "Forest"], 0)?;
    pass_round(&mut driver, seats)?;
    // The enters trigger asks for its targets.
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("{:?}", driver.decision()?.kind.name());
    };
    let corps = *options
        .legal
        .iter()
        .find(|t| {
            driver
                .state()
                .object(t.object().unwrap().id)
                .unwrap()
                .card()
                .name
                == "Armament Corps"
        })
        .unwrap();
    driver.choose_targets(&[corps])?;
    let trigger = *driver.state().stack().last().unwrap();
    let target = Target::Object(driver.state().object(trigger).unwrap().reference());
    let triggered_before = driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, Event::AbilityTriggered(_)))
        .count();
    driver.begin_activation("Strionic Resonator")?;
    driver.choose_targets(&[target])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    pass_round(&mut driver, seats)?;
    let DecisionKind::YesNo(question) = &driver.decision()?.kind else {
        panic!("{:?}", driver.decision()?.kind.name());
    };
    assert_eq!(
        question.text,
        "Choose new targets for the copy of Armament Corps?"
    );
    say(&mut driver, true)?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("{:?}", driver.decision()?.kind.name());
    };
    assert_eq!(
        (options.min, options.max),
        (1, 1),
        "as many targets as the original"
    );
    let bears = *options
        .legal
        .iter()
        .find(|t| {
            driver
                .state()
                .object(t.object().unwrap().id)
                .unwrap()
                .card()
                .name
                == "Grizzly Bears"
        })
        .unwrap();
    driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![bears])))?;
    let copy = driver.state().object(ability_copies(&driver)[0]).unwrap();
    assert_eq!(copy.choices().targets, vec![vec![bears]]);
    assert_eq!(
        copy.choices().division,
        vec![(bears, 2)],
        "the two counters follow the target"
    );
    run(&mut driver, seats)?;
    let counters = |driver: &Driver, name: &str| {
        let id = driver.id_of(name).unwrap();
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&CounterKind::PlusOnePlusOne)
    };
    assert_eq!(
        (
            counters(&driver, "Armament Corps"),
            counters(&driver, "Grizzly Bears")
        ),
        (2, 2)
    );
    let triggered_after = driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, Event::AbilityTriggered(_)))
        .count();
    assert_eq!(
        triggered_before, triggered_after,
        "the copy did not trigger"
    );
    replays(&driver, &setup);
    Ok(())
}

fn cytoshape_game(seats: usize) -> Scenario {
    Scenario::new(seats)
        .hand(0, [card("Cytoshape").unwrap()])
        .battlefield(
            0,
            [
                ready(forest()),
                ready(island()),
                ready(island()),
                ready(card("Grizzly Bears").unwrap()),
            ],
        )
        .battlefield(
            1,
            [with_counters(
                ready(card("Hero of Bladehold").unwrap()),
                CounterKind::PlusOnePlusOne,
                1,
            )],
        )
}

#[rstest]
fn cytoshape_makes_a_creature_a_copy_until_the_end_of_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = cytoshape_game(seats).setup().unwrap();
    let mut driver = Driver::from_scenario(cytoshape_game(seats))?;
    let bears = driver.id_of("Grizzly Bears")?;
    let hero = driver.id_of("Hero of Bladehold")?;
    let target = Target::Object(driver.state().object(bears).unwrap().reference());
    driver.cast_at("Cytoshape", &[target])?;
    pass_round(&mut driver, seats)?;
    let DecisionKind::ChooseCards(options) = &driver.decision()?.kind else {
        panic!("{:?}", driver.decision()?.kind.name());
    };
    assert!(options.from.contains(&hero));
    driver.answer(Answer::ChooseCards(CardsChoice(vec![hero])))?;
    run(&mut driver, seats)?;
    let object = driver.state().object(bears).unwrap();
    assert_eq!(object.card().name, "Hero of Bladehold");
    assert!(
        object
            .copy_effect()
            .is_some_and(mtg_engine::copies::CopyEffect::is_temporary)
    );
    let chars = characteristics(driver.state(), bears).unwrap();
    assert_eq!(
        (chars.power, chars.toughness),
        (Some(3), Some(4)),
        "none of its counters"
    );
    // The cleanup step ends it.
    driver.idle_until(2, Step::Upkeep)?;
    let object = driver.state().object(bears).unwrap();
    assert_eq!(object.card().name, "Grizzly Bears");
    assert!(object.copy_effect().is_none());
    let chars = characteristics(driver.state(), bears).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn hand_sizes_are_untouched_by_copies_of_spells_without_cards() -> Result<(), ScriptError> {
    // A copy is not a card: the graveyard and the hand hold the cards that were cast and nothing else.
    let setup = storm_game(2, 1, "Grapeshot").setup().unwrap();
    let mut driver = Driver::from_scenario(storm_game(2, 1, "Grapeshot"))?;
    cast_fillers(&mut driver, 1, 2)?;
    let foe = driver.target_player(1);
    driver.begin_cast("Grapeshot")?;
    driver.choose_targets(&[foe])?;
    driver.pay_tapping(&["Mountain", "Mountain"], 0)?;
    keep_targets_until_done(&mut driver, 2)?;
    assert_eq!(hand(&driver, 0), 0);
    assert_eq!(
        common::names(driver.state(), Zone::Graveyard(pid(0))).len(),
        2,
        "the Filler and Grapeshot"
    );
    replays(&driver, &setup);
    let _ = (Target::Player(pid(0)), PayChoice::Auto);
    Ok(())
}
