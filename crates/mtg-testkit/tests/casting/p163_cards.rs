//! Goal: the p163 cards play whole at two to four seats: Tahngarth, First Mate, tapped, may join an opponent's attack under
//! that opponent's control until end of combat, aimed by its owner at what that opponent attacks; Soul Echo keeps its
//! controller in the game at 0 life and lets the opponent it targets turn damage into echo counters removed until the next
//! upkeep, and is sacrificed with none left; Cyclopean Tomb makes lands Swamps with mire counters, and once in a graveyard
//! clears one of those lands at each of its owner's upkeeps for the rest of the game; each game replays and each card is
//! Verified with the engine's capabilities.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::card::Subtype;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice, TargetChoice, YesNoChoice};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, island, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready, tapped, with_counters};
use rstest::rstest;

use crate::common::pid;

fn p163_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn p163_libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 12])
    })
}

fn p163_life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

fn p163_counters(driver: &Driver, id: ObjectId, kind: &str) -> u32 {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::Other(kind.to_owned()))
        })
}

/// Answers `pick` gives first, then mana automatically, the first legal targets and passes, `steps` times at most or
/// until `stop` holds.
fn p163_run(
    driver: &mut Driver,
    steps: usize,
    pick: &dyn Fn(&Driver, &Decision) -> Option<Answer>,
    stop: &dyn Fn(&Driver) -> bool,
) {
    for _ in 0..steps {
        if stop(driver) {
            return;
        }
        let decision = driver.decision().unwrap().clone();
        if let Some(answer) = pick(driver, &decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        let answer = match &decision.kind {
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::ChooseTargets(options) => Answer::ChooseTargets(TargetChoice::Targets(
                options
                    .legal
                    .iter()
                    .copied()
                    .take(options.min.max(1) as usize)
                    .collect(),
            )),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
}

fn p163_at(turn: u32, step: Step) -> impl Fn(&Driver) -> bool {
    move |driver: &Driver| {
        let now = driver.state().turn();
        (now.number() == turn && now.step() >= step) || now.number() > turn
    }
}

#[rstest]
#[case::declined(false, false)]
#[case::its_owner_attacked(true, false)]
#[case::another_player_chosen(true, true)]
fn tahngarth_joins_an_opponents_attack_under_their_control(
    #[values(2, 3, 4)] seats: usize,
    #[case] joins: bool,
    #[case] elsewhere: bool,
) {
    // With two seats there is nobody else to attack: both Bears attack the owner.
    let other = if elsewhere && seats > 2 { 2 } else { 0 };
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(0, [tapped(card("Tahngarth, First Mate").unwrap())])
        .battlefield(
            1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Cub", 1, 1)),
            ],
        );
    let mut driver = Driver::from_scenario(p163_libraries(scenario, seats)).unwrap();
    let tahngarth = driver.id_of("Tahngarth, First Mate").unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Bear", 0), ("Cub", other)]).unwrap();
    let before: Vec<i32> = (0..seats).map(|seat| p163_life(&driver, seat)).collect();
    let asked_where = std::cell::Cell::new(false);
    p163_run(
        &mut driver,
        60,
        &|_, decision| match &decision.kind {
            DecisionKind::YesNo(_) if decision.player == pid(0) => {
                Some(Answer::YesNo(YesNoChoice(joins)))
            }
            DecisionKind::ChooseOption(options) => {
                assert_eq!(decision.player, pid(0), "the owner aims it");
                asked_where.set(true);
                let at = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::Player(pid(other)))
                    .unwrap();
                Some(Answer::ChooseOption(OptionChoice(at)))
            }
            _ => None,
        },
        &|driver| driver.state().turn().step() == Step::DeclareBlockers,
    );
    let attacking = driver
        .state()
        .combat()
        .attackers()
        .iter()
        .find(|attacker| attacker.object() == tahngarth)
        .map(mtg_engine::combat::Attacker::target);
    let controller = mtg_engine::characteristics::controller_of(driver.state(), tahngarth).unwrap();
    if joins {
        assert_eq!(attacking, Some(AttackTarget::Player(pid(other))));
        assert_eq!(controller, pid(1));
        assert_eq!(
            asked_where.get(),
            other != 0,
            "asked only between two players"
        );
    } else {
        assert_eq!(attacking, None);
        assert_eq!(controller, pid(0));
    }
    p163_run(&mut driver, 80, &|_, _| None, &|driver| {
        driver.state().turn().step() == Step::Main2
    });
    assert_eq!(
        mtg_engine::characteristics::controller_of(driver.state(), tahngarth).unwrap(),
        pid(0),
        "control returns at end of combat"
    );
    let mut lost = vec![0; seats];
    lost[0] += 2;
    lost[other] += 1;
    if joins {
        lost[other] += 5;
    }
    for seat in 0..seats {
        assert_eq!(
            p163_life(&driver, seat),
            before[seat] - lost[seat],
            "seat {seat}"
        );
    }
    p163_replays(&driver);
}

#[rstest]
fn tahngarth_untapped_does_not_trigger(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(2, 1, Step::Main1)
        .battlefield(0, [ready(card("Tahngarth, First Mate").unwrap())])
        .battlefield(1, [ready(free_creature("Bear", 2, 2))]);
    let mut driver = Driver::from_scenario(p163_libraries(scenario, seats)).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Bear", 0)]).unwrap();
    p163_run(
        &mut driver,
        60,
        &|_, decision| {
            assert!(
                !matches!(decision.kind, DecisionKind::YesNo(_)),
                "no question"
            );
            None
        },
        &|driver| driver.state().turn().step() == Step::Main2,
    );
    p163_replays(&driver);
}

#[rstest]
#[case::replaced(true, 5, 3)]
#[case::refused(false, 5, 3)]
#[case::fewer_counters_than_damage(true, 1, 3)]
fn soul_echo_turns_damage_into_echo_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] accept: bool,
    #[case] echoes: u32,
    #[case] power: i32,
) {
    let scenario = Scenario::new(seats)
        .at(u32::try_from(seats).unwrap(), seats - 1, Step::End)
        .life(0, 2)
        .battlefield(
            0,
            [with_counters(
                ready(card("Soul Echo").unwrap()),
                CounterKind::Other("echo".into()),
                echoes,
            )],
        )
        .battlefield(1, [ready(free_creature("Brute", power, power))]);
    let mut driver = Driver::from_scenario(p163_libraries(scenario, seats)).unwrap();
    let echo = driver.id_of("Soul Echo").unwrap();
    let turn = u32::try_from(seats).unwrap() + 1;
    // My upkeep: the trigger targets an opponent, who chooses.
    p163_run(
        &mut driver,
        60,
        &|_, decision| match &decision.kind {
            DecisionKind::YesNo(_) => {
                assert_ne!(decision.player, pid(0), "the opponent chooses");
                Some(Answer::YesNo(YesNoChoice(accept)))
            }
            _ => None,
        },
        &p163_at(turn, Step::Draw),
    );
    // Seat 1 attacks me on its turn.
    driver.idle_until(turn + 1, Step::Main1).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Brute", 0)]).unwrap();
    p163_run(&mut driver, 80, &|_, _| None, &|driver| {
        driver.state().turn().step() == Step::Main2
    });
    let damage = u32::try_from(power).unwrap();
    if accept {
        assert_eq!(p163_life(&driver, 0), 2, "the damage was replaced");
        assert_eq!(
            p163_counters(&driver, echo, "echo"),
            echoes.saturating_sub(damage)
        );
    } else {
        assert_eq!(p163_life(&driver, 0), 2 - power);
        assert_eq!(p163_counters(&driver, echo, "echo"), echoes);
    }
    assert!(driver.state().is_alive(pid(0)), "no loss at 0 or less life");
    p163_replays(&driver);
}

#[rstest]
fn soul_echo_without_counters_is_sacrificed(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats)
        .at(u32::try_from(seats).unwrap(), seats - 1, Step::End)
        .battlefield(0, [ready(card("Soul Echo").unwrap())]);
    let mut driver = Driver::from_scenario(p163_libraries(scenario, seats)).unwrap();
    let turn = u32::try_from(seats).unwrap() + 1;
    p163_run(
        &mut driver,
        60,
        &|_, decision| {
            assert!(
                !matches!(decision.kind, DecisionKind::YesNo(_)),
                "no choice"
            );
            None
        },
        &p163_at(turn, Step::Draw),
    );
    assert!(
        driver.id_of("Soul Echo").is_err() || {
            let id = driver.id_of("Soul Echo").unwrap();
            driver.state().object(id).unwrap().zone() == mtg_engine::zones::Zone::Graveyard(pid(0))
        }
    );
    p163_replays(&driver);
}

fn p163_is_swamp(driver: &Driver, id: ObjectId) -> bool {
    let object = driver.state().object(id).unwrap();
    mtg_engine::characteristics::characteristics(driver.state(), id)
        .unwrap()
        .has_subtype(&object.card().subtypes, &Subtype("Swamp".to_owned()))
}

#[rstest]
fn cyclopean_tomb_clears_one_land_each_upkeep_after_it_dies(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] mired: usize,
) {
    let seats_turns = u32::try_from(seats).unwrap();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Upkeep)
        .battlefield(
            0,
            [
                ready(card("Cyclopean Tomb").unwrap()),
                ready(swamp()),
                ready(swamp()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .battlefield(1, [ready(island()), ready(island())])
        .hand(0, [card("Smash to Smithereens").unwrap()]);
    let mut driver = Driver::from_scenario(p163_libraries(scenario, seats)).unwrap();
    let tomb = driver.id_of("Cyclopean Tomb").unwrap();
    let lands: Vec<ObjectId> = driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().controller() == pid(1))
        .collect();
    for (at, land) in lands.iter().take(mired).enumerate() {
        let turn = 1 + seats_turns * u32::try_from(at).unwrap();
        driver.idle_until(turn, Step::Upkeep).unwrap();
        driver.begin_activation("Cyclopean Tomb").unwrap();
        let land = *land;
        p163_run(
            &mut driver,
            40,
            &move |_, decision| match &decision.kind {
                DecisionKind::ChooseTargets(options) => {
                    Some(Answer::ChooseTargets(TargetChoice::Targets(
                        options
                            .legal
                            .iter()
                            .copied()
                            .filter(|target| target.object().is_some_and(|o| o.id == land))
                            .collect(),
                    )))
                }
                _ => None,
            },
            &|driver| driver.state().stack().is_empty() && p163_counters(driver, land, "mire") == 1,
        );
        assert!(p163_is_swamp(&driver, land));
    }
    // Smash the Tomb in my main phase.
    let turn = 1 + seats_turns * u32::try_from(mired - 1).unwrap();
    driver.idle_until(turn, Step::Main1).unwrap();
    driver.begin_cast("Smash to Smithereens").unwrap();
    p163_run(
        &mut driver,
        40,
        &move |_, decision| match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                Some(Answer::ChooseTargets(TargetChoice::Targets(
                    options
                        .legal
                        .iter()
                        .copied()
                        .filter(|target| target.object().is_some_and(|o| o.id == tomb))
                        .collect(),
                )))
            }
            _ => None,
        },
        &|driver| {
            driver.state().stack().is_empty()
                && driver.id_of("Cyclopean Tomb").is_ok_and(|id| id != tomb)
        },
    );
    for upkeep in 1..=mired + 1 {
        let next = turn + seats_turns * u32::try_from(upkeep).unwrap();
        p163_run(
            &mut driver,
            200,
            &|_, decision| match &decision.kind {
                DecisionKind::ChooseCards(options) => {
                    Some(Answer::ChooseCards(CardsChoice(vec![options.from[0]])))
                }
                _ => None,
            },
            &p163_at(next, Step::Draw),
        );
        let cleared = lands
            .iter()
            .take(mired)
            .filter(|land| p163_counters(&driver, **land, "mire") == 0)
            .count();
        assert_eq!(cleared, upkeep.min(mired), "upkeep {upkeep}");
        for land in lands.iter().take(mired) {
            assert_eq!(
                p163_is_swamp(&driver, *land),
                p163_counters(&driver, *land, "mire") > 0
            );
        }
    }
    p163_replays(&driver);
}

#[rstest]
#[case("Tahngarth, First Mate")]
#[case("Soul Echo")]
#[case("Cyclopean Tomb")]
fn p163_the_cards_are_verified(#[case] name: &str) {
    let support = mtg_testkit::corpus::corpus()
        .unwrap()
        .support(name)
        .unwrap();
    assert_eq!(
        support.support,
        mtg_cards::support::Support::Verified,
        "{name}: {support:?}"
    );
}
