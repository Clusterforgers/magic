//! Goal: the r55-14 pieces play by the rules at tables of two to four: the opponent who chose Fatal Lore's mode is
//! "that player" for its targets and its draws (`Ref::ModesChooser`); Time and Tide phases every phased-out creature
//! in and every creature with phasing out at once, so none moves twice; Teferi's Reproach protects its target and holds
//! their life total until that player's next turn, not the caster's, and phases out their nonland permanents only.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{ModeChoice, NumberChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::state::GameState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, keyworded};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A corpus card for no mana.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut definition = (*card(name).unwrap()).clone();
    definition.mana_cost = Some(ManaCost::parse("{0}").unwrap());
    Arc::new(definition)
}

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

fn hand(state: &GameState, index: usize) -> usize {
    state.zone(Zone::Hand(seat(index))).len()
}

fn on_battlefield(driver: &Driver, name: &str) -> bool {
    driver.state().zone(Zone::Battlefield).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == name)
    })
}

fn phased_out(driver: &Driver, name: &str) -> bool {
    driver.state().zone(Zone::PhasedOut).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == name)
    })
}

/// Every seat holds ten Forests in its library; seat `i` controls the creature "Beast i" and seat 1 also "Brute".
fn table(seats: usize, spell: &str) -> Driver {
    let mut scenario = Scenario::new(seats).hand(0, [free(spell), free(spell)]);
    for index in 0..seats {
        let mut permanents = vec![
            ready(forest()),
            ready(free_creature(&format!("Beast {index}"), 2, 2)),
        ];
        if index == 1 {
            permanents.push(ready(free_creature("Brute", 3, 3)));
        }
        scenario = scenario
            .battlefield(index, permanents)
            .library(index, (0..10).map(|_| forest()));
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Passes priority until the stack is empty or something else is asked.
fn settle(driver: &mut Driver) {
    for _ in 0..20 {
        match &driver.decision().unwrap().kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            _ => return,
        }
    }
}

#[rstest]
fn fatal_lore_draws_three_for_its_caster(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats, "Fatal Lore");
    driver.begin_cast("Fatal Lore").unwrap();
    let asked = driver.decision().unwrap().clone();
    assert!(
        matches!(asked.kind, DecisionKind::ChooseMode(_)),
        "{asked:?}"
    );
    assert_eq!(asked.player, seat(1), "the next opponent chooses the mode");
    driver
        .answer(Answer::ChooseMode(ModeChoice::Modes(vec![0])))
        .unwrap();
    driver
        .answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))
        .unwrap();
    settle(&mut driver);
    assert_eq!(
        hand(driver.state(), 0),
        4,
        "the caster drew three beside the second copy"
    );
    assert_eq!(hand(driver.state(), 1), 0);
}

#[rstest]
fn fatal_lore_destroys_what_the_chooser_controls_and_they_draw_up_to_three(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2, 3)] drawn: u32,
) {
    let mut driver = table(seats, "Fatal Lore");
    driver.begin_cast("Fatal Lore").unwrap();
    driver
        .answer(Answer::ChooseMode(ModeChoice::Modes(vec![1])))
        .unwrap();
    let DecisionKind::ChooseTargets(options) = driver.decision().unwrap().kind.clone() else {
        panic!("the second mode asks for its targets");
    };
    assert_eq!(
        driver.decision().unwrap().player,
        seat(0),
        "the caster chooses the targets"
    );
    let brute = driver.target_named("Brute").unwrap();
    let beast = driver.target_named("Beast 1").unwrap();
    let mut legal = options.legal.clone();
    legal.sort();
    let mut expected = vec![brute, beast];
    expected.sort();
    assert_eq!(
        legal, expected,
        "only creatures of the player who chose the mode"
    );
    assert_eq!((options.min, options.max), (0, 2));
    driver
        .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
            brute, beast,
        ])))
        .unwrap();
    driver
        .answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))
        .unwrap();
    settle(&mut driver);
    let asked = driver.decision().unwrap().clone();
    let DecisionKind::ChooseNumber(numbers) = &asked.kind else {
        panic!("the chooser picks how many to draw: {asked:?}");
    };
    assert_eq!(asked.player, seat(1));
    assert_eq!((numbers.min, numbers.max), (0, Some(3)));
    driver
        .answer(Answer::ChooseNumber(NumberChoice::Number(drawn)))
        .unwrap();
    settle(&mut driver);
    assert!(!on_battlefield(&driver, "Brute") && !on_battlefield(&driver, "Beast 1"));
    assert!(on_battlefield(&driver, "Beast 0"));
    assert_eq!(hand(driver.state(), 1), drawn as usize);
    assert_eq!(hand(driver.state(), 0), 1);
}

/// A table where seat 0 holds two Time and Tides and every seat controls a creature with phasing and one without.
fn tide_table(seats: usize) -> Driver {
    let mut scenario = Scenario::new(seats).hand(0, [free("Time and Tide"), free("Time and Tide")]);
    for index in 0..seats {
        scenario = scenario.battlefield(
            index,
            [
                ready(keyworded(
                    &format!("Shifter {index}"),
                    1,
                    1,
                    &[Keyword::Phasing],
                )),
                ready(free_creature(&format!("Stayer {index}"), 2, 2)),
            ],
        );
    }
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
fn time_and_tide_swaps_phased_out_creatures_and_creatures_with_phasing(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = tide_table(seats);
    driver.cast("Time and Tide").unwrap();
    settle(&mut driver);
    for index in 0..seats {
        assert!(
            phased_out(&driver, &format!("Shifter {index}")),
            "every creature with phasing phased out"
        );
        assert!(on_battlefield(&driver, &format!("Stayer {index}")));
    }
    driver.cast("Time and Tide").unwrap();
    settle(&mut driver);
    for index in 0..seats {
        // Phased in by the second, it has phasing but does not phase out again: both sets were judged before either moved.
        assert!(
            on_battlefield(&driver, &format!("Shifter {index}")),
            "seat {index}"
        );
        assert!(on_battlefield(&driver, &format!("Stayer {index}")));
    }
}

#[rstest]
fn teferis_reproach_lasts_until_the_targets_next_turn(#[values(2, 3, 4)] seats: usize) {
    let target = seats - 1;
    let mut driver = table(seats, "Teferi's Reproach");
    driver.begin_cast("Teferi's Reproach").unwrap();
    driver
        .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
            driver.target_player(target),
        ])))
        .unwrap();
    driver
        .answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))
        .unwrap();
    settle(&mut driver);
    let player = seat(target);
    assert!(mtg_engine::rules::life_total_cant_change(
        driver.state(),
        player
    ));
    assert!(!mtg_engine::rules::life_total_cant_change(
        driver.state(),
        seat(0)
    ));
    assert!(
        phased_out(&driver, &format!("Beast {target}")),
        "their nonland permanents phase out"
    );
    assert!(!phased_out(&driver, "Beast 0"));
    let forests = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| {
                driver
                    .state()
                    .object(**id)
                    .is_some_and(|o| o.card().name == "Forest")
            })
            .count()
    };
    assert_eq!(forests(&driver), seats, "lands stay");
    assert!(
        driver.state().zone(Zone::Exile).iter().any(|id| driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == "Teferi's Reproach")),
        "the spell exiles itself"
    );
    // The turns of the seats before the target keep it in force; the target's own turn ends it.
    for turn in 2..=u32::try_from(target).unwrap() {
        driver.idle_until(turn, Step::Upkeep).unwrap();
        assert!(
            mtg_engine::rules::life_total_cant_change(driver.state(), player),
            "turn {turn}"
        );
    }
    let theirs = u32::try_from(target).unwrap() + 1;
    driver.idle_until(theirs, Step::Upkeep).unwrap();
    assert_eq!(driver.state().turn().active(), player);
    assert!(!mtg_engine::rules::life_total_cant_change(
        driver.state(),
        player
    ));
    assert!(
        on_battlefield(&driver, &format!("Beast {target}")),
        "phased in during their untap step"
    );
}

/// A Forest whose mana abilities cost an additional `life` life to activate, as Thran Portal's do.
fn portal_forest(life: u32) -> Arc<CardDefinition> {
    let modifier: mtg_cards::ability::Ability = serde_json::from_str(&format!(
        r#"{{"CostModifier":{{"applies_to":["This"],"change":"Increase","mana":"{{0}}","activating":"Mana","life":{life}}}}}"#
    ))
    .unwrap();
    let mut definition = (*forest()).clone();
    definition.name = "Portal Forest".to_owned();
    Arc::new(definition.with_rules(modifier))
}

fn life(driver: &Driver, index: usize) -> i32 {
    driver.state().player(seat(index)).unwrap().life()
}

#[rstest]
fn mana_abilities_of_a_portal_cost_life_each_time(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] added: u32,
) {
    let mut scenario = Scenario::new(seats);
    for index in 0..seats {
        scenario = scenario.battlefield(index, [ready(portal_forest(added)), ready(forest())]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let before = life(&driver, 0);
    driver.tap_for_mana("Forest").unwrap();
    assert_eq!(
        life(&driver, 0),
        before,
        "another land's mana ability costs no life"
    );
    driver.tap_for_mana("Portal Forest").unwrap();
    assert_eq!(life(&driver, 0), before - i32::try_from(added).unwrap());
    for index in 1..seats {
        assert_eq!(
            life(&driver, index),
            before,
            "only the activating player pays"
        );
    }
}

fn energy(driver: &Driver, index: usize) -> u32 {
    driver
        .state()
        .player(seat(index))
        .unwrap()
        .counters_of(&mtg_cards::value::CounterKind::Other("energy".into()))
}

/// Answers until the stack is empty: a number asked is `times`, targets are `targets` (the first `most` legal ones
/// when empty), a yes or no is `yes`, mana is paid automatically and priority is passed.
fn drive(driver: &mut Driver, times: u32, targets: &[mtg_engine::choose::Target], yes: bool) {
    for _ in 0..80 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return;
            }
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
                continue;
            }
            DecisionKind::ChooseNumber(_) => Answer::ChooseNumber(NumberChoice::Number(times)),
            DecisionKind::YesNo(_) => Answer::YesNo(mtg_engine::choose::YesNoChoice(yes)),
            DecisionKind::PayMana(_) => Answer::PayMana(mtg_engine::decision::PayChoice::Auto),
            DecisionKind::ChooseTargets(options) => {
                let wanted: Vec<_> = targets
                    .iter()
                    .copied()
                    .filter(|target| options.legal.contains(target))
                    .take(usize::try_from(options.max).unwrap_or(usize::MAX))
                    .collect();
                Answer::ChooseTargets(TargetChoice::Targets(wanted))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the stack did not settle");
}

#[rstest]
fn behemoth_pays_the_targets_mana_value_in_energy_to_destroy_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] pays: bool,
) {
    let foe = seats - 1;
    let mut scenario = Scenario::new(seats)
        .hand(0, [free("Behemoth of Vault 0"), free("Doom Blade")])
        .battlefield(foe, [ready(card("Grizzly Bears").unwrap())]);
    for index in 0..seats {
        scenario = scenario.library(index, (0..10).map(|_| forest()));
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Behemoth of Vault 0").unwrap();
    drive(&mut driver, 0, &[], true);
    assert_eq!(energy(&driver, 0), 4);
    let behemoth = driver.target_named("Behemoth of Vault 0").unwrap();
    let bears = driver.target_named("Grizzly Bears").unwrap();
    driver.begin_cast("Doom Blade").unwrap();
    driver
        .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![behemoth])))
        .unwrap();
    drive(&mut driver, 0, &[bears], pays);
    assert!(!on_battlefield(&driver, "Behemoth of Vault 0"));
    assert_eq!(on_battlefield(&driver, "Grizzly Bears"), !pays);
    assert_eq!(
        energy(&driver, 0),
        if pays { 2 } else { 4 },
        "the bears' mana value is paid"
    );
}

#[rstest]
fn spectral_adversary_grows_and_phases_out_up_to_that_many(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] times: u32,
) {
    let mut scenario = Scenario::new(seats)
        .hand(0, [free("Spectral Adversary")])
        .battlefield(
            0,
            (0..4)
                .map(|_| ready(mtg_testkit::cards::island()))
                .collect::<Vec<_>>(),
        );
    for index in 1..seats {
        scenario = scenario.battlefield(
            index,
            [
                ready(free_creature(&format!("Beast {index}"), 2, 2)),
                ready(free_creature(&format!("Ox {index}"), 2, 2)),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Spectral Adversary").unwrap();
    let wanted = [
        driver.target_named("Beast 1").unwrap(),
        driver.target_named("Ox 1").unwrap(),
    ];
    drive(&mut driver, times, &wanted, true);
    let adversary = driver.id_of("Spectral Adversary").unwrap();
    let counters = driver
        .state()
        .object(adversary)
        .and_then(|o| o.permanent())
        .map_or(0, |p| {
            p.counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne)
        });
    assert_eq!(counters, times);
    assert_eq!(phased_out(&driver, "Beast 1"), times >= 1);
    assert_eq!(phased_out(&driver, "Ox 1"), times >= 2);
    if seats > 2 {
        assert!(
            on_battlefield(&driver, "Beast 2"),
            "nothing untargeted phases out"
        );
    }
}
