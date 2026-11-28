//! Goal: Space Beleren's space sculptor at two to four seats: while it is on the battlefield every creature is assigned
//! to a sector by its controller, the players who control no space sculptor first in turn order from the active player
//! and its controller last, and a creature that arrives later is asked about alone; its +1 lets a creature in a sector
//! be blocked only by creatures in the same sector, which nothing restricts without it; its −1 and −5 put a counter on
//! and destroy exactly the creatures in the sector chosen; a creature keeps its sector when its control changes; every
//! sector is lost once no space sculptor is left; each game replays and the card is Verified.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::abilities::AbilityRef;
use mtg_engine::choose::{ChoiceOption, OptionChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::events::Event;
use mtg_engine::p170a_sectors::Sector;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::pid;

const P170A_CARD: &str = "Space Beleren";

fn p170a_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Seat 0's Space Beleren with `loyalty` counters, a creature named `Seat N` for every seat, and libraries, in seat
/// `active`'s first main phase of turn 2, with `lands` Mountains for seat 0.
fn p170a_scenario(seats: usize, active: usize, loyalty: u32, lands: usize) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats).at(2, active, Step::Main1),
        |scenario, seat| {
            let creature = ready(free_creature(&format!("Seat {seat}"), 2, 2));
            let permanents: Vec<_> = if seat == 0 {
                let beleren = with_counters(
                    ready(card(P170A_CARD).unwrap()),
                    CounterKind::Loyalty,
                    loyalty,
                );
                [beleren, creature]
                    .into_iter()
                    .chain((0..lands).map(|_| ready(mountain())))
                    .collect()
            } else {
                vec![creature]
            };
            scenario
                .battlefield(seat, permanents)
                .library(seat, vec![island(); 12])
        },
    )
}

/// The sector of the permanent `id`, if it is in one.
fn p170a_sector(driver: &Driver, id: ObjectId) -> Option<Sector> {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .and_then(mtg_engine::object::PermanentState::sector)
}

/// The one card in seat 0's hand.
fn p170a_in_hand(driver: &Driver) -> ObjectId {
    driver
        .state()
        .zone(Zone::Hand(pid(0)))
        .first()
        .copied()
        .unwrap()
}

/// Answers each sector question with `sectors` (alpha for a creature not named) and records who was asked about what,
/// until a decision of another kind is pending.
fn p170a_assign(
    driver: &mut Driver,
    sectors: &BTreeMap<ObjectId, Sector>,
) -> Vec<(PlayerId, Vec<ObjectId>)> {
    let mut asked = Vec::new();
    while let DecisionKind::P170aAssignSectors(options) = &driver.decision().unwrap().kind {
        let player = driver.decision().unwrap().player;
        let creatures = options.creatures.clone();
        let answer = creatures
            .iter()
            .map(|id| (*id, sectors.get(id).copied().unwrap_or(Sector::Alpha)))
            .collect();
        driver.answer(Answer::P170aAssignSectors(answer)).unwrap();
        asked.push((player, creatures));
    }
    asked
}

/// Answers mana automatically, the first legal targets, the sector `chosen` and passes, `steps` times at most or until
/// `stop` holds; `pick` answers first.
fn p170a_run(
    driver: &mut Driver,
    steps: usize,
    chosen: Sector,
    pick: &mut dyn FnMut(&Driver, &Decision) -> Option<Answer>,
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
            DecisionKind::ChooseOption(options) => Answer::ChooseOption(OptionChoice(
                options
                    .options
                    .iter()
                    .position(|option| matches!(option, ChoiceOption::Label(word) if word == chosen.word()))
                    .unwrap(),
            )),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
}

/// Activates the loyalty ability at `index` among Space Beleren's rules and resolves it, choosing `chosen`.
fn p170a_activate(driver: &mut Driver, index: usize, chosen: Sector) {
    let beleren = driver.id_of(P170A_CARD).unwrap();
    driver
        .answer(Answer::Priority(PriorityAction::Activate(
            AbilityRef::printed(beleren, index),
        )))
        .unwrap();
    let before = driver.state().stack().len();
    p170a_run(driver, 40, chosen, &mut |_, _| None, &|driver| {
        driver.state().stack().len() < before
            && matches!(
                driver.decision().map(|d| &d.kind),
                Ok(DecisionKind::Priority(_))
            )
    });
}

#[rstest]
#[case::active_holds_it(0)]
#[case::active_is_an_opponent(1)]
fn p170a_opponents_assign_first_in_turn_order(
    #[values(2, 3, 4)] seats: usize,
    #[case] active: usize,
) {
    let mut driver = Driver::from_scenario(p170a_scenario(seats, active, 3, 0)).unwrap();
    let asked = p170a_assign(&mut driver, &BTreeMap::new());
    let order: Vec<PlayerId> = asked.iter().map(|(player, _)| *player).collect();
    let mut expected: Vec<PlayerId> = (0..seats)
        .map(|at| pid((active + at) % seats))
        .filter(|player| *player != pid(0))
        .collect();
    expected.push(pid(0));
    assert_eq!(order, expected);
    for (player, creatures) in &asked {
        let name = format!("Seat {}", player.0);
        assert_eq!(creatures, &vec![driver.id_of(&name).unwrap()], "{name}");
    }
    let mine = driver.id_of("Seat 0").unwrap();
    assert_eq!(p170a_sector(&driver, mine), Some(Sector::Alpha));
    p170a_replays(&driver);
}

#[rstest]
fn p170a_a_creature_that_arrives_is_asked_about_alone(#[values(2, 3, 4)] seats: usize) {
    let scenario = p170a_scenario(seats, 0, 3, 0).hand(0, [free_creature("Late", 1, 1)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let first = driver.id_of("Seat 0").unwrap();
    p170a_assign(&mut driver, &BTreeMap::from([(first, Sector::Gamma)]));
    let late = p170a_in_hand(&driver);
    driver
        .answer(Answer::Priority(PriorityAction::CastSpell(late)))
        .unwrap();
    let mut asked = Vec::new();
    p170a_run(
        &mut driver,
        30,
        Sector::Alpha,
        &mut |_, decision| match &decision.kind {
            DecisionKind::P170aAssignSectors(options) => {
                asked.push((decision.player, options.creatures.clone()));
                Some(Answer::P170aAssignSectors(vec![(late, Sector::Beta)]))
            }
            _ => None,
        },
        &|driver| {
            driver.state().zone(Zone::Battlefield).contains(&late)
                && p170a_sector(driver, late).is_some()
        },
    );
    assert_eq!(asked, vec![(pid(0), vec![late])]);
    assert_eq!(p170a_sector(&driver, late), Some(Sector::Beta));
    assert_eq!(
        p170a_sector(&driver, first),
        Some(Sector::Gamma),
        "an assignment stays"
    );
    p170a_replays(&driver);
}

#[rstest]
#[case::plus_same_sector(true, Sector::Alpha, true)]
#[case::plus_other_sector(true, Sector::Beta, false)]
#[case::plus_third_sector(true, Sector::Gamma, false)]
#[case::without_plus_other_sector(false, Sector::Beta, true)]
#[case::without_plus_same_sector(false, Sector::Alpha, true)]
fn p170a_blocks_stay_within_a_sector_after_the_plus(
    #[values(2, 3)] seats: usize,
    #[case] plus: bool,
    #[case] blocker_sector: Sector,
    #[case] can_block: bool,
) {
    let mut driver = Driver::from_scenario(p170a_scenario(seats, 0, 3, 0)).unwrap();
    let attacker = driver.id_of("Seat 0").unwrap();
    let blocker = driver.id_of("Seat 1").unwrap();
    p170a_assign(
        &mut driver,
        &BTreeMap::from([(attacker, Sector::Alpha), (blocker, blocker_sector)]),
    );
    if plus {
        p170a_activate(&mut driver, 1, Sector::Alpha);
    }
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Seat 0", 1)]).unwrap();
    let mut offered = None;
    p170a_run(
        &mut driver,
        30,
        Sector::Alpha,
        &mut |_, decision| match &decision.kind {
            DecisionKind::DeclareBlockers(options) if decision.player == pid(1) => {
                offered = Some(options.blockers.iter().any(|option| {
                    option.object == blocker && option.can_block.contains(&attacker)
                }));
                Some(Answer::DeclareBlockers(Vec::new()))
            }
            _ => None,
        },
        &|driver| driver.state().turn().step() >= Step::CombatDamage,
    );
    // With no creature able to block, the defending player is not asked at all.
    assert_eq!(offered.unwrap_or(false), can_block);
    p170a_replays(&driver);
}

#[rstest]
#[case::alpha(Sector::Alpha)]
#[case::beta(Sector::Beta)]
#[case::gamma(Sector::Gamma)]
fn p170a_minus_one_and_minus_five_reach_only_the_sector_chosen(
    #[values(2, 4)] seats: usize,
    #[case] chosen: Sector,
    #[values(2, 3)] index: usize,
) {
    let mut driver = Driver::from_scenario(p170a_scenario(seats, 0, 6, 0)).unwrap();
    let creatures: Vec<ObjectId> = (0..seats)
        .map(|seat| driver.id_of(&format!("Seat {seat}")).unwrap())
        .collect();
    let sectors: BTreeMap<ObjectId, Sector> = creatures
        .iter()
        .enumerate()
        .map(|(at, id)| (*id, Sector::ALL[at % 3]))
        .collect();
    p170a_assign(&mut driver, &sectors);
    p170a_activate(&mut driver, index, chosen);
    for (id, sector) in &sectors {
        let object = driver.state().object(*id).unwrap();
        let hit = *sector == chosen;
        if index == 2 {
            let counters = object
                .permanent()
                .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne));
            assert_eq!(counters, u32::from(hit), "{sector:?}");
        } else {
            assert_eq!(object.zone() == Zone::Battlefield, !hit, "{sector:?}");
        }
    }
    p170a_replays(&driver);
}

#[rstest]
fn p170a_a_creature_keeps_its_sector_when_its_control_changes(#[values(2, 3, 4)] seats: usize) {
    let scenario = p170a_scenario(seats, 0, 3, 3).hand(0, [card("Act of Treason").unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let theirs = driver.id_of("Seat 1").unwrap();
    p170a_assign(&mut driver, &BTreeMap::from([(theirs, Sector::Gamma)]));
    let spell = p170a_in_hand(&driver);
    driver
        .answer(Answer::Priority(PriorityAction::CastSpell(spell)))
        .unwrap();
    let mut asked = 0;
    p170a_run(
        &mut driver,
        30,
        Sector::Alpha,
        &mut |_, decision| {
            match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                Some(Answer::ChooseTargets(TargetChoice::Targets(
                    options
                        .legal
                        .iter()
                        .copied()
                        .filter(|target| matches!(target, Target::Object(object) if object.id == theirs))
                        .collect(),
                )))
            }
            DecisionKind::P170aAssignSectors(_) => {
                asked += 1;
                None
            }
            _ => None,
        }
        },
        &|driver| {
            driver.state().zone(Zone::Stack).is_empty()
                && mtg_engine::characteristics::controller_of(driver.state(), theirs)
                    == Some(pid(0))
        },
    );
    assert_eq!(
        mtg_engine::characteristics::controller_of(driver.state(), theirs),
        Some(pid(0))
    );
    assert_eq!(asked, 0, "its new controller is not asked");
    assert_eq!(p170a_sector(&driver, theirs), Some(Sector::Gamma));
    p170a_replays(&driver);
}

#[rstest]
fn p170a_every_sector_is_lost_once_no_sculptor_is_left(#[values(2, 3, 4)] seats: usize) {
    // Five loyalty: the −5 puts Space Beleren into its owner's graveyard as it resolves.
    let mut driver = Driver::from_scenario(p170a_scenario(seats, 0, 5, 0)).unwrap();
    let creatures: Vec<ObjectId> = (0..seats)
        .map(|seat| driver.id_of(&format!("Seat {seat}")).unwrap())
        .collect();
    p170a_assign(
        &mut driver,
        &creatures.iter().map(|id| (*id, Sector::Beta)).collect(),
    );
    let from = driver.engine().events().len();
    let beleren = driver.id_of(P170A_CARD).unwrap();
    p170a_activate(&mut driver, 3, Sector::Gamma);
    assert_ne!(
        driver.state().object(beleren).unwrap().zone(),
        Zone::Battlefield
    );
    assert!(driver.engine().events()[from..].contains(&Event::P170aSectorsLost));
    for id in creatures {
        assert_eq!(driver.state().object(id).unwrap().zone(), Zone::Battlefield);
        assert_eq!(p170a_sector(&driver, id), None);
    }
    assert!(!matches!(
        driver.decision().unwrap().kind,
        DecisionKind::P170aAssignSectors(_)
    ));
    p170a_replays(&driver);
}

#[test]
fn p170a_space_beleren_is_verified() {
    let support = mtg_testkit::corpus::corpus()
        .unwrap()
        .support(P170A_CARD)
        .unwrap();
    assert_eq!(
        support.support,
        mtg_cards::support::Support::Verified,
        "{support:?}"
    );
}
