//! Goal: the r30-5b cards play their counts as printed at two to four seats: Yavimaya Kavu's power and toughness are
//! defined alone (red creatures, green creatures, every player's, itself included) on the battlefield and in a hand,
//! Fungal Behemoth counts the +1/+1 counters on its controller's creatures only, Lumbering Megasloth costs {1} less
//! for each counter on any player or permanent, Aven Interrupter taxes opponents' casts from graveyards, Hinata changes
//! costs by the targets announced, Rampant Frogantua grows for each player who lost, Avenge costs less after an attack on
//! its caster, a spell being cast targets the player it announced, stun counters stay on opponents' permanents under
//! Fear of Sleep Paralysis, and Case of the Locked Hothouse is solved with seven lands.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::characteristics::characteristics;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// A 1/1 creature of the colour its mana cost gives.
fn colored(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse(cost).unwrap())
            .with_power_toughness(1, 1),
    )
}

fn stats(driver: &Driver, id: mtg_core::ids::ObjectId) -> (Option<i32>, Option<i32>) {
    let chars = characteristics(driver.state(), id).unwrap();
    (chars.power, chars.toughness)
}

#[rstest]
#[case::none(0, 0)]
#[case::reds_only(2, 0)]
#[case::both(1, 3)]
fn r305b_yavimaya_kavu_counts_red_and_green_creatures_apart(
    #[values(2, 3, 4)] seats: usize,
    #[case] reds: usize,
    #[case] greens: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Yavimaya Kavu").unwrap())])
        .hand(0, [card("Yavimaya Kavu").unwrap()]);
    // The creatures lie with the last seat: every player's creatures count.
    let last = seats - 1;
    scenario = scenario.battlefield(
        last,
        (0..reds)
            .map(|_| ready(colored("Red", "{R}")))
            .chain((0..greens).map(|_| ready(colored("Green", "{G}"))))
            .chain([ready(free_creature("Colorless", 1, 1))]),
    );
    let driver = Driver::from_scenario(scenario)?;
    let on_battlefield = driver.id_of("Yavimaya Kavu").unwrap();
    // The Kavu is red and green itself.
    let wanted = (
        Some(i32::try_from(reds).unwrap() + 1),
        Some(i32::try_from(greens).unwrap() + 1),
    );
    assert_eq!(stats(&driver, on_battlefield), wanted);
    let in_hand = driver.state().zone(Zone::Hand(pid(0)))[0];
    assert_eq!(
        stats(&driver, in_hand),
        wanted,
        "a characteristic-defining ability works in every zone"
    );
    Ok(())
}

#[rstest]
#[case::no_counters(0, 0, 0)]
#[case::mine(3, 0, 3)]
#[case::theirs_do_not_count(2, 5, 2)]
fn r305b_fungal_behemoth_counts_its_controllers_counters(
    #[values(2, 4)] seats: usize,
    #[case] mine: u32,
    #[case] theirs: u32,
    #[case] size: i32,
) -> Result<(), ScriptError> {
    let counted = |name: &str, count: u32| {
        with_counters(
            ready(free_creature(name, 1, 1)),
            CounterKind::PlusOnePlusOne,
            count,
        )
    };
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(card("Fungal Behemoth").unwrap()),
                counted("Mine", mine),
            ],
        )
        .battlefield(1, [counted("Theirs", theirs)]);
    let driver = Driver::from_scenario(scenario)?;
    match driver.id_of("Fungal Behemoth") {
        Ok(behemoth) => assert_eq!(stats(&driver, behemoth), (Some(size), Some(size))),
        Err(_) => assert_eq!(size, 0, "a 0/0 Behemoth is put into its owner's graveyard"),
    }
    Ok(())
}

#[rstest]
#[case::nothing(0, 0, 0)]
#[case::permanents(3, 2, 0)]
#[case::players_too(1, 1, 4)]
#[case::more_than_the_generic(6, 4, 3)]
fn r305b_lumbering_megasloth_costs_one_less_per_counter_anywhere(
    #[values(2, 3, 4)] seats: usize,
    #[case] on_mine: u32,
    #[case] on_theirs: u32,
    #[case] poison: u32,
) -> Result<(), ScriptError> {
    let counted = |name: &str, count: u32| {
        with_counters(
            ready(free_creature(name, 1, 1)),
            CounterKind::PlusOnePlusOne,
            count,
        )
    };
    let scenario = Scenario::new(seats)
        .hand(0, [card("Lumbering Megasloth").unwrap()])
        .battlefield(
            0,
            lands(12, forest)
                .into_iter()
                .chain([counted("Mine", on_mine)]),
        )
        .battlefield(seats - 1, [counted("Theirs", on_theirs)])
        .counters(seats - 1, CounterKind::Poison, poison);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Lumbering Megasloth")?;
    let generic = 10u32.saturating_sub(on_mine + on_theirs + poison);
    let wanted = if generic == 0 {
        ManaCost::parse("{G}{G}").unwrap()
    } else {
        ManaCost::parse(&format!("{{{generic}}}{{G}}{{G}}")).unwrap()
    };
    assert_eq!(driver.pay_options()?.cost, wanted);
    Ok(())
}

/// Seat 1, the active player, holds Faithless Looting in its hand or graveyard over five Mountains; seat `aven_seat`
/// controls Aven Interrupter.
fn looting(seats: usize, graveyard: bool, aven_seat: usize) -> Scenario {
    let aven = || ready(card("Aven Interrupter").unwrap());
    let mut theirs = lands(5, mtg_testkit::cards::mountain);
    if aven_seat == 1 {
        theirs.push(aven());
    }
    let mut scenario = Scenario::new(seats)
        .at(2, 1, mtg_engine::turn::Step::Main1)
        .battlefield(1, theirs)
        .library(1, (0..4).map(|_| forest()));
    if aven_seat != 1 {
        scenario = scenario.battlefield(aven_seat, [aven()]);
    }
    scenario = if graveyard {
        scenario.graveyard_named(1, ["Faithless Looting"])
    } else {
        scenario.hand_named(1, ["Faithless Looting"])
    };
    scenario
}

#[rstest]
#[case::flashback_against_an_opponent(true, 0, "{4}{R}")]
#[case::from_hand(false, 0, "{R}")]
#[case::flashback_under_its_own_aven(true, 1, "{2}{R}")]
fn r305b_aven_interrupter_taxes_opponents_casting_from_graveyards(
    #[values(2, 3, 4)] seats: usize,
    #[case] graveyard: bool,
    #[case] aven_seat: usize,
    #[case] cost: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(looting(seats, graveyard, aven_seat))?;
    driver.begin_cast("Faithless Looting")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), cost);
    Ok(())
}

/// A sorcery for {4} with a creature target and a player target ("Tap target creature. Target player loses 1 life.").
fn two_targets() -> Arc<CardDefinition> {
    let rules: mtg_cards::ability::Ability = serde_json::from_str(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":"Player"}],"effects":[{"Tap":{"what":{"Target":0}}},{"LoseLife":{"who":{"Target":1},"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Pinch")
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse("{4}").unwrap())
            .with_rules(rules),
    )
}

/// Seat `caster` casts Pinch on its own turn with Hinata under seat 0; the cost asked once both targets are chosen.
fn pinch_cost(seats: usize, caster: usize) -> Result<String, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, caster, mtg_engine::turn::Step::Main1)
        .hand(caster, [two_targets()])
        .battlefield(caster, lands(6, forest));
    let bear = || ready(free_creature("Bear", 2, 2));
    scenario = if caster == 0 {
        scenario.battlefield(
            0,
            lands(6, forest)
                .into_iter()
                .chain([ready(card("Hinata, Dawn-Crowned").unwrap()), bear()]),
        )
    } else {
        scenario.battlefield(0, [ready(card("Hinata, Dawn-Crowned").unwrap()), bear()])
    };
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Pinch")?;
    let bear = driver.target_named("Bear")?;
    let player = driver.target_player(1 - caster.min(1));
    driver.choose_targets(&[bear])?;
    driver.choose_targets(&[player])?;
    Ok(driver.pay_options()?.cost.to_string())
}

#[rstest]
fn r305b_hinata_changes_costs_by_the_targets_of_each_spell(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    assert_eq!(
        pinch_cost(seats, 0)?,
        "{2}",
        "its controller's spell costs one less per target"
    );
    assert_eq!(
        pinch_cost(seats, 1)?,
        "{6}",
        "an opponent's spell costs one more per target"
    );
    Ok(())
}

#[rstest]
#[case::nobody_lost(3, 0)]
#[case::one_lost(3, 1)]
#[case::two_lost(4, 2)]
fn r305b_rampant_frogantua_grows_for_each_player_who_lost(
    #[case] seats: usize,
    #[case] lost: usize,
) -> Result<(), ScriptError> {
    let mut scenario =
        Scenario::new(seats).battlefield(0, [ready(card("Rampant Frogantua").unwrap())]);
    for seat in 1..=lost {
        scenario = scenario.life(seat, 0);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass()?;
    let frog = driver.id_of("Rampant Frogantua").unwrap();
    let bonus = 10 * i32::try_from(lost).unwrap();
    let printed = characteristics(driver.state(), frog).unwrap();
    assert_eq!(
        (printed.power, printed.toughness),
        (Some(3 + bonus), Some(3 + bonus))
    );
    Ok(())
}

/// Seat 1 attacks with its Raider on its turn (seat `attacked`, or not at all), then the game idles to seat 0's next
/// main phase, where seat 0 begins to cast Avenge: the mana asked.
fn avenge_cost(seats: usize, attacked: Option<usize>) -> Result<String, ScriptError> {
    use mtg_engine::turn::Step;
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .hand(0, [card("Avenge").unwrap()])
        .battlefield(0, lands(6, mtg_testkit::cards::plains))
        .battlefield(1, [ready(free_creature("Raider", 1, 1))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::DeclareAttackers)?;
    match attacked {
        Some(seat) => driver.attack(&[("Raider", seat)])?,
        None => driver.attack(&[])?,
    };
    let mine = 2 + u32::try_from(seats).unwrap() - 1;
    driver.idle_until(mine, Step::Main1)?;
    driver.begin_cast("Avenge")?;
    Ok(driver.pay_options()?.cost.to_string())
}

#[rstest]
fn r305b_avenge_costs_less_after_a_player_attacked_you(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    assert_eq!(avenge_cost(seats, Some(0))?, "{2}{W}{W}");
    assert_eq!(avenge_cost(seats, None)?, "{4}{W}{W}", "no attack");
    if seats > 2 {
        assert_eq!(
            avenge_cost(seats, Some(2))?,
            "{4}{W}{W}",
            "an attack on another player"
        );
    }
    Ok(())
}

/// An enchantment of seat 0: "Spells you cast that target an opponent cost {1} less to cast."
fn kinder() -> Arc<CardDefinition> {
    let rules: mtg_cards::ability::Ability = serde_json::from_str(
        r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"You"},{"TargetsPlayer":"Opponents"}],"change":"Reduce","mana":"{1}"}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Kinder")
            .with_type(CardType::Enchantment)
            .with_rules(rules),
    )
}

#[rstest]
#[case::an_opponent(1, "{3}")]
#[case::the_last_opponent(9, "{3}")]
#[case::yourself(0, "{4}")]
fn r305b_a_spell_being_cast_targets_the_player_it_announced(
    #[values(2, 3, 4)] seats: usize,
    #[case] player: usize,
    #[case] cost: &str,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).hand(0, [two_targets()]).battlefield(
        0,
        lands(6, forest)
            .into_iter()
            .chain([ready(kinder()), ready(free_creature("Bear", 2, 2))]),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Pinch")?;
    let bear = driver.target_named("Bear")?;
    let player = driver.target_player(player.min(seats - 1));
    driver.choose_targets(&[bear])?;
    driver.choose_targets(&[player])?;
    assert_eq!(driver.pay_options()?.cost.to_string(), cost);
    Ok(())
}

fn stun() -> CounterKind {
    CounterKind::Other("stun".to_owned())
}

/// The stun counters on the named permanent and whether it is tapped.
fn stunned(driver: &Driver, name: &str) -> (u32, bool) {
    let id = driver.id_of(name).unwrap();
    let permanent = driver.state().object(id).unwrap().permanent().unwrap();
    (permanent.counters_of(&stun()), permanent.tapped())
}

#[rstest]
#[case::an_opponents_permanent_keeps_it(true, (1, true))]
#[case::without_fear_it_is_removed(false, (0, true))]
fn r305b_stun_counters_cant_be_removed_from_opponents_permanents(
    #[values(2, 3, 4)] seats: usize,
    #[case] fear: bool,
    #[case] after: (u32, bool),
) -> Result<(), ScriptError> {
    use mtg_engine::turn::Step;
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            1,
            [with_counters(
                mtg_testkit::tapped(free_creature("Sleeper", 2, 2)),
                stun(),
                1,
            )],
        )
        .library(1, (0..3).map(|_| forest()));
    if fear {
        scenario = scenario.battlefield(0, [ready(card("Fear of Sleep Paralysis").unwrap())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::Main1)?;
    assert_eq!(stunned(&driver, "Sleeper"), after);
    // A second untap step keeps it tapped again under Fear, and untaps it without.
    let next = 2 + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::Main1)?;
    let (counters, tapped) = stunned(&driver, "Sleeper");
    assert_eq!(
        (counters, tapped),
        if fear { (1, true) } else { (0, false) }
    );
    Ok(())
}

#[rstest]
#[case::seven_lands(7, true)]
#[case::six_lands(6, false)]
fn r305b_case_of_the_locked_hothouse_is_solved_with_seven_lands(
    #[values(2, 3)] seats: usize,
    #[case] count: usize,
    #[case] solved: bool,
) -> Result<(), ScriptError> {
    use mtg_engine::turn::Step;
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            lands(count, forest)
                .into_iter()
                .chain([ready(card("Case of the Locked Hothouse").unwrap())]),
        )
        .library(
            0,
            [free_creature("Topmost", 2, 2), forest(), forest(), forest()],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    let next = 1 + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::Main1)?;
    // The draw step took the creature; the next card on top is a Forest, which is played from the library once solved.
    let mtg_engine::decision::DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("seat 0 has priority in its main phase");
    };
    let from_library = options.playable_lands.iter().any(|land| {
        driver
            .state()
            .object(*land)
            .is_some_and(|object| object.zone() == Zone::Library(pid(0)))
    });
    assert_eq!(from_library, solved);
    Ok(())
}
