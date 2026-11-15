//! Goal: the r64-04 cards play as they read at tables of two, three and four seats: Grist is a 1/1 Insect creature in
//! every zone but the battlefield (and only a planeswalker there), Rigo draws when its controller attacks a player or a
//! planeswalker with a creature of power 1 or less and not with a bigger one, Heart of Kiran crews by removing a loyalty
//! counter from a planeswalker its controller has (and not without one), and Advancing the Spirit makes the first
//! power-up of each of its controller's turns cost {0}.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::ObjectId;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{characteristics, replay};
use mtg_testkit::cards::{free_creature, walker};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// The creature type and size of the named card as the layers leave it.
fn shape(driver: &Driver, name: &str) -> (bool, bool, Option<(i32, i32)>) {
    let id = driver
        .state()
        .objects()
        .find(|object| object.card().name == name)
        .map(mtg_engine::object::GameObject::id)
        .unwrap();
    let chars = characteristics(driver.state(), id).unwrap();
    let insect = chars.subtypes.contains(&Subtype("Insect".to_owned()));
    let stats = chars.power.zip(chars.toughness);
    (chars.is_creature(), insect, stats)
}

#[rstest]
fn grist_is_a_one_one_insect_creature_everywhere_but_on_the_battlefield(
    #[values(2, 3, 4)] seats: usize,
    #[values("hand", "library", "graveyard")] zone: &str,
) -> Result<(), ScriptError> {
    let grist = ["Grist, the Hunger Tide"];
    let scenario = Scenario::new(seats);
    let scenario = match zone {
        "hand" => scenario.hand_named(0, grist),
        "library" => scenario.library_named(0, grist),
        _ => scenario.graveyard_named(0, grist),
    };
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(
        shape(&driver, grist[0]),
        (true, true, Some((1, 1))),
        "in the {zone}"
    );
    Ok(())
}

#[rstest]
fn grist_on_the_battlefield_is_only_a_planeswalker(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(seats).battlefield_named(0, ["Grist, the Hunger Tide"]),
    )?;
    let chars = characteristics(driver.state(), driver.id_of("Grist, the Hunger Tide")?).unwrap();
    assert!(!chars.is_creature());
    assert!(!chars.subtypes.contains(&Subtype("Insect".to_owned())));
    assert!(chars.types.contains(CardType::Planeswalker));
    Ok(())
}

/// Seat 0 attacks seat 1 (or a planeswalker of seat 1) with one creature of this power while Rigo stays home; how many
/// cards seat 0 holds afterwards, less those it started with.
fn rigo_draws(seats: usize, power: i32, at_walker: bool) -> Result<i64, ScriptError> {
    let build = || {
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(card("Rigo, Streetwise Mentor").unwrap()),
                    ready(free_creature("Raider", power, 2)),
                ],
            )
            .battlefield(1, [ready(walker("Walker", 0))])
            .library_named(0, vec!["Grizzly Bears"; 5])
    };
    let setup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    let before = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.pass_until(Step::DeclareAttackers)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("seat 0 declares attackers");
    };
    let raider: ObjectId = options
        .attackers
        .iter()
        .map(|attacker| attacker.object)
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == "Raider")
        })
        .unwrap();
    let target = if at_walker {
        AttackTarget::Planeswalker(driver.id_of("Walker")?)
    } else {
        AttackTarget::Player(pid(1))
    };
    driver.answer(Answer::DeclareAttackers(vec![(raider, target)]))?;
    for _ in 0..50 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            break;
        }
        driver.answer_passively()?;
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let after = driver.state().zone(Zone::Hand(pid(0))).len();
    Ok(i64::try_from(after).unwrap() - i64::try_from(before).unwrap())
}

#[rstest]
#[case::power_one_at_a_player(1, false, 1)]
#[case::power_zero_at_a_player(0, false, 1)]
#[case::power_one_at_a_planeswalker(1, true, 1)]
#[case::power_two_at_a_player(2, false, 0)]
#[case::power_two_at_a_planeswalker(2, true, 0)]
fn rigo_draws_for_an_attack_with_a_creature_of_power_one_or_less(
    #[values(2, 3, 4)] seats: usize,
    #[case] power: i32,
    #[case] at_walker: bool,
    #[case] drawn: i64,
) -> Result<(), ScriptError> {
    assert_eq!(rigo_draws(seats, power, at_walker)?, drawn);
    Ok(())
}

/// The rules of the card as an ability list, to look an index up.
fn rules_of(name: &str) -> Vec<std::sync::Arc<Ability>> {
    mtg_testkit::corpus::card(name).unwrap().rules.clone()
}

#[rstest]
fn heart_of_kiran_crews_by_removing_a_loyalty_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let alternative = rules_of("Heart of Kiran")
        .iter()
        .position(|rule| matches!(&**rule, Ability::Activated { .. }))
        .unwrap();
    let build = |walkers: usize| {
        let mut permanents = vec![ready(card("Heart of Kiran").unwrap())];
        permanents.extend((0..walkers).map(|at| ready(walker(&format!("Walker {at}"), 0))));
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .battlefield(0, permanents)
    };
    // No planeswalker and no creature: nothing crews it.
    let driver = Driver::from_scenario(build(0))?;
    assert!(driver.offered_abilities("Heart of Kiran")?.is_empty());
    // A planeswalker: the alternative is offered, costs one loyalty and animates the Vehicle until end of turn.
    let mut driver = Driver::from_scenario(build(1))?;
    assert!(
        driver
            .offered_abilities("Heart of Kiran")?
            .contains(&alternative)
    );
    let walker_id = driver.id_of("Walker 0")?;
    let loyalty = |driver: &Driver| {
        driver
            .state()
            .object(walker_id)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&mtg_cards::value::CounterKind::Loyalty)
    };
    let before = loyalty(&driver);
    driver.begin_ability("Heart of Kiran", alternative)?;
    for _ in 0..50 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(loyalty(&driver), before - 1);
    let chars = characteristics(driver.state(), driver.id_of("Heart of Kiran")?).unwrap();
    assert!(chars.is_creature());
    Ok(())
}

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_testkit::cards::land;
use mtg_testkit::corpus::card;
use mtg_testkit::with_counters;

/// The +1/+1 counters on the named permanent.
fn counters_on(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

/// Settles the stack, answering "yes" to a may and picking the creature named next of `picks` for each choice among
/// creatures; the number of choices asked.
fn settle_with(driver: &mut Driver, picks: &[&str]) -> Result<usize, ScriptError> {
    let mut asked = 0;
    for _ in 0..200 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(asked);
        }
        match driver.engine().decision().map(|d| d.kind.clone()) {
            Some(DecisionKind::YesNo(_)) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            Some(DecisionKind::ChooseCards(options)) => {
                let name = picks[asked];
                let wanted = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| driver.state().object(*id).unwrap().card().name == name)
                    .unwrap_or_else(|| panic!("{name} is offered"));
                asked += 1;
                driver.answer(Answer::ChooseCards(CardsChoice(vec![wanted])))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::all_on_one(&["Bear A", "Bear A", "Bear A"], 3, 0)]
#[case::split(&["Bear A", "Bear B", "Bear B"], 1, 2)]
#[case::on_the_other(&["Bear B", "Bear B", "Bear B"], 0, 3)]
fn vastwood_hydra_distributes_its_counters_among_your_creatures_when_it_dies(
    #[values(2, 3, 4)] seats: usize,
    #[case] picks: &[&str],
    #[case] first: u32,
    #[case] second: u32,
) -> Result<(), ScriptError> {
    let bear_a = free_creature("Bear A", 2, 2);
    let bear_b = free_creature("Bear B", 2, 2);
    let scenario = Scenario::new(seats)
        .hand(0, [card("Lightning Bolt").unwrap()])
        .battlefield(
            0,
            [
                ready(land(Color::Red)),
                with_counters(
                    ready(card("Vastwood Hydra").unwrap()),
                    CounterKind::PlusOnePlusOne,
                    3,
                ),
                ready(bear_a),
                ready(bear_b),
            ],
        )
        .battlefield(1, [ready(free_creature("Foe Bear", 2, 2))]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Vastwood Hydra")?;
    driver.cast_at("Lightning Bolt", &[target])?;
    let asked = settle_with(&mut driver, picks)?;
    assert_eq!(asked, 3, "one choice for each counter");
    assert_eq!(counters_on(&driver, "Bear A"), first);
    assert_eq!(counters_on(&driver, "Bear B"), second);
    assert_eq!(counters_on(&driver, "Foe Bear"), 0);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::PayChoice;

/// Casts `spell` with X = `x`, answering the choices among creatures with `picks` in turn; how many were asked.
fn cast_with_x(
    driver: &mut Driver,
    spell: &str,
    x: u32,
    picks: &[&str],
) -> Result<usize, ScriptError> {
    driver.begin_cast(spell)?;
    for _ in 0..50 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseNumber(_) => {
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => break,
        }
    }
    settle_with(driver, picks)
}

#[rstest]
fn stumpsquall_hydra_puts_every_counter_on_itself_when_no_commander_is_in_play(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 4)] x: u32,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand_named(0, ["Stumpsquall Hydra"])
        .battlefield(0, vec![ready(land(Color::Green)); 8]);
    let mut driver = Driver::from_scenario(scenario)?;
    // Only the Hydra can take a counter, so no one is asked.
    assert_eq!(cast_with_x(&mut driver, "Stumpsquall Hydra", x, &[])?, 0);
    assert_eq!(counters_on(&driver, "Stumpsquall Hydra"), x);
    Ok(())
}

#[rstest]
#[case::all_on_the_commander(&["Cmd"], 1, 1)]
#[case::all_on_the_hydra(&["Stumpsquall Hydra"], 2, 0)]
fn stumpsquall_hydra_always_keeps_one_counter_and_shares_the_rest_with_commanders(
    #[values(2, 3, 4)] seats: usize,
    #[case] picks: &[&str],
    #[case] hydra: u32,
    #[case] commander: u32,
) -> Result<(), ScriptError> {
    let cmd = mtg_testkit::commander_cards::legend("Cmd", 1, &[Color::Green], 2, 2);
    let scenario = Scenario::new(seats)
        .format(std::sync::Arc::new(mtg_engine::format::Commander))
        .commanders(0, [cmd])
        .hand_named(0, ["Stumpsquall Hydra"])
        .battlefield(0, vec![ready(land(Color::Green)); 8]);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cmd")?;
    settle_with(&mut driver, &[])?;
    // X = 2: one counter on the Hydra first, the second wherever the caster chooses.
    let asked = cast_with_x(&mut driver, "Stumpsquall Hydra", 2, picks)?;
    assert_eq!(asked, 1);
    assert_eq!(counters_on(&driver, "Stumpsquall Hydra"), hydra);
    assert_eq!(counters_on(&driver, "Cmd"), commander);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

/// The index of the power-up ability among the rules of the card.
fn power_up_of(name: &str) -> usize {
    rules_of(name)
        .iter()
        .position(|rule| {
            matches!(
                &**rule,
                Ability::Activated {
                    limit: Some(mtg_cards::ability::Limit::PowerUp),
                    ..
                }
            )
        })
        .unwrap()
}

#[rstest]
fn hulk_makes_the_power_ups_of_other_creatures_cost_three_less(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] hulks: usize,
    #[values(3, 4)] mountains: usize,
) -> Result<(), ScriptError> {
    let index = power_up_of("Wonder Man, Hollywood Hero");
    let mut board = vec![ready(card("Wonder Man, Hollywood Hero").unwrap())];
    board.extend((0..hulks).map(|_| ready(card("Hulk, Gamma Goliath").unwrap())));
    board.extend((0..mountains).map(|_| ready(land(Color::Red))));
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .battlefield(0, board),
    )?;
    // {5}{R}{R} less {3} is {2}{R}{R}: four lands, and only with Hulk.
    let offered = driver
        .offered_abilities("Wonder Man, Hollywood Hero")?
        .contains(&index);
    assert_eq!(offered, hulks == 1 && mountains == 4);
    Ok(())
}

#[rstest]
fn advancing_the_spirit_makes_the_first_power_up_each_turn_free(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] spirit: bool,
) -> Result<(), ScriptError> {
    let first = power_up_of("Wonder Man, Hollywood Hero");
    let second = power_up_of("Kang the Conqueror");
    let mut board = vec![
        ready(card("Wonder Man, Hollywood Hero").unwrap()),
        ready(card("Kang the Conqueror").unwrap()),
    ];
    if spirit {
        board.push(ready(card("Advancing the Spirit").unwrap()));
    }
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(4, 0, Step::Main1)
            .battlefield(0, board),
    )?;
    assert_eq!(
        driver
            .offered_abilities("Wonder Man, Hollywood Hero")?
            .contains(&first),
        spirit,
        "free only with the Enchantment"
    );
    if spirit {
        driver.begin_ability("Wonder Man, Hollywood Hero", first)?;
        // The whole cost is {0}: paying asks nothing of the lands, and the player has none.
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        settle_with(&mut driver, &[])?;
        assert_eq!(counters_on(&driver, "Wonder Man, Hollywood Hero"), 2);
        // The second power-up of the turn pays its cost.
        assert!(
            !driver
                .offered_abilities("Kang the Conqueror")?
                .contains(&second)
        );
    }
    Ok(())
}
