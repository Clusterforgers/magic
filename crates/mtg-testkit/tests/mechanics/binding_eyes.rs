//! Goal: a boss's side aims its attacks and its targeted removal at the hero each seat eyes, at two, three and four
//! heroes, and leaves that hero only for a named reason (it can't be attacked or targeted, another hero is an
//! overwhelming threat or could kill the boss, an attack at it is futile, an attack kills another hero), which the log
//! carries as `FocusShifted`; minions eye the next hero so the side splits.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::spotlight::{emblem, turn_of};

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, AttackOptions, DecisionKind};
use mtg_engine::events::Event;
use mtg_engine::phases::BossPhase;
use mtg_engine::spotlight;
use mtg_engine::spotlight_aim::{self, AimWhy};
use mtg_engine::turn::Step;
use mtg_testkit::brain::seen::Seen;
use mtg_testkit::brain::{Level, brain, combat, targets};
use mtg_testkit::cards::{burn, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// One hero: life, creatures, the cards in hand, artifacts (2 presence each) and keywords.
#[derive(Clone)]
struct Hero {
    life: i32,
    creatures: Vec<Arc<CardDefinition>>,
    hand: usize,
    rocks: usize,
    keywords: Vec<Keyword>,
}

fn hero(life: i32) -> Hero {
    Hero {
        life,
        creatures: Vec::new(),
        hand: 0,
        rocks: 0,
        keywords: Vec::new(),
    }
}

impl Hero {
    fn with(mut self, creatures: &[(&str, i32, i32)]) -> Hero {
        self.creatures = creatures
            .iter()
            .map(|(name, power, toughness)| free_creature(name, *power, *toughness))
            .collect();
        self
    }

    fn cards(mut self, hand: usize) -> Hero {
        self.hand = hand;
        self
    }

    fn rocks(mut self, rocks: usize) -> Hero {
        self.rocks = rocks;
        self
    }
}

fn rock(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Artifact))
}

/// A fight in `Teams` mode: the heroes, then the boss (life `boss_life`, these creatures and `hand` spells), then
/// `minions` minions with a Bear each.
fn fight(
    heroes: &[Hero],
    boss_life: i32,
    boss_board: &[(&str, i32, i32)],
    boss_hand: Vec<Arc<CardDefinition>>,
    minions: usize,
) -> Scenario {
    let count = heroes.len();
    let mut seats = vec![0; count];
    seats.extend(std::iter::repeat_n(1, 1 + minions));
    let house = HouseRules {
        teams: Some(Teams {
            mode: TeamMode::Teams,
            seats,
        }),
        ..HouseRules::default()
    };
    let mut scenario = Scenario::new(count + 1 + minions).house_rules(house);
    for (seat, hero) in heroes.iter().enumerate() {
        let mut board: Vec<_> = hero.creatures.iter().cloned().map(ready).collect();
        board.extend((0..hero.rocks).map(|i| ready(rock(&format!("Rock {seat}.{i}")))));
        scenario = scenario
            .life(seat, hero.life)
            .spirit(seat, emblem("Haunt", &[]))
            .battlefield(seat, board)
            .hand(
                seat,
                (0..hero.hand).map(|i| free_creature(&format!("Card {seat}.{i}"), 1, 1)),
            )
            .player_keywords(seat, hero.keywords.clone());
    }
    let boss = count;
    scenario = scenario
        .life(boss, boss_life)
        .phases(
            boss,
            [BossPhase {
                at_life: 1,
                emblem: emblem("Wrath", &[]),
            }],
        )
        .battlefield(
            boss,
            boss_board
                .iter()
                .map(|(name, power, toughness)| ready(free_creature(name, *power, *toughness))),
        )
        .hand(boss, boss_hand);
    for at in 0..minions {
        let seat = boss + 1 + at;
        scenario = scenario
            .leader(seat, boss)
            .battlefield(seat, [ready(free_creature(&format!("Minion {at}"), 3, 3))]);
    }
    scenario
}

/// The game at the declare-attackers decision of `seat`'s third turn (the lull is over), and the seat's options.
fn at_attack(scenario: Scenario, seat: usize, seats: usize) -> (Driver, AttackOptions) {
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .idle_until(turn_of(seat, 3, seats), Step::DeclareAttackers)
        .unwrap();
    let decision = driver.decision().unwrap().clone();
    assert_eq!(decision.player, pid(seat));
    let DecisionKind::DeclareAttackers(options) = decision.kind else {
        panic!("expected the declare-attackers decision")
    };
    (driver, options)
}

/// The heroes each attacker of `attack` goes at.
fn aimed_at(attack: &[(mtg_core::ids::ObjectId, AttackTarget)]) -> Vec<PlayerId> {
    let mut heroes: Vec<PlayerId> = attack
        .iter()
        .filter_map(|(_, target)| match target {
            AttackTarget::Player(player) => Some(*player),
            AttackTarget::Planeswalker(_) => None,
        })
        .collect();
    heroes.sort();
    heroes.dedup();
    heroes
}

fn shifts(events: &[Event]) -> Vec<&Event> {
    events
        .iter()
        .filter(|event| matches!(event, Event::FocusShifted { .. }))
        .collect()
}

/// Hero 0 holds a full hand (the hero eyed), the others a chump blocker and a low life that bait a hunter of weakness.
fn baited(heroes: usize) -> Vec<Hero> {
    (0..heroes)
        .map(|seat| {
            if seat == 0 {
                hero(30).cards(7)
            } else {
                hero(3).with(&[(&format!("Chump {seat}"), 2, 2)])
            }
        })
        .collect()
}

#[rstest]
#[case::two_heroes(2)]
#[case::three_heroes(3)]
#[case::four_heroes(4)]
fn attacks_go_at_the_eyed_hero_not_the_weakest(
    #[case] heroes: usize,
    #[values(Level::Normal, Level::Hard)] level: Level,
) -> Result<(), ScriptError> {
    let scenario = fight(&baited(heroes), 40, &[("Boss Bear", 3, 3)], Vec::new(), 0);
    let (mut driver, options) = at_attack(scenario, heroes, heroes + 1);
    let eyed = spotlight::focus(driver.state(), pid(heroes)).unwrap();
    assert_eq!(eyed, pid(0), "the full hand is the most presence");
    let decision = driver.decision()?.clone();
    let answer = brain(level, 7, pid(heroes), false).choose(&decision, Some(driver.engine()));
    let Answer::DeclareAttackers(attack) = &answer else {
        panic!("expected an attack, got {answer:?}")
    };
    assert!(!attack.is_empty(), "the Bear attacks");
    assert_eq!(aimed_at(attack), vec![eyed]);
    let aim = spotlight_aim::attack_aim(driver.state(), pid(heroes), &options).unwrap();
    assert_eq!((aim.hero, aim.why), (eyed, None));
    let events = driver.answer(answer)?;
    assert!(shifts(&events).is_empty(), "no reason is needed");
    Ok(())
}

/// A fight at three heroes whose third turn of the boss is reached, with `tweak` applied to the state the boss's
/// brain reads and `options` altered by `adjust`; the boss's attack and the log note it would make.
fn decide(
    scenario: Scenario,
    heroes: usize,
    tweak: impl FnOnce(&mut mtg_engine::GameState),
    adjust: impl FnOnce(&mut AttackOptions, PlayerId),
) -> (PlayerId, spotlight_aim::Aim, Vec<PlayerId>, Option<Event>) {
    let (driver, mut options) = at_attack(scenario, heroes, heroes + 1);
    let boss = pid(heroes);
    let mut state = driver.state().clone();
    tweak(&mut state);
    let eyed = spotlight::focus(&state, boss).unwrap();
    adjust(&mut options, eyed);
    let seen = Seen::new(&state, boss);
    let attack = combat::quick_attack(&seen, &options);
    assert!(!attack.is_empty(), "the boss attacks");
    let aim = spotlight_aim::attack_aim(&state, boss, &options).unwrap();
    let note = spotlight_aim::shifted(&state, boss, &options, &attack);
    (eyed, aim, aimed_at(&attack), note)
}

fn attention(state: &mut mtg_engine::GameState, hero: usize, amount: u32) {
    let source = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            mtg_engine::characteristics::controller_of(state, *id)
                .is_some_and(|seat| spotlight::is_boss_side(state, seat))
        })
        .unwrap();
    mtg_engine::apply_event(
        state,
        &Event::DamageDealt {
            source,
            target: mtg_engine::combat::DamageTarget::Player(pid(hero)),
            amount,
            deathtouch: false,
            form: mtg_engine::combat::DamageForm::default(),
            toxic: 0,
            unpreventable: false,
        },
    )
    .unwrap();
}

#[test]
fn an_eyed_hero_who_cannot_be_attacked_is_left_with_that_reason() {
    let scenario = fight(&baited(3), 40, &[("Boss Bear", 3, 3)], Vec::new(), 0);
    let (eyed, aim, heroes, note) = decide(
        scenario,
        3,
        |_| {},
        |options, eyed| {
            for option in &mut options.attackers {
                option
                    .defenders
                    .retain(|target| *target != AttackTarget::Player(eyed));
            }
        },
    );
    assert_eq!(eyed, pid(0));
    assert_ne!(aim.hero, eyed);
    assert_eq!(aim.why, Some(AimWhy::Unreachable));
    assert_eq!(heroes, vec![aim.hero]);
    assert!(matches!(
        note,
        Some(Event::FocusShifted { eyed: e, instead, why: AimWhy::Unreachable, .. })
            if e == eyed && instead == aim.hero
    ));
}

#[test]
fn an_attack_at_the_eyed_hero_that_would_only_lose_creatures_goes_elsewhere() {
    // Hero 0 is eyed (a full hand) and blocks the Bear with a bigger creature; hero 1 has no blocker.
    let heroes = vec![
        hero(30).cards(7).with(&[("Wall", 4, 4)]),
        hero(30),
        hero(30),
    ];
    let scenario = fight(&heroes, 40, &[("Boss Bear", 3, 3)], Vec::new(), 0);
    let (eyed, aim, aimed, note) = decide(scenario, 3, |_| {}, |_, _| {});
    assert_eq!(eyed, pid(0));
    assert_eq!(aim.why, Some(AimWhy::Futile));
    assert_ne!(aim.hero, eyed);
    assert_eq!(aimed, vec![aim.hero]);
    assert!(matches!(
        note,
        Some(Event::FocusShifted {
            why: AimWhy::Futile,
            ..
        })
    ));
}

#[test]
fn a_kill_on_another_hero_is_always_taken() {
    // Hero 0 is eyed and no pushover; hero 1 is at 5 life with nothing to block two Bears.
    let heroes = vec![hero(30).cards(7), hero(5), hero(30)];
    let scenario = fight(
        &heroes,
        40,
        &[("Boss Bear", 3, 3), ("Boss Ogre", 3, 3)],
        Vec::new(),
        0,
    );
    let (eyed, aim, aimed, note) = decide(scenario, 3, |_| {}, |_, _| {});
    assert_eq!(eyed, pid(0));
    assert_eq!((aim.hero, aim.why), (pid(1), Some(AimWhy::Lethal)));
    assert_eq!(aimed, vec![pid(1)]);
    assert!(matches!(
        note,
        Some(Event::FocusShifted { why: AimWhy::Lethal, instead, .. }) if instead == pid(1)
    ));
}

#[test]
fn an_overwhelming_hero_is_attacked_whoever_is_eyed() {
    // Hero 0 towers (rocks and creatures) but has taken the pressure, so the boss eyes a smaller hero.
    let heroes = vec![
        hero(30).rocks(12).with(&[("Giant", 4, 4)]),
        hero(30).cards(7),
        hero(30).cards(7),
    ];
    let scenario = fight(&heroes, 40, &[("Boss Bear", 5, 5)], Vec::new(), 0);
    let (eyed, aim, aimed, note) = decide(scenario, 3, |state| attention(state, 0, 25), |_, _| {});
    assert_ne!(eyed, pid(0), "the pressure moved the eye");
    assert_eq!((aim.hero, aim.why), (pid(0), Some(AimWhy::Overwhelming)));
    assert_eq!(aimed, vec![pid(0)]);
    assert!(matches!(
        note,
        Some(Event::FocusShifted {
            why: AimWhy::Overwhelming,
            ..
        })
    ));
}

#[test]
fn a_hero_whose_creatures_could_kill_the_boss_is_attacked_whoever_is_eyed() {
    // Hero 1 is eyed by presence (rocks), hero 0's creatures add up to the boss's life.
    let heroes = vec![
        hero(30).with(&[("Giant", 5, 5), ("Ogre", 4, 4)]),
        hero(30).rocks(4).cards(7),
    ];
    let scenario = fight(&heroes, 9, &[("Boss Bear", 6, 6)], Vec::new(), 0);
    let (eyed, aim, aimed, note) = decide(scenario, 2, |_| {}, |_, _| {});
    assert_eq!(eyed, pid(1));
    assert_eq!(aim.hero, pid(0));
    assert_eq!(aim.why, Some(AimWhy::Deadly { boss: pid(2) }));
    assert_eq!(aimed, vec![pid(0)]);
    assert!(matches!(
        note,
        Some(Event::FocusShifted {
            why: AimWhy::Deadly { .. },
            ..
        })
    ));
}

#[test]
fn a_fairly_sized_party_gets_no_reason_when_the_eyed_hero_is_attacked() {
    let heroes = vec![hero(30).cards(7), hero(30).with(&[("Bear", 2, 2)])];
    let scenario = fight(&heroes, 40, &[("Boss Bear", 3, 3)], Vec::new(), 0);
    let (eyed, aim, aimed, note) = decide(scenario, 2, |_| {}, |_, _| {});
    assert_eq!((aim.hero, aim.why), (eyed, None));
    assert_eq!(aimed, vec![eyed]);
    assert!(note.is_none());
}

#[test]
fn the_log_carries_the_reason_when_the_declared_attack_leaves_the_eyed_hero()
-> Result<(), ScriptError> {
    let heroes = vec![hero(30).cards(7), hero(5), hero(30)];
    let scenario = fight(
        &heroes,
        40,
        &[("Boss Bear", 3, 3), ("Boss Ogre", 3, 3)],
        Vec::new(),
        0,
    );
    let (mut driver, _) = at_attack(scenario, 3, 4);
    let decision = driver.decision()?.clone();
    let answer = brain(Level::Hard, 7, pid(3), false).choose(&decision, Some(driver.engine()));
    let events = driver.answer(answer)?;
    let found = shifts(&events);
    assert_eq!(found.len(), 1);
    assert!(matches!(
        found[0],
        Event::FocusShifted { player, eyed, instead, why: AimWhy::Lethal }
            if *player == pid(3) && *eyed == pid(0) && *instead == pid(1)
    ));
    Ok(())
}

#[test]
fn a_minion_eyes_the_next_hero_so_the_side_splits() -> Result<(), ScriptError> {
    let heroes = vec![
        hero(40).cards(7).with(&[("Cat", 1, 1)]),
        hero(40).cards(5).with(&[("Dog", 1, 1)]),
        hero(40).cards(3).with(&[("Eel", 1, 1)]),
    ];
    let scenario = fight(&heroes, 60, &[("Boss Bear", 1, 1)], Vec::new(), 1);
    let (boss, minion) = (3, 4);
    let state = Driver::from_scenario(scenario.clone())?.state().clone();
    assert_ne!(
        spotlight::focus(&state, pid(boss)),
        spotlight::focus(&state, pid(minion))
    );
    let (driver, options) = at_attack(scenario.clone(), minion, 5);
    let eyed = spotlight::focus(driver.state(), pid(minion)).unwrap();
    assert_ne!(Some(eyed), spotlight::focus(driver.state(), pid(boss)));
    let seen = Seen::new(driver.state(), pid(minion));
    let attack = combat::quick_attack(&seen, &options);
    assert!(!attack.is_empty());
    let aim = spotlight_aim::attack_aim(driver.state(), pid(minion), &options).unwrap();
    assert_eq!(aimed_at(&attack), vec![aim.hero]);
    assert_eq!(
        aim.hero, eyed,
        "the minion's attack goes at the hero it eyes"
    );
    Ok(())
}

/// Hero 0 is eyed (a full hand); the others hold the juicier creatures. The boss has `Zap` in hand.
fn zap_table(heroes: usize, hexproof_eyed: bool) -> Scenario {
    let all: Vec<Hero> = (0..heroes)
        .map(|seat| {
            if seat == 0 {
                let mut eyed = hero(30).cards(7).rocks(2);
                if !hexproof_eyed {
                    eyed = eyed.with(&[("Pest", 1, 1)]);
                } else {
                    eyed.keywords = vec![Keyword::Hexproof];
                }
                eyed
            } else {
                hero(30).with(&[(&format!("Titan {seat}"), 4, 4)])
            }
        })
        .collect();
    fight(&all, 40, &[], vec![burn("Zap", 3)], 0).battlefield(heroes, [ready(mountain())])
}

fn ranked_targets(
    scenario: Scenario,
    seat: usize,
    seats: usize,
) -> (Driver, Vec<mtg_engine::choose::Target>) {
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver
        .idle_until(turn_of(seat, 3, seats), Step::Main1)
        .unwrap();
    driver.begin_cast("Zap").unwrap();
    let DecisionKind::ChooseTargets(options) = driver.decision().unwrap().kind.clone() else {
        panic!("expected a target decision")
    };
    let seen = Seen::new(driver.state(), pid(seat));
    let ranked = targets::rank_targets(&seen, &options);
    (driver, ranked)
}

fn holder(driver: &Driver, target: mtg_engine::choose::Target) -> PlayerId {
    match target {
        mtg_engine::choose::Target::Player(player) => player,
        mtg_engine::choose::Target::Object(object) => {
            mtg_engine::characteristics::controller_of(driver.state(), object.id).unwrap()
        }
    }
}

#[rstest]
#[case::two_heroes(2)]
#[case::three_heroes(3)]
#[case::four_heroes(4)]
fn removal_goes_at_the_eyed_hero_not_the_best_target(#[case] heroes: usize) {
    let (driver, ranked) = ranked_targets(zap_table(heroes, false), heroes, heroes + 1);
    let eyed = spotlight::focus(driver.state(), pid(heroes)).unwrap();
    assert_eq!(eyed, pid(0));
    assert!(!ranked.is_empty());
    for target in &ranked {
        let holder = holder(&driver, *target);
        assert!(
            holder == eyed || !spotlight::is_hero(driver.state(), holder),
            "{target:?} is held by hero {holder:?}, not the eyed one"
        );
    }
}

#[rstest]
#[case::two_heroes(2)]
#[case::three_heroes(3)]
#[case::four_heroes(4)]
fn removal_leaves_a_hero_that_cannot_be_targeted_with_that_reason(#[case] heroes: usize) {
    let (driver, ranked) = ranked_targets(zap_table(heroes, true), heroes, heroes + 1);
    let eyed = spotlight::focus(driver.state(), pid(heroes)).unwrap();
    assert_eq!(eyed, pid(0));
    let offered: Vec<PlayerId> = (1..heroes).map(pid).collect();
    let aim = spotlight_aim::target_aim(driver.state(), pid(heroes), &offered).unwrap();
    assert_eq!(aim.why, Some(AimWhy::Unreachable));
    assert!(!ranked.is_empty());
    assert!(
        ranked.iter().all(|target| holder(&driver, *target) != eyed),
        "the hexproof hero is not offered"
    );
    let first = holder(&driver, ranked[0]);
    assert_eq!(first, aim.hero, "the first of the others goes first");
}
