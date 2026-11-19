//! Goal: the stack forecast names what the top spell will change (power and toughness, damage and deaths, destruction, exile, bounce, counters, tapping, control, life, a countered spell) at two to four seats, agrees with the state after the real resolution, says nothing when the result hangs on a choice, a hidden card or chance, and the preview of a target answer gives the same lines.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::removal::{corpus, pass_to, resolve_the_stack, spell_named};
use common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable};
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::CardType;
use mtg_engine::characteristics;
use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::stack_forecast::{Change, ForecastChange, Forecasted, stack_forecast};
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, PresetPermanent};
use mtg_testkit::cards::{
    cost, drain, edict, forest, free_creature, island, mind_rot, mountain, one_target, plains,
    salve, study, swamp,
};
use mtg_testkit::pump_cards::unsummon;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use proptest::prelude::*;
use rstest::rstest;

/// An instant for `{R}` with one target of `what` and `effects`.
fn spell(name: &str, what: Targetable, effects: Vec<Effect>) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[mtg_core::color::Color::Red]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(what)],
                effects,
            }),
    )
}

fn a_creature() -> Targetable {
    Targetable::Object(Selector(vec![Pred::Type(CardType::Creature)]))
}

/// Put two +1/+1 counters on target creature.
fn bolster() -> Arc<CardDefinition> {
    spell(
        "Bolster",
        a_creature(),
        vec![Effect::PutCounters {
            on: Ref::Target(0),
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(2),
        }],
    )
}

/// Tap target creature.
fn frost() -> Arc<CardDefinition> {
    spell(
        "Frost",
        a_creature(),
        vec![Effect::Tap {
            what: Ref::Target(0),
            bind: None,
        }],
    )
}

/// Deal `amount` damage to target creature.
fn zap(amount: i32) -> Arc<CardDefinition> {
    spell(
        "Zap",
        a_creature(),
        vec![mtg_testkit::cards::damage(Ref::Target(0), amount)],
    )
}

/// Lands enough for any spell here.
fn lands() -> Vec<PresetPermanent> {
    [mountain, forest, island, swamp, plains]
        .into_iter()
        .flat_map(|land| (0..3).map(move |_| ready(land())))
        .collect()
}

/// Seat 0 holds `card` and lands; seat 1 controls a 4/4 Wolf and a 2/2 Bear; every other seat a 1/1.
fn table(seats: usize, card: Arc<CardDefinition>) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(0, [card])
        .battlefield(0, lands())
        .battlefield(
            1,
            [
                ready(free_creature("Wolf", 4, 4)),
                ready(free_creature("Bear", 2, 2)),
            ],
        );
    for seat in 2..seats {
        scenario = scenario.battlefield(
            seat,
            [ready(free_creature(&format!("Bystander {seat}"), 1, 1))],
        );
    }
    scenario
}

/// What `aim` names as a target: a creature by name, or a seat as `P<n>`.
fn target(driver: &Driver, aim: &str) -> Target {
    aim.strip_prefix('P').map_or_else(
        || driver.target_named(aim).unwrap(),
        |seat| driver.target_player(seat.parse().unwrap()),
    )
}

/// Seat 0 casts `card` at `aims` (none for a spell without targets); the spell is on the stack.
fn cast(seats: usize, card: Arc<CardDefinition>, aims: &[&str]) -> Result<Driver, ScriptError> {
    let name = card.name.clone();
    let mut driver = Driver::from_scenario(table(seats, card))?;
    if aims.is_empty() {
        driver.cast(&name)?;
    } else {
        let targets: Vec<Target> = aims.iter().map(|aim| target(&driver, aim)).collect();
        driver.cast_at(&name, &targets)?;
    }
    Ok(driver)
}

/// The forecast in words: `<name or P<n>>: <change>`.
fn words(driver: &Driver, lines: &[ForecastChange]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            let who = match line.subject {
                Forecasted::Player(player) => format!("P{}", player.index()),
                Forecasted::Object(object) => name_of(driver.state(), object),
            };
            let what = match &line.change {
                Change::Damage { amount } => format!("{amount} damage"),
                Change::Stats { from, to } => {
                    format!("{}/{} -> {}/{}", from.0, from.1, to.0, to.1)
                }
                Change::Counters { kind, from, to } => format!("{kind:?} {from} -> {to}"),
                Change::Tapped => "tapped".to_owned(),
                Change::Untapped => "untapped".to_owned(),
                Change::Control { to } => format!("to P{}", to.index()),
                Change::Leaves { fate } => format!("{fate:?}"),
                Change::Life { from, to } => format!("life {from} -> {to}"),
            };
            format!("{who}: {what}")
        })
        .collect()
}

/// Resolves the stack for real and checks that each line of `lines` holds afterwards.
fn agrees(driver: &mut Driver, lines: &[ForecastChange]) {
    resolve_the_stack(driver).unwrap();
    let state = driver.state();
    for line in lines {
        match (line.subject, &line.change) {
            (Forecasted::Player(player), Change::Life { to, .. }) => {
                assert_eq!(state.player(player).unwrap().life(), *to, "{line:?}");
            }
            (Forecasted::Player(player), Change::Counters { kind, to, .. }) => {
                assert_eq!(state.player(player).unwrap().counters_of(kind), *to);
            }
            (Forecasted::Object(object), Change::Leaves { .. }) => {
                assert!(
                    state
                        .object(object)
                        .is_none_or(|o| !matches!(o.zone(), Zone::Battlefield | Zone::Stack)),
                    "{line:?} stayed"
                );
            }
            (Forecasted::Object(object), change) => {
                let Some(permanent) = state.object(object).and_then(|o| o.permanent()) else {
                    continue;
                };
                match change {
                    Change::Damage { amount } => assert_eq!(permanent.damage(), *amount),
                    Change::Stats { to, .. } => {
                        let chars = characteristics(state, object).unwrap();
                        assert_eq!((chars.power.unwrap(), chars.toughness.unwrap()), *to);
                    }
                    Change::Counters { kind, to, .. } => {
                        assert_eq!(permanent.counters_of(kind), *to);
                    }
                    Change::Tapped => assert!(permanent.tapped()),
                    Change::Untapped => assert!(!permanent.tapped()),
                    Change::Control { to } => {
                        assert_eq!(controller_of(state, object), Some(*to));
                    }
                    _ => panic!("{line:?} is no line of an object"),
                }
            }
            _ => panic!("{line:?} is no line of a player"),
        }
    }
}

#[rstest]
#[case::pump(corpus("Giant Growth"), &["Wolf"], &["Wolf: 4/4 -> 7/7"])]
#[case::damage_survived(zap(3), &["Wolf"], &["Wolf: 3 damage"])]
#[case::damage_dies(corpus("Lightning Bolt"), &["Bear"], &["Bear: 3 damage", "Bear: Dies"])]
#[case::damage_to_a_player(corpus("Lightning Bolt"), &["P1"], &["P1: life 20 -> 17"])]
#[case::destroy(corpus("Doom Blade"), &["Wolf"], &["Wolf: Destroyed"])]
#[case::exile_and_life(corpus("Swords to Plowshares"), &["Wolf"], &["P1: life 20 -> 24", "Wolf: Exiled"])]
#[case::bounce(unsummon("Unsummon"), &["Bear"], &["Bear: Hand"])]
#[case::counters(bolster(), &["Wolf"], &["Wolf: 4/4 -> 6/6", "Wolf: PlusOnePlusOne 0 -> 2"])]
#[case::tap(frost(), &["Wolf"], &["Wolf: tapped"])]
#[case::drain(drain("Drain"), &["P1"], &["P0: life 20 -> 22", "P1: life 20 -> 18"])]
#[case::control(corpus("Act of Treason"), &["Bear"], &["Bear: to P0"])]
fn the_forecast_names_each_change_and_holds(
    #[case] card: Arc<CardDefinition>,
    #[case] aims: &[&str],
    #[case] expected: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = cast(seats, card, aims)?;
    let lines = stack_forecast(driver.state()).expect("a forecast");
    assert_eq!(words(&driver, &lines), expected);
    agrees(&mut driver, &lines);
    Ok(())
}

#[rstest]
#[case::draws_a_hidden_card(study("Study"), &[])]
#[case::damage_then_a_draw(salve("Salve"), &["Wolf"])]
#[case::scry(corpus("Opt"), &[])]
#[case::the_opponent_chooses(edict("Edict"), &["P1"])]
#[case::discard_of_their_choice(mind_rot("Mind Rot"), &["P1"])]
#[case::coin_flips(corpus("Mana Clash"), &["P1"])]
#[case::a_creature_spell_changes_nothing_public(corpus("Grizzly Bears"), &[])]
fn no_forecast_when_a_choice_a_hidden_card_or_chance_decides(
    #[case] card: Arc<CardDefinition>,
    #[case] aims: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = cast(seats, card, aims)?;
    assert!(
        !driver.state().stack().is_empty(),
        "the spell waits on the stack"
    );
    assert_eq!(stack_forecast(driver.state()), None);
    Ok(())
}

#[rstest]
fn a_countered_spell_is_forecast_as_countered(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(0, [corpus("Giant Growth")])
        .battlefield(0, lands())
        .hand(1, [corpus("Counterspell")])
        .battlefield(
            1,
            [
                ready(free_creature("Wolf", 4, 4)),
                ready(island()),
                ready(island()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let wolf = driver.target_named("Wolf")?;
    driver.cast_at("Giant Growth", &[wolf])?;
    pass_to(&mut driver, 1)?;
    let growth = spell_named(&driver, "Giant Growth");
    driver.cast_at("Counterspell", &[growth])?;
    let lines = stack_forecast(driver.state()).expect("a forecast");
    assert_eq!(words(&driver, &lines), ["Giant Growth: Countered"]);
    Ok(())
}

#[rstest]
#[case::pump(corpus("Giant Growth"), "Wolf")]
#[case::burn(corpus("Lightning Bolt"), "Bear")]
#[case::player(corpus("Lightning Bolt"), "P1")]
#[case::destroy(corpus("Doom Blade"), "Wolf")]
fn a_previewed_target_gives_the_lines_of_the_cast_spell(
    #[case] card: Arc<CardDefinition>,
    #[case] aim: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let name = card.name.clone();
    let mut driver = Driver::from_scenario(table(seats, card))?;
    driver.begin_cast(&name)?;
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::ChooseTargets(_)
    ));
    let answer = Answer::ChooseTargets(TargetChoice::Targets(vec![target(&driver, aim)]));
    let hash = driver.engine().state_hash();
    let previewed = driver.engine().preview_targets(pid(0), &answer).unwrap();
    assert_eq!(
        driver.engine().state_hash(),
        hash,
        "the preview moved the game"
    );
    assert!(driver.engine().preview_targets(pid(1), &answer).is_err());
    driver.answer(answer)?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(Some(previewed), stack_forecast(driver.state()));
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Whenever a forecast is given it is what the real resolution does.
    #[test]
    fn a_forecast_given_is_the_resolution(seats in 2usize..=4, amount in 1i32..=6, wolf in prop::bool::ANY) {
        let aim = if wolf { "Wolf" } else { "Bear" };
        let mut driver = cast(seats, zap(amount), &[aim]).unwrap();
        let lines = stack_forecast(driver.state()).expect("a forecast");
        agrees(&mut driver, &lines);
    }
}
