//! Goal: a cost reduction that depends on a target (a creature dealt damage this turn, an attacking creature, a legendary creature, any creature, the Scion itself), on what one opponent has (seven lands, seven cards in a graveyard) or on a player's life (half of the starting life), on what happened this turn (life gained, creatures that died or attacked, the second spell cast), on a kind of spell (Aura and Equipment, a creature with power 4 or greater, the chosen creature type) or on the creatures attacking the caster holds exactly while the fact does, whoever else has it, judges the targets that were chosen (not any that could be), stacks with the floor of the generic part, and lasts only this turn, at every table size.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::common::casting::lands;
use common::pid;
use mtg_core::mana::ManaCost;
use mtg_engine::PresetPermanent;
use mtg_engine::choose::{ChoiceOption, OptionChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The table as seat 0 sees it: the hand `hand`, four lands of every kind, the permanents `mine` and, on the seats given,
/// the permanents `theirs`; it is the second turn of seat 0, at the start of its combat.
fn table(
    seats: usize,
    hand: &[&str],
    mine: Vec<PresetPermanent>,
    theirs: Vec<(usize, Vec<PresetPermanent>)>,
) -> Scenario {
    let mut battlefield = lands(4, island);
    battlefield.extend(lands(4, swamp));
    battlefield.extend(lands(4, forest));
    battlefield.extend(lands(4, plains));
    battlefield.extend(lands(4, mountain));
    battlefield.extend(mine);
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .hand_named(0, hand.iter().copied())
        .battlefield(0, battlefield);
    for (seat, permanents) in theirs {
        scenario = scenario.battlefield(seat, permanents);
    }
    scenario
}

/// The cost as the engine writes it.
fn cost(text: &str) -> String {
    ManaCost::parse(text).unwrap().to_string()
}

/// `count` ready copies of a corpus permanent.
fn copies(name: &str, count: usize) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(card(name).unwrap())).collect()
}

/// The total cost at the payment of casting `name` from the priority of seat 0.
fn cost_of(driver: &mut Driver, name: &str) -> Result<String, ScriptError> {
    driver.begin_cast(name)?;
    let paid = driver.pay_options()?.cost.to_string();
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    Ok(paid)
}

/// The total cost at the payment of casting `name` at the permanent called `target`.
fn cost_at(driver: &mut Driver, name: &str, target: Target) -> Result<String, ScriptError> {
    driver.begin_cast(name)?;
    driver.choose_targets(&[target])?;
    let paid = driver.pay_options()?.cost.to_string();
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    Ok(paid)
}

/// Everyone passes: the top of the stack resolves.
fn resolve(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        driver.pass()?;
    }
    Ok(())
}

/// Passes until seat `seat` has the decision.
fn until(driver: &mut Driver, seat: usize) -> Result<(), ScriptError> {
    for _ in 0..8 {
        if driver.decision()?.player == pid(seat) {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::seven_lands_of_an_opponent(0, 7, "{R}{R}")]
#[case::six_lands_of_an_opponent(0, 6, "{6}{R}{R}")]
#[case::my_own_seven_lands(7, 0, "{6}{R}{R}")]
fn seven_lands_of_one_opponent_cheapen_the_avatar(
    #[case] mine: usize,
    #[case] theirs: usize,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let others = if theirs == 0 {
        vec![]
    } else {
        vec![(seats - 1, lands(theirs, mountain))]
    };
    let scenario = table(seats, &["Avatar of Fury"], lands(mine, mountain), others);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(cost_of(&mut driver, "Avatar of Fury")?, cost(expected));
    Ok(())
}

#[rstest]
fn lands_spread_over_two_opponents_do_not_cheapen_the_avatar(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let others = vec![(1, lands(4, mountain)), (2, lands(3, mountain))];
    let mut driver = Driver::from_scenario(table(seats, &["Avatar of Fury"], vec![], others))?;
    assert_eq!(cost_of(&mut driver, "Avatar of Fury")?, cost("{6}{R}{R}"));
    Ok(())
}

#[rstest]
#[case::seven_cards_of_an_opponent(Some(1), 7, "{2}{U}{U}")]
#[case::six_cards_of_an_opponent(Some(1), 6, "{5}{U}{U}")]
#[case::seven_cards_of_mine(None, 7, "{5}{U}{U}")]
fn seven_cards_in_one_opponents_graveyard_cheapen_the_draw(
    #[case] holder: Option<usize>,
    #[case] cards: usize,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = table(seats, &["Into the Story"], vec![], vec![]);
    scenario = match holder {
        Some(_) => scenario.graveyard_named(seats - 1, vec!["Grizzly Bears"; cards]),
        None => scenario.graveyard_named(0, vec!["Grizzly Bears"; cards]),
    };
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(cost_of(&mut driver, "Into the Story")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::an_opponent_at_half(Some((1, 10)), "{1}{B}{B}")]
#[case::an_opponent_above_half(Some((1, 11)), "{3}{B}{B}")]
#[case::the_caster_at_half(Some((0, 10)), "{1}{B}{B}")]
#[case::everyone_at_the_start(None, "{3}{B}{B}")]
fn a_life_total_at_half_the_starting_life_cheapens_game_over(
    #[case] life: Option<(usize, i32)>,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = table(seats, &["Game Over"], vec![], vec![]);
    if let Some((seat, total)) = life {
        scenario = scenario.life(if seat == 0 { 0 } else { seats - 1 }, total);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(cost_of(&mut driver, "Game Over")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::damaged_this_turn(true, "{B}")]
#[case::not_damaged(false, "{3}{B}")]
fn a_creature_dealt_damage_this_turn_cheapens_the_removal(
    #[case] bolted: bool,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        &["Lightning Bolt", "Bitter Downfall"],
        vec![],
        vec![(1, copies("Bogardan Hellkite", 1))],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Bogardan Hellkite")?;
    if bolted {
        driver.begin_cast("Lightning Bolt")?;
        driver.choose_targets(&[target])?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        resolve(&mut driver, seats)?;
    }
    let target = driver.target_named("Bogardan Hellkite")?;
    assert_eq!(
        cost_at(&mut driver, "Bitter Downfall", target)?,
        cost(expected)
    );
    Ok(())
}

#[rstest]
#[case::an_attacking_creature("Grizzly Bears", "{U}")]
#[case::a_creature_that_stays_home("Bogardan Hellkite", "{2}{U}")]
fn an_attacking_creature_as_the_target_cheapens_the_bounce(
    #[case] target: &str,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        &["Ephara's Dispersal"],
        [copies("Grizzly Bears", 1), copies("Bogardan Hellkite", 1)].concat(),
        vec![],
    )
    .at(2, 0, Step::DeclareAttackers);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Grizzly Bears", 1)])?;
    let target = driver.target_named(target)?;
    assert_eq!(
        cost_at(&mut driver, "Ephara's Dispersal", target)?,
        cost(expected)
    );
    Ok(())
}

#[rstest]
#[case::a_legendary_creature("Goreclaw, Terror of Qal Sisma", "{1}{B}")]
#[case::an_ordinary_creature("Grizzly Bears", "{3}{B}")]
fn a_legendary_creature_as_the_target_cheapens_the_removal(
    #[case] target: &str,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        &["Price of Fame"],
        vec![],
        vec![(1, copies(target, 1))],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named(target)?;
    assert_eq!(
        cost_at(&mut driver, "Price of Fame", target)?,
        cost(expected)
    );
    Ok(())
}

#[rstest]
#[case::the_killian_of_the_caster(0, "Grizzly Bears", "{1}{B}")]
#[case::the_killian_of_an_opponent(1, "Grizzly Bears", "{3}{B}")]
fn spells_that_target_a_creature_cost_less_for_the_controller_of_killian(
    #[case] holder: usize,
    #[case] target: &str,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let killian = copies("Killian, Ink Duelist", 1);
    let (mine, others) = if holder == 0 {
        (killian, vec![(1, copies(target, 1))])
    } else {
        (vec![], vec![(1, [killian, copies(target, 1)].concat())])
    };
    let mut driver = Driver::from_scenario(table(seats, &["Price of Fame"], mine, others))?;
    let target = driver.target_named(target)?;
    assert_eq!(
        cost_at(&mut driver, "Price of Fame", target)?,
        cost(expected)
    );
    Ok(())
}

#[rstest]
#[case::the_scion_of_the_caster(0, "{1}{B}")]
#[case::the_scion_of_an_opponent(1, "{5}{B}")]
fn spells_that_target_the_scion_cost_less_or_more(
    #[case] holder: usize,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scion = copies("Elderwood Scion", 1);
    let (mine, others) = if holder == 0 {
        (scion, vec![])
    } else {
        (vec![], vec![(seats - 1, scion)])
    };
    let mut driver = Driver::from_scenario(table(seats, &["Price of Fame"], mine, others))?;
    let target = driver.target_named("Elderwood Scion")?;
    assert_eq!(
        cost_at(&mut driver, "Price of Fame", target)?,
        cost(expected)
    );
    Ok(())
}

#[rstest]
#[case::no_life_gained(0, "{6}{R}{R}")]
#[case::two_life_gained(1, "{4}{R}{R}")]
#[case::more_than_the_generic_part(2, "{R}{R}")]
fn life_gained_this_turn_cheapens_a_creature_spell_by_the_amount(
    #[case] gains: usize,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hand: &[&str] = if gains == 2 {
        &["Renewed Faith", "Bogardan Hellkite"]
    } else {
        &["Bogardan Hellkite"]
    };
    let mine = copies("Ezzaroot Channeler", 1);
    let mut driver = Driver::from_scenario(table(seats, hand, mine, vec![]))?;
    if gains >= 1 {
        driver.begin_activation("Ezzaroot Channeler")?;
        resolve(&mut driver, seats)?;
    }
    if gains == 2 {
        driver.begin_cast("Renewed Faith")?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        resolve(&mut driver, seats)?;
    }
    assert_eq!(cost_of(&mut driver, "Bogardan Hellkite")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::nothing_died(0, "{8}{B}{B}{R}")]
#[case::one_died(1, "{7}{B}{B}{R}")]
#[case::two_died(2, "{6}{B}{B}{R}")]
fn each_creature_that_died_this_turn_cheapens_blood_for_the_blood_god(
    #[case] deaths: usize,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let bolts = vec!["Lightning Bolt"; deaths];
    let mut hand = bolts.clone();
    hand.push("Blood for the Blood God!");
    let theirs = vec![(seats - 1, copies("Grizzly Bears", deaths))];
    let mut driver = Driver::from_scenario(table(seats, &hand, vec![], theirs))?;
    for _ in 0..deaths {
        driver.begin_cast("Lightning Bolt")?;
        let target = driver.target_named("Grizzly Bears")?;
        driver.choose_targets(&[target])?;
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
        resolve(&mut driver, seats)?;
    }
    assert_eq!(
        cost_of(&mut driver, "Blood for the Blood God!")?,
        cost(expected)
    );
    Ok(())
}

#[rstest]
#[case::no_attacker(0, "{6}{U}")]
#[case::one_attacker(1, "{5}{U}")]
#[case::two_attackers(2, "{4}{U}")]
fn each_creature_that_attacked_this_turn_cheapens_rowdy_research(
    #[case] attackers: usize,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(
        seats,
        &["Rowdy Research"],
        copies("Grizzly Bears", 2),
        vec![],
    )
    .at(2, 0, Step::DeclareAttackers);
    let mut driver = Driver::from_scenario(scenario)?;
    let attacks: Vec<(&str, usize)> = vec![("Grizzly Bears", 1); attackers];
    driver.attack(&attacks)?;
    assert_eq!(cost_of(&mut driver, "Rowdy Research")?, cost(expected));
    Ok(())
}

#[rstest]
#[case::an_aura("Pacifism", "{W}")]
#[case::another_spell("Grizzly Bears", "{1}{G}")]
fn aura_and_equipment_spells_cost_less_with_danitha(
    #[case] spell: &str,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mine = copies("Danitha Capashen, Paragon", 1);
    let mut driver = Driver::from_scenario(table(seats, &[spell], mine, vec![]))?;
    if spell == "Pacifism" {
        let target = driver.target_named("Danitha Capashen, Paragon")?;
        assert_eq!(cost_at(&mut driver, spell, target)?, cost(expected));
    } else {
        assert_eq!(cost_of(&mut driver, spell)?, cost(expected));
    }
    Ok(())
}

#[rstest]
#[case::power_five_mine(0, "Bogardan Hellkite", "{4}{R}{R}")]
#[case::power_two_mine(0, "Grizzly Bears", "{1}{G}")]
#[case::power_five_cast_beside_an_opponents(1, "Bogardan Hellkite", "{6}{R}{R}")]
fn creature_spells_with_power_four_or_greater_cost_less_with_goreclaw(
    #[case] holder: usize,
    #[case] spell: &str,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let goreclaw = copies("Goreclaw, Terror of Qal Sisma", 1);
    let (mine, others) = if holder == 0 {
        (goreclaw, vec![])
    } else {
        (vec![], vec![(1, goreclaw)])
    };
    let mut driver = Driver::from_scenario(table(seats, &[spell], mine, others))?;
    assert_eq!(cost_of(&mut driver, spell)?, cost(expected));
    Ok(())
}

/// Answers the pending choice of a creature type with the option called `label`.
fn choose_type(driver: &mut Driver, label: &str) -> Result<(), ScriptError> {
    for _ in 0..8 {
        if let DecisionKind::ChooseOption(options) = &driver.decision()?.kind {
            let at = options
                .options
                .iter()
                .position(|option| matches!(option, ChoiceOption::Label(found) if found == label))
                .unwrap();
            driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::the_chosen_type("Grizzly Bears", "{G}")]
#[case::another_type("Aven Squire", "{1}{W}")]
fn creature_spells_of_the_chosen_type_cost_less_for_everyone_with_the_incubator(
    #[case] spell: &str,
    #[case] expected: &str,
    #[values(2)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &["Urza's Incubator", spell], vec![], vec![])
        .graveyard_named(seats - 1, ["Grizzly Bears"]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Urza's Incubator")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    choose_type(&mut driver, "Bear")?;
    assert_eq!(cost_of(&mut driver, spell)?, cost(expected));
    Ok(())
}

#[rstest]
#[case::the_chosen_type_loses_the_colored_symbols("Grizzly Bears", "{1}")]
#[case::another_type("Aven Squire", "{1}{W}")]
fn spells_of_the_chosen_type_lose_one_of_each_color_with_morophon(
    #[case] spell: &str,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &["Morophon, the Boundless", spell], vec![], vec![])
        .graveyard_named(seats - 1, ["Grizzly Bears"]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Morophon, the Boundless")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    choose_type(&mut driver, "Bear")?;
    assert_eq!(cost_of(&mut driver, spell)?, cost(expected));
    Ok(())
}

#[rstest]
#[case::no_attacker(0, "{4}{W}")]
#[case::one_attacker(1, "{3}{W}")]
#[case::two_attackers(2, "{2}{W}")]
fn each_creature_attacking_the_caster_cheapens_the_sentry_bot(
    #[case] attackers: usize,
    #[case] expected: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, &["Sentry Bot"], vec![], vec![])
        .at(2, 1, Step::DeclareAttackers)
        .battlefield(1, copies("Grizzly Bears", 2));
    let mut driver = Driver::from_scenario(scenario)?;
    let attacks: Vec<(&str, usize)> = vec![("Grizzly Bears", 0); attackers];
    driver.attack(&attacks)?;
    until(&mut driver, 0)?;
    assert_eq!(cost_of(&mut driver, "Sentry Bot")?, cost(expected));
    Ok(())
}
