//! Goal: cost increases and reductions (CR 601.2f) change the total cost of a spell in the order of the rules (the printed cost, then X, then the increases, then the reductions) whoever controls the modifier and whoever casts the spell, a reduction takes generic mana down to nothing and a named colour only where the cost has it, hybrid and Phyrexian symbols stay beside the modifiers, a spell is offered only when its modified cost can be paid, a cost that grows after the targets are chosen rewinds the announcement, the corpus cards Fireball, Thalia and Foundry Inspector play as printed, and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{NumberChoice, Target};
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event, GameSetup, replay};
use mtg_testkit::cards::{bear, forest, island, mountain, plains};
use mtg_testkit::cost_cards::{modifier, spell};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A seat's lands: the first three of each kind of mana the spells below ask for, then one Island.
fn lands(count: usize) -> Vec<mtg_engine::PresetPermanent> {
    let kinds: [fn() -> std::sync::Arc<mtg_cards::CardDefinition>; 4] =
        [mountain, forest, plains, island];
    (0..count).map(|at| ready(kinds[at % 4]())).collect()
}

fn permanents(names: &[&str]) -> Vec<mtg_engine::PresetPermanent> {
    names
        .iter()
        .map(|name| ready(modifier(name).unwrap()))
        .collect()
}

/// `spell` in the hand of seat 0 with `lands` lands, the modifiers `mine` and `bears` bears on its side and the
/// modifiers `theirs` on the other.
fn scenario(
    cast: &str,
    mine: &[&str],
    theirs: &[&str],
    bears: usize,
    lands_of_mine: usize,
) -> Scenario {
    let mut seat = lands(lands_of_mine);
    seat.extend(permanents(mine));
    seat.extend((0..bears).map(|_| ready(bear())));
    Scenario::new(2)
        .hand(0, [spell(cast).unwrap()])
        .battlefield(0, seat)
        .battlefield(1, permanents(theirs))
}

fn cost(text: &str) -> ManaCost {
    ManaCost::parse(text).unwrap()
}

/// Casts `name`, aims it at the other player when it asks, and gives the cost the payment is asked for.
fn total_of(driver: &mut Driver, name: &str) -> Result<ManaCost, ScriptError> {
    driver.begin_cast(name)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        let foe = driver.target_player(1);
        driver.choose_targets(&[foe])?;
    }
    Ok(driver.pay_options()?.cost.clone())
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn taps(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, Event::PermanentTapped { .. }))
        .count()
}

// The total

#[rstest]
// Increases
#[case::an_opponents_tax_on_a_noncreature_spell("Bolt", &[], &["Tax Rock"], 0, "{1}{R}")]
#[case::my_own_tax("Bolt", &["Tax Rock"], &[], 0, "{1}{R}")]
#[case::several_taxes_add_up("Bolt", &["Tax Rock", "Sphere"], &["Tax Rock"], 0, "{3}{R}")]
#[case::a_creature_spell_is_not_a_noncreature_spell("Grizzly Bears", &["Tax Rock"], &["Tax Rock"], 0, "{1}{G}")]
#[case::every_spell_is_taxed("Grizzly Bears", &[], &["Sphere"], 0, "{2}{G}")]
#[case::opponents_pay_more_for_what_they_cast("Bolt", &[], &["Hostile Tax"], 0, "{1}{R}")]
#[case::not_what_i_cast("Bolt", &["Hostile Tax"], &[], 0, "{R}")]
#[case::a_coloured_increase("Bolt", &["Blue Tax"], &[], 0, "{U}{R}")]
#[case::a_coloured_increase_on_the_wrong_colour("Grizzly Bears", &["Blue Tax"], &[], 0, "{1}{G}")]
#[case::a_free_spell_is_taxed("Free Bear", &["Sphere"], &[], 0, "{1}")]
// Reductions
#[case::my_reduction("Big Bolt", &["My Helm"], &[], 0, "{2}{R}")]
#[case::everyones_reduction_applies_to_me("Big Bolt", &[], &["Helm"], 0, "{2}{R}")]
#[case::an_opponents_reduction_for_themselves_does_not("Big Bolt", &[], &["My Helm"], 0, "{3}{R}")]
#[case::two_reductions_add_up("Big Bolt", &["Helm", "My Helm"], &["Helm"], 0, "{R}")]
#[case::never_below_zero_generic("Big Bolt", &["Deep Helm", "Deep Helm"], &[], 0, "{R}")]
#[case::never_a_colour_by_a_generic_reduction("Bolt", &["Helm"], &[], 0, "{R}")]
#[case::a_colour_the_reduction_names("Bolt", &["Red Helm"], &[], 0, "")]
#[case::a_colour_beside_generic("Big Bolt", &["Red Helm"], &[], 0, "{3}")]
#[case::a_colour_the_cost_lacks("Grizzly Bears", &["Red Helm"], &[], 0, "{1}{G}")]
#[case::to_nothing("Trinket", &["Helm"], &[], 0, "")]
#[case::as_many_times_as_there_are_creatures("Big Bolt", &["Crowd Helm"], &[], 2, "{1}{R}")]
#[case::no_creature_no_reduction("Big Bolt", &["Crowd Helm"], &[], 0, "{3}{R}")]
#[case::more_creatures_than_generic("Big Bolt", &["Crowd Helm"], &[], 5, "{R}")]
#[case::only_while_the_condition_holds_on_my_turn("Big Bolt", &["Day Helm"], &[], 0, "{2}{R}")]
#[case::not_on_a_turn_that_is_not_its_controllers("Big Bolt", &[], &["Day Helm"], 0, "{3}{R}")]
// Both
#[case::increases_come_before_reductions("Bolt", &["Tax Rock", "Helm"], &[], 0, "{R}")]
#[case::a_reduction_cannot_use_up_what_an_increase_adds_later("Bolt", &["Sphere", "Deep Helm"], &[], 0, "{R}")]
#[case::an_increase_beside_a_deeper_reduction("Big Bolt", &["Sphere", "Deep Helm"], &["Tax Rock"], 0, "{3}{R}")]
// Hybrid and Phyrexian symbols
#[case::hybrid_stays_beside_a_reduction("Hybrid Bear", &["Helm"], &[], 0, "{G/W}{G/W}")]
#[case::hybrid_with_an_increase("Hybrid Bear", &["Sphere"], &[], 0, "{1}{G/W}{G/W}")]
#[case::phyrexian_loses_only_generic("Phyrexian Bear", &["Helm"], &[], 0, "{G/P}")]
#[case::phyrexian_with_an_increase("Phyrexian Bear", &["Sphere"], &[], 0, "{2}{G/P}")]
#[case::phyrexian_not_named_by_a_colour("Double Phyrexian", &["Red Helm", "Helm"], &[], 0, "{W/P}{W/P}")]
fn modifiers_change_the_total_cost_in_the_order_of_the_rules(
    #[case] cast: &str,
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] bears: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(scenario(cast, mine, theirs, bears, 12))?;
    let total = total_of(&mut driver, cast)?;
    assert_eq!(total, cost(expected), "{total} for {expected}");
    // What is paid is the total: a plain cost taps one land for each mana.
    if total.is_plain() {
        let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
        assert_eq!(
            taps(&events),
            usize::try_from(total.mana_value()).unwrap(),
            "{total}"
        );
    }
    Ok(())
}

#[test]
fn a_cast_under_modifiers_replays_to_the_live_state() -> Result<(), ScriptError> {
    let build = || scenario("Big Bolt", &["Deep Helm"], &["Tax Rock"], 0, 12);
    let setup = build().setup().unwrap();
    let mut driver = Driver::from_scenario(build())?;
    let total = total_of(&mut driver, "Big Bolt")?;
    assert_eq!(total, cost("{2}{R}"));
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_permanent_that_lost_its_abilities_changes_no_cost() -> Result<(), ScriptError> {
    let mut driver =
        Driver::from_scenario(scenario("Big Bolt", &["Deep Helm"], &["Sphere"], 0, 12))?;
    let sphere = driver.id_of("Sphere")?;
    let helm = driver.id_of("Deep Helm")?;
    for lost in [sphere, helm] {
        driver.add_effect(
            driver.on(lost),
            Modification::LoseAbilities,
            Duration::Forever,
        )?;
    }
    assert_eq!(total_of(&mut driver, "Big Bolt")?, cost("{3}{R}"));
    Ok(())
}

#[test]
fn a_modifier_cast_this_turn_changes_the_next_cost() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .hand(0, [modifier("Sphere").unwrap(), spell("Bolt").unwrap()])
        .battlefield(0, lands(6));
    let mut driver = Driver::from_scenario(scenario)?;
    // Before the Sphere is in play the Bolt costs its {R}; backing out leaves no trace.
    assert_eq!(total_of(&mut driver, "Bolt")?, cost("{R}"));
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    driver.cast("Sphere")?;
    driver.pass()?;
    driver.pass()?;
    assert_eq!(
        common::names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|name| *name == "Sphere")
            .count(),
        1
    );
    assert_eq!(total_of(&mut driver, "Bolt")?, cost("{1}{R}"));
    Ok(())
}

// What is offered

#[rstest]
#[case::enough_for_the_printed_cost("Bolt", &[], &[], 1, true)]
#[case::an_increase_makes_it_unaffordable("Bolt", &[], &["Tax Rock"], 1, false)]
#[case::an_increase_beside_enough_lands("Bolt", &[], &["Tax Rock"], 2, true)]
#[case::a_creature_is_not_taxed("Free Bear", &[], &["Tax Rock"], 0, true)]
#[case::an_artifact_is_taxed("Trinket", &[], &["Tax Rock"], 1, false)]
#[case::a_free_spell_is_taxed("Free Bear", &[], &["Sphere"], 0, false)]
#[case::a_reduction_makes_it_affordable("Big Bolt", &["Deep Helm"], &[], 2, true)]
#[case::a_reduction_that_is_not_enough("Big Bolt", &["Helm"], &[], 2, false)]
#[case::the_cost_before_is_not_enough("Big Bolt", &[], &[], 3, false)]
#[case::an_opponents_reduction_for_themselves("Big Bolt", &[], &["My Helm"], 3, false)]
fn a_spell_is_offered_only_when_its_modified_cost_can_be_paid(
    #[case] cast: &str,
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] lands_of_mine: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    // Only Mountains pay: {R} and then generic mana.
    let mut seat: Vec<_> = (0..lands_of_mine).map(|_| ready(mountain())).collect();
    seat.extend(permanents(mine));
    let scenario = Scenario::new(2)
        .hand(0, [spell(cast).unwrap()])
        .battlefield(0, seat)
        .battlefield(1, permanents(theirs));
    let mut driver = Driver::from_scenario(scenario)?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    assert_eq!(!options.castable.is_empty(), offered);
    assert_eq!(driver.begin_cast(cast).is_ok(), offered);
    Ok(())
}

// X

fn most_x(driver: &Driver) -> Option<u32> {
    match &driver.decision().unwrap().kind {
        DecisionKind::ChooseNumber(options) => options.max,
        _ => None,
    }
}

#[rstest]
#[case::no_modifier(&[], &[], 4, 3)]
#[case::a_reduction_adds_to_x(&["Helm"], &[], 4, 4)]
#[case::two_reductions(&["Helm", "My Helm"], &[], 4, 5)]
#[case::an_increase_takes_from_x(&[], &["Sphere"], 4, 2)]
#[case::an_increase_and_a_reduction(&["Helm"], &["Sphere"], 4, 3)]
#[case::the_colour_is_still_paid_for(&["Deep Helm"], &[], 4, 5)]
fn x_is_offered_up_to_what_the_modified_cost_allows(
    #[case] mine: &[&str],
    #[case] theirs: &[&str],
    #[case] mountains: usize,
    #[case] most: u32,
) -> Result<(), ScriptError> {
    let mut seat: Vec<_> = (0..mountains).map(|_| ready(mountain())).collect();
    seat.extend(permanents(mine));
    let scenario = Scenario::new(2)
        .hand(0, [spell("Blaze").unwrap()])
        .battlefield(0, seat)
        .battlefield(1, permanents(theirs));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Blaze")?;
    assert_eq!(most_x(&driver), Some(most));
    // The whole X is paid for with the generic mana the modifiers left over.
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(most)))?;
    let foe = driver.target_player(1);
    driver.choose_targets(&[foe])?;
    assert_eq!(
        driver.pay_options()?.cost.mana_value(),
        u32::try_from(mountains).unwrap()
    );
    Ok(())
}

// Fireball: the cost for each target beyond the first, X and the even division

fn fireball_with(mountains: usize, modifiers: &[&str]) -> Scenario {
    let mut seat: Vec<_> = (0..mountains).map(|_| ready(mountain())).collect();
    seat.extend(permanents(modifiers));
    Scenario::new(2)
        .hand_named(0, ["Fireball"])
        .battlefield(0, seat)
        .battlefield_named(1, ["Grizzly Bears", "Grizzly Bears"])
}

/// Casts Fireball for `x` at the other player and then at `creatures` of the bears on the other side.
fn aim_fireball(driver: &mut Driver, x: u32, creatures: usize) -> Result<(), ScriptError> {
    driver.begin_cast("Fireball")?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseNumber(_)) {
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    }
    let mut targets = vec![driver.target_player(1)];
    let state = driver.state();
    targets.extend(
        state
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| state.object(*id))
            .filter(|object| object.card().name == "Grizzly Bears")
            .take(creatures)
            .map(|object| Target::Object(object.reference())),
    );
    driver.choose_targets(&targets)?;
    Ok(())
}

#[rstest]
#[case::one_target_costs_nothing_more(0, "{3}{R}")]
#[case::two_targets_cost_one_more(1, "{4}{R}")]
#[case::three_targets_cost_two_more(2, "{5}{R}")]
fn fireball_costs_one_more_for_each_target_beyond_the_first(
    #[case] creatures: usize,
    #[case] expected: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(fireball_with(8, &[]))?;
    aim_fireball(&mut driver, 3, creatures)?;
    assert_eq!(driver.pay_options()?.cost, cost(expected));
    Ok(())
}

#[test]
fn fireball_divides_its_x_evenly_among_the_targets_it_was_paid_for() -> Result<(), ScriptError> {
    let setup = fireball_with(8, &[]).setup().unwrap();
    let mut driver = Driver::from_scenario(fireball_with(8, &[]))?;
    aim_fireball(&mut driver, 4, 1)?;
    assert_eq!(driver.pay_options()?.cost, cost("{5}{R}"));
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    driver.pass()?;
    driver.pass()?;
    assert_eq!(
        driver.state().player(pid(1)).unwrap().life(),
        18,
        "4 damage divided between two targets is 2 each"
    );
    assert_eq!(
        common::names(driver.state(), mtg_engine::zones::Zone::Battlefield)
            .iter()
            .filter(|name| *name == "Grizzly Bears")
            .count(),
        1,
        "the bear dealt 2 died"
    );
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_reduction_pays_for_the_targets_beyond_the_first() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(fireball_with(8, &["Helm"]))?;
    aim_fireball(&mut driver, 3, 1)?;
    assert_eq!(driver.pay_options()?.cost, cost("{3}{R}"));
    Ok(())
}

#[test]
fn a_total_the_player_cannot_pay_after_the_targets_rewinds_the_announcement()
-> Result<(), ScriptError> {
    // Four lands pay X = 3 and the {R}; a second target costs {1} more, which they cannot pay.
    let mut driver = Driver::from_scenario(fireball_with(4, &[]))?;
    let before = (driver.engine().events().len(), driver.engine().state_hash());
    aim_fireball(&mut driver, 3, 1)?;
    driver.expect_decision(0, "priority")?;
    crate::common::casting::assert_rewound(&driver, before, mtg_engine::decision::Unplayable::Mana);
    // The same spell with one target less goes through.
    aim_fireball(&mut driver, 3, 0)?;
    assert_eq!(driver.pay_options()?.cost, cost("{3}{R}"));
    Ok(())
}

// The corpus cards

#[test]
fn thalia_taxes_the_noncreature_spells_of_every_player_and_no_creature_spell()
-> Result<(), ScriptError> {
    let build = |mountains: usize| {
        Scenario::new(2)
            .hand_named(0, ["Lightning Bolt", "Grizzly Bears"])
            .battlefield(
                0,
                (0..mountains)
                    .map(|_| ready(mountain()))
                    .chain([ready(forest())]),
            )
            .battlefield_named(1, ["Thalia, Guardian of Thraben"])
    };
    // One land more than the Bolt costs: the creature spell costs its own {1}{G}, the Bolt {1}{R}.
    let mut driver = Driver::from_scenario(build(1))?;
    assert_eq!(total_of(&mut driver, "Lightning Bolt")?, cost("{1}{R}"));
    let mut driver = Driver::from_scenario(build(1))?;
    assert_eq!(total_of(&mut driver, "Grizzly Bears")?, cost("{1}{G}"));
    // A Mountain alone paid for a Bolt before Thalia; now it is not offered.
    let alone = Scenario::new(2)
        .hand_named(0, ["Lightning Bolt"])
        .battlefield(0, [ready(mountain())])
        .battlefield_named(1, ["Thalia, Guardian of Thraben"]);
    let mut driver = Driver::from_scenario(alone)?;
    assert!(driver.begin_cast("Lightning Bolt").is_err());
    Ok(())
}

#[test]
fn foundry_inspector_reduces_the_artifact_spells_of_its_controller_only() -> Result<(), ScriptError>
{
    let build = |inspector_seat: usize| {
        let mut seats = [vec![ready(mountain())], Vec::new()];
        seats[inspector_seat].push(ready(
            mtg_testkit::corpus::card("Foundry Inspector").unwrap(),
        ));
        let [mine, theirs] = seats;
        Scenario::new(2)
            .hand_named(0, ["Bonesplitter"])
            .battlefield(0, mine)
            .battlefield(1, theirs)
    };
    let mut driver = Driver::from_scenario(build(0))?;
    assert_eq!(total_of(&mut driver, "Bonesplitter")?, ManaCost::ZERO);
    // Another player's Inspector changes nothing about my artifacts.
    let mut driver = Driver::from_scenario(build(1))?;
    assert_eq!(total_of(&mut driver, "Bonesplitter")?, cost("{1}"));
    Ok(())
}

#[test]
fn an_inspector_in_the_hand_does_not_reduce_its_own_spell() -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .hand_named(0, ["Foundry Inspector"])
        .battlefield(0, [ready(mountain()), ready(mountain()), ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(total_of(&mut driver, "Foundry Inspector")?, cost("{3}"));
    Ok(())
}
