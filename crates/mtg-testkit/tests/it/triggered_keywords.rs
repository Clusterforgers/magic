//! Goal: exalted, melee, mentor, dethrone, annihilator, extort and evolve, the keywords that are triggers on a combat or a cast or an entering creature, play whole at tables of two, three and four seats as their rules say: exalted gives a lone attacker +1/+1 for each instance on its controller's side and nothing to a group of attackers or for an opponent's instance; melee grows by the number of distinct opponents attacked, also one whose attacker left combat in response; mentor puts a counter on an attacker with lesser power only and has no trigger without one; dethrone counts the attacked player's life against every player's, ties included; annihilator makes the defending player of that attacker sacrifice, asking only when they have more permanents than the number, also when the creature left in response, once for each instance; extort asks for {W/B} once for each instance and drains each opponent when it is paid; evolve compares both power and toughness of each entering creature with the creature's own current size, also when the creature left in response; and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup, characteristics, replay};
use mtg_testkit::cards::{burn, free_creature, keyworded, mountain, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// The power and toughness of the permanent called `name`.
fn size(driver: &Driver, name: &str) -> (i32, i32) {
    let id = driver.id_of(name).unwrap();
    let chars = characteristics(driver.state(), id).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

fn counters(driver: &Driver, name: &str) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

/// How many permanents the seat controls.
fn permanents(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().controller() == pid(seat))
        .count()
}

/// The log replays to the live state, and an engine rebuilt from it stands at the same decision.
#[allow(clippy::unnecessary_wraps)] // the tests end with it, as they end with their last script step
fn assert_replays(driver: &Driver, setup: &GameSetup) -> Result<(), ScriptError> {
    let log = driver.engine().events().to_vec();
    let replayed = replay(setup, &log).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    let resumed = Engine::resume(setup.clone(), log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    assert_eq!(resumed.decision(), driver.engine().decision());
    Ok(())
}

/// What a settled stack asked: permanents to sacrifice and payments.
#[derive(Debug, Default, PartialEq, Eq)]
struct Asked {
    sacrifices: usize,
    payments: usize,
}

/// Plays on until the stack is empty at a priority decision: triggers are ordered as listed, the first legal target is
/// chosen, the fewest permanents offered are sacrificed (the first ones), and each yes-or-no question is answered from
/// `pays` in turn (No once they run out, and always No when it is the only answer) with the payment made automatically.
fn settle(driver: &mut Driver, pays: &[bool]) -> Result<Asked, ScriptError> {
    let mut pays = pays.iter();
    let mut asked = Asked::default();
    for _ in 0..80 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::OrderTriggers(options) => {
                let order: Vec<usize> = (0..options.triggers.len()).collect();
                driver.order_triggers(&order)?;
            }
            DecisionKind::ChooseTargets(options) => {
                let first = options.legal[0];
                driver.choose_targets(&[first])?;
            }
            DecisionKind::ChooseCards(options) => {
                asked.sacrifices += 1;
                let picks = options
                    .from
                    .iter()
                    .copied()
                    .take(options.min as usize)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(picks)))?;
            }
            DecisionKind::YesNo(options) => {
                asked.payments += 1;
                let yes = !options.only_no && pays.next().copied().unwrap_or(false);
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            other => return Err(ScriptError::Stuck(format!("a decision: {}", other.name()))),
        }
    }
    Err(ScriptError::Stuck("an empty stack".into()))
}

/// Seat 0 attacks on turn 2 with `mine`; every other seat controls `others(seat)`.
fn attack_table(
    seats: usize,
    mine: Vec<Arc<CardDefinition>>,
    others: impl Fn(usize) -> Vec<Arc<CardDefinition>>,
) -> Scenario {
    let mut table = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, mine.into_iter().map(ready).collect::<Vec<_>>());
    for seat in 1..seats {
        table = table.battlefield(
            seat,
            others(seat).into_iter().map(ready).collect::<Vec<_>>(),
        );
    }
    table
}

fn bystander(_seat: usize) -> Vec<Arc<CardDefinition>> {
    vec![free_creature("Bystander", 1, 1)]
}

fn exalter(name: &str) -> Arc<CardDefinition> {
    keyworded(name, 1, 1, &[Keyword::Exalted])
}

#[rstest]
#[case::one_instance(1, false, 1, 1)]
#[case::three_instances(3, false, 1, 3)]
#[case::the_attacker_has_it_too(1, true, 1, 2)]
#[case::two_attackers_are_not_alone(2, true, 2, 0)]
fn exalted_gives_a_lone_attacker_plus_one_for_each_instance(
    #[values(2, 3, 4)] seats: usize,
    #[case] others: usize,
    #[case] attacker_has_it: bool,
    #[case] attackers: usize,
    #[case] bonus: i32,
) -> Result<(), ScriptError> {
    let table = || {
        let grunt = if attacker_has_it {
            exalter("Grunt")
        } else {
            free_creature("Grunt", 1, 1)
        };
        let mut mine = vec![grunt, free_creature("Cub", 1, 1)];
        mine.extend((0..others).map(|i| exalter(&format!("Monk{i}"))));
        attack_table(seats, mine, bystander)
    };
    let mut driver = Driver::from_scenario(table())?;
    let attacking: Vec<(&str, usize)> = ["Grunt", "Cub"][..attackers]
        .iter()
        .map(|name| (*name, 1))
        .collect();
    driver.attack(&attacking)?;
    settle(&mut driver, &[])?;
    assert_eq!(size(&driver, "Grunt"), (1 + bonus, 1 + bonus));
    assert_eq!(size(&driver, "Cub"), (1, 1));
    // The bonus is until end of turn.
    driver.idle_until(3, Step::Upkeep)?;
    assert_eq!(size(&driver, "Grunt"), (1, 1));
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
fn an_opponents_exalted_does_not_trigger_for_my_lone_attacker(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        attack_table(seats, vec![free_creature("Grunt", 1, 1)], |seat| {
            vec![exalter(&format!("Rival{seat}"))]
        })
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Grunt", 1)])?;
    settle(&mut driver, &[])?;
    assert_eq!(size(&driver, "Grunt"), (1, 1));
    assert_replays(&driver, &table().setup().unwrap())
}

fn meleer() -> Arc<CardDefinition> {
    keyworded("Meleer", 2, 2, &[Keyword::Melee])
}

/// The seat the `n`th target index names: the other seats in turn, wrapping around.
fn other_seat(seats: usize, n: usize) -> usize {
    1 + n % (seats - 1)
}

#[rstest]
#[case::one_opponent(&[0])]
#[case::two_attackers_one_opponent(&[0, 0])]
#[case::two_opponents(&[0, 1])]
#[case::three_opponents(&[0, 1, 2])]
fn melee_grows_by_the_number_of_opponents_attacked(
    #[values(2, 3, 4)] seats: usize,
    #[case] targets: &[usize],
) -> Result<(), ScriptError> {
    let names = ["Meleer", "Cub0", "Cub1"];
    let table = || {
        let mut mine = vec![meleer()];
        mine.extend((0..2).map(|i| free_creature(&format!("Cub{i}"), 1, 1)));
        attack_table(seats, mine, bystander)
    };
    let mut driver = Driver::from_scenario(table())?;
    let attacks: Vec<(&str, usize)> = targets
        .iter()
        .enumerate()
        .map(|(at, target)| (names[at], other_seat(seats, *target)))
        .collect();
    driver.attack(&attacks)?;
    settle(&mut driver, &[])?;
    let mut distinct: Vec<usize> = attacks.iter().map(|(_, seat)| *seat).collect();
    distinct.sort_unstable();
    distinct.dedup();
    let n = i32::try_from(distinct.len()).unwrap();
    assert_eq!(size(&driver, "Meleer"), (2 + n, 2 + n));
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
fn melee_counts_an_opponent_whose_attacker_left_combat_in_response(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        attack_table(
            seats,
            vec![meleer(), free_creature("Cub0", 1, 1), mountain()],
            bystander,
        )
        .hand(0, [burn("Bolt", 3)])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Meleer", 1), ("Cub0", 2)])?;
    // The melee trigger is on the stack; Cub0 dies before it resolves.
    let cub = driver.target_named("Cub0")?;
    driver.cast_at("Bolt", &[cub])?;
    settle(&mut driver, &[])?;
    assert!(driver.id_of("Cub0").is_err(), "Cub0 died");
    assert_eq!(size(&driver, "Meleer"), (4, 4));
    assert_replays(&driver, &table().setup().unwrap())
}

fn mentor() -> Arc<CardDefinition> {
    keyworded("Mentor", 3, 3, &[Keyword::Mentor])
}

#[rstest]
fn mentor_offers_attackers_with_lesser_power_only(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        attack_table(
            seats,
            vec![
                mentor(),
                free_creature("Cub", 1, 1),
                free_creature("Grunt", 2, 2),
                free_creature("Peer", 3, 3),
                free_creature("Idler", 1, 1),
            ],
            bystander,
        )
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Mentor", 1), ("Cub", 1), ("Grunt", 1), ("Peer", 1)])?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("mentor asks for its target");
    };
    // Not the Mentor itself, the Peer of equal power, or the Idler that stayed home.
    assert_eq!(options.legal.len(), 2);
    let grunt = driver.target_named("Grunt")?;
    driver.choose_targets(&[grunt])?;
    settle(&mut driver, &[])?;
    assert_eq!(counters(&driver, "Grunt"), 1);
    assert_eq!(size(&driver, "Grunt"), (3, 3));
    for name in ["Cub", "Peer", "Idler", "Mentor"] {
        assert_eq!(counters(&driver, name), 0, "{name}");
    }
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
#[case::attacking_alone(&["Mentor"])]
#[case::beside_an_equal(&["Mentor", "Peer"])]
fn mentor_has_no_trigger_without_an_attacker_of_lesser_power(
    #[values(2, 3, 4)] seats: usize,
    #[case] attacking: &[&str],
) -> Result<(), ScriptError> {
    let table = || {
        attack_table(
            seats,
            vec![
                mentor(),
                free_creature("Peer", 3, 3),
                free_creature("Idler", 1, 1),
            ],
            bystander,
        )
    };
    let mut driver = Driver::from_scenario(table())?;
    let attacks: Vec<(&str, usize)> = attacking.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks)?;
    assert_eq!(driver.state().stack().len(), 0, "no trigger was put on it");
    assert_eq!(settle(&mut driver, &[])?, Asked::default());
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
fn two_mentors_give_two_counters(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let table = || {
        attack_table(
            seats,
            vec![
                mentor(),
                keyworded("Mentor2", 3, 3, &[Keyword::Mentor]),
                free_creature("Cub", 1, 1),
            ],
            bystander,
        )
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Mentor", 1), ("Mentor2", 1), ("Cub", 1)])?;
    settle(&mut driver, &[])?;
    assert_eq!(counters(&driver, "Cub"), 2);
    assert_replays(&driver, &table().setup().unwrap())
}

fn dethroner() -> Arc<CardDefinition> {
    keyworded("Dethroner", 2, 2, &[Keyword::Dethrone])
}

#[rstest]
#[case::all_tied(&[20, 20, 20, 20], 1)]
#[case::the_most_life(&[20, 25, 20, 20], 1)]
#[case::less_than_another(&[20, 15, 25, 20], 0)]
#[case::less_than_the_attacker(&[30, 20, 20, 20], 0)]
#[case::tied_for_the_most(&[20, 30, 30, 10], 1)]
fn dethrone_counts_the_attacked_players_life_against_every_players(
    #[values(2, 3, 4)] seats: usize,
    #[case] lives: &[i32],
    #[case] counters_expected: u32,
) -> Result<(), ScriptError> {
    let table = || {
        let mut table = attack_table(seats, vec![dethroner()], bystander);
        for (seat, life) in lives.iter().take(seats).enumerate() {
            table = table.life(seat, *life);
        }
        table
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Dethroner", 1)])?;
    settle(&mut driver, &[])?;
    assert_eq!(counters(&driver, "Dethroner"), counters_expected);
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
fn dethrone_does_not_trigger_for_a_creature_that_attacks_another_player(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        attack_table(seats, vec![dethroner()], bystander)
            .life(1, 30)
            .life(2, 10)
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Dethroner", 2)])?;
    settle(&mut driver, &[])?;
    assert_eq!(counters(&driver, "Dethroner"), 0);
    assert_replays(&driver, &table().setup().unwrap())
}

fn annihilator(name: &str, count: u32) -> Arc<CardDefinition> {
    keyworded(name, 1, 1, &[Keyword::Annihilator(count)])
}

/// Seat 0 attacks with `attackers`, each aimed at the seat `aimed` names; `held` permanents belong to the defending seats.
fn annihilator_table(
    seats: usize,
    attackers: Vec<Arc<CardDefinition>>,
    held: impl Fn(usize) -> usize,
) -> Scenario {
    attack_table(seats, attackers, move |seat| {
        (0..held(seat))
            .map(|i| free_creature(&format!("Pawn{seat}{i}"), 1, 1))
            .collect()
    })
}

#[rstest]
#[case::chooses_among_more(2, 5, 3, 1)]
#[case::exactly_enough(2, 2, 0, 0)]
#[case::fewer_than_the_number(3, 1, 0, 0)]
#[case::nothing_to_sacrifice(2, 0, 0, 0)]
fn annihilator_makes_the_defending_player_sacrifice_that_many_permanents(
    #[values(2, 3, 4)] seats: usize,
    #[case] count: u32,
    #[case] held: usize,
    #[case] left: usize,
    #[case] questions: usize,
) -> Result<(), ScriptError> {
    // The last seat is attacked; the others hold three permanents that nothing touches.
    let defender = seats - 1;
    let table = || {
        annihilator_table(seats, vec![annihilator("Devourer", count)], |seat| {
            if seat == defender { held } else { 3 }
        })
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Devourer", defender)])?;
    let asked = settle(&mut driver, &[])?;
    assert_eq!(permanents(&driver, defender), left);
    assert_eq!(asked.sacrifices, questions);
    for seat in 1..defender {
        assert_eq!(permanents(&driver, seat), 3, "seat {seat}");
    }
    assert_eq!(permanents(&driver, 0), 1);
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
fn two_annihilators_hit_the_defending_players_they_attack(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        annihilator_table(
            seats,
            vec![annihilator("Small", 1), annihilator("Large", 2)],
            |_| 4,
        )
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Small", 1), ("Large", 2)])?;
    settle(&mut driver, &[])?;
    assert_eq!(permanents(&driver, 1), 3);
    assert_eq!(permanents(&driver, 2), 2);
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
fn two_instances_on_one_creature_sacrifice_both_numbers(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        annihilator_table(
            seats,
            vec![keyworded(
                "Twice",
                1,
                1,
                &[Keyword::Annihilator(1), Keyword::Annihilator(2)],
            )],
            |_| 5,
        )
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Twice", 1)])?;
    settle(&mut driver, &[])?;
    assert_eq!(permanents(&driver, 1), 2);
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
fn annihilator_still_works_when_the_creature_left_in_response(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        annihilator_table(seats, vec![annihilator("Devourer", 2), mountain()], |_| 4)
            .hand(0, [burn("Bolt", 3)])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.attack(&[("Devourer", 1)])?;
    let devourer = driver.target_named("Devourer")?;
    driver.cast_at("Bolt", &[devourer])?;
    settle(&mut driver, &[])?;
    assert!(driver.id_of("Devourer").is_err(), "the Devourer died");
    assert_eq!(permanents(&driver, 1), 2);
    assert_replays(&driver, &table().setup().unwrap())
}

fn extorter(name: &str) -> Arc<CardDefinition> {
    keyworded(name, 1, 1, &[Keyword::Extort])
}

/// Seat 0 controls the creatures and a Plains for each in `lands`, and holds a free Bear to cast.
fn extort_table(seats: usize, extorters: &[&str], lands: usize) -> Scenario {
    let mut mine: Vec<_> = extorters.iter().map(|name| ready(extorter(name))).collect();
    mine.extend((0..lands).map(|_| ready(plains())));
    let mut table = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .hand(0, [free_creature("Bear", 2, 2)]);
    for seat in 1..seats {
        table = table.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    table
}

#[rstest]
#[case::paid(&["Tax"], 1, &[true], 1, 1)]
#[case::declined(&["Tax"], 1, &[false], 1, 0)]
#[case::two_instances_paid(&["Tax", "Levy"], 2, &[true, true], 2, 2)]
#[case::one_of_two_paid(&["Tax", "Levy"], 2, &[true, false], 2, 1)]
#[case::one_land_for_two_instances(&["Tax", "Levy"], 1, &[true, true], 2, 1)]
#[case::no_mana_so_only_no(&["Tax"], 0, &[true], 1, 0)]
fn extort_drains_each_opponent_when_its_cost_is_paid(
    #[values(2, 3, 4)] seats: usize,
    #[case] extorters: &[&str],
    #[case] lands: usize,
    #[case] pays: &[bool],
    #[case] questions: usize,
    #[case] paid: i32,
) -> Result<(), ScriptError> {
    let table = || extort_table(seats, extorters, lands);
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Bear")?;
    let asked = settle(&mut driver, pays)?;
    assert_eq!(asked.payments, questions);
    let opponents = i32::try_from(seats - 1).unwrap();
    for seat in 1..seats {
        assert_eq!(life(&driver, seat), 20 - paid, "seat {seat}");
    }
    assert_eq!(life(&driver, 0), 20 + paid * opponents);
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
fn extort_ignores_the_spells_of_other_players(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 1, Step::Main1)
            .battlefield(0, [ready(extorter("Tax")), ready(plains())])
            .hand(1, [free_creature("Bear", 2, 2)])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Bear")?;
    assert_eq!(settle(&mut driver, &[true])?, Asked::default());
    assert_eq!(life(&driver, 0), 20);
    assert_replays(&driver, &table().setup().unwrap())
}

fn evolver() -> Arc<CardDefinition> {
    keyworded("Evo", 1, 1, &[Keyword::Evolve])
}

#[rstest]
#[case::more_power(&[(2, 1)], 1, (2, 2))]
#[case::more_toughness(&[(1, 2)], 1, (2, 2))]
#[case::neither(&[(1, 1)], 0, (1, 1))]
#[case::less_of_one_and_more_of_the_other(&[(0, 3)], 1, (2, 2))]
#[case::the_same_size_after_growing(&[(2, 2), (2, 2)], 1, (2, 2))]
#[case::grows_again(&[(2, 1), (3, 1)], 2, (3, 3))]
#[case::smaller_after_growing(&[(3, 3), (2, 2), (4, 1)], 2, (3, 3))]
fn evolve_compares_each_entering_creature_with_the_current_size(
    #[values(2, 3, 4)] seats: usize,
    #[case] entering: &[(i32, i32)],
    #[case] expected_counters: u32,
    #[case] expected_size: (i32, i32),
) -> Result<(), ScriptError> {
    let table = || {
        let hand: Vec<_> = entering
            .iter()
            .enumerate()
            .map(|(at, (power, toughness))| free_creature(&format!("Entrant{at}"), *power, *toughness))
            .collect();
        let mut table = Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(evolver())])
            .hand(0, hand);
        for seat in 1..seats {
            table = table.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
        }
        table
    };
    let mut driver = Driver::from_scenario(table())?;
    for at in 0..entering.len() {
        driver.cast(&format!("Entrant{at}"))?;
        settle(&mut driver, &[])?;
    }
    assert_eq!(counters(&driver, "Evo"), expected_counters);
    assert_eq!(size(&driver, "Evo"), expected_size);
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
fn each_instance_of_evolve_compares_for_itself(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(evolver()),
                    ready(keyworded("Big", 3, 3, &[Keyword::Evolve])),
                ],
            )
            .hand(0, [free_creature("Entrant", 2, 2)])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Entrant")?;
    settle(&mut driver, &[])?;
    assert_eq!(counters(&driver, "Evo"), 1);
    assert_eq!(counters(&driver, "Big"), 0);
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
fn evolve_ignores_creatures_that_enter_under_another_players_control(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 1, Step::Main1)
            .battlefield(0, [ready(evolver())])
            .hand(1, [free_creature("Giant", 5, 5)])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Giant")?;
    settle(&mut driver, &[])?;
    assert_eq!(counters(&driver, "Evo"), 0);
    assert_replays(&driver, &table().setup().unwrap())
}

#[rstest]
fn evolve_reads_the_creature_that_left_in_response_as_it_was(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let table = || {
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, [ready(evolver()), ready(mountain())])
            .hand(0, [free_creature("Entrant", 2, 1), burn("Bolt", 3)])
    };
    let mut driver = Driver::from_scenario(table())?;
    driver.cast("Entrant")?;
    // Everyone passes: the Entrant resolves and the evolve trigger goes on the stack above nothing.
    for _ in 0..seats {
        let events = driver.pass()?;
        if events
            .iter()
            .any(|event| matches!(event, mtg_engine::Event::AbilityTriggered(_)))
        {
            break;
        }
    }
    // The Entrant dies before the trigger resolves.
    let entrant = driver.target_named("Entrant")?;
    driver.cast_at("Bolt", &[entrant])?;
    settle(&mut driver, &[])?;
    assert!(driver.id_of("Entrant").is_err(), "the Entrant died");
    assert_eq!(counters(&driver, "Evo"), 1);
    assert_replays(&driver, &table().setup().unwrap())
}
