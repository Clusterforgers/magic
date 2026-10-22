//! Goal: the first (or nth) time in a turn that an object attacks, becomes a target or is dealt damage triggers once for that object alone, whoever else attacked, was targeted or was hurt before; a target counts by what its controller or anyone controls, a spell and an ability alike; the count starts again with each turn and for a permanent that changed zones, counts the events of one batch in order and is the same at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Bounds, Ref, TargetSpec, Targetable, ZoneKind};
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::detect_events::{FOE, ME, fired, fold, id, pattern, target};
use crate::abilities::{AbilitySource, PutOnStack};
use crate::choose::{Choices, Target};
use crate::combat::{AttackTarget, DamageTarget};
use crate::events::{Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::snapshot::snapshot;
use crate::state::GameState;
use crate::zones::Zone;

fn creature_with(name: &str, rules: Vec<Ability>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1);
    for rule in rules {
        card = card.with_rules(rule);
    }
    Arc::new(card)
}

/// A creature that gains its controller 1 life whenever `when`.
fn watcher(when: &str) -> Arc<CardDefinition> {
    creature_with(
        "Watcher",
        vec![Ability::Triggered {
            when: pattern(when),
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::GainLife {
                who: Ref::You,
                amount: Amount::Fixed(1),
            }],
        }],
    )
}

/// A creature with "{T}: 1 damage to any target".
fn pinger(name: &str) -> Arc<CardDefinition> {
    creature_with(
        name,
        vec![Ability::Activated {
            cost: Cost(Vec::new()),
            timing: Timing::Instant,
            condition: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: vec![TargetSpec {
                what: Targetable::AnyTarget,
                bounds: Bounds::default(),
                divide: None,
            }],
            effects: vec![Effect::DealDamage {
                source: None,
                to: Ref::Target(0),
                amount: Amount::Fixed(1),
            }],
        }],
    )
}

/// `seats` players: every seat holds three Spells ("Spell 1.2" is seat 1's third) and controls a Pinger ("Pinger 1");
/// seat 0 also controls the Watcher of `when` and a Bear, the others a Guest.
fn table(seats: usize, when: &str) -> GameState {
    let spells = |seat: usize| {
        (0..3)
            .map(|n| {
                let name = format!("Spell {seat}.{n}");
                Arc::new(CardDefinition::new(&name).with_type(CardType::Instant))
            })
            .collect::<Vec<_>>()
    };
    let field = |seat: usize| {
        let mut cards = vec![pinger(&format!("Pinger {seat}"))];
        if seat == 0 {
            cards.extend([watcher(when), creature("Bear", 2, 2)]);
        } else {
            cards.push(creature(&format!("Guest {seat}"), 2, 2));
        }
        cards
    };
    state(
        (0..seats).map(spells).collect(),
        (0..seats).map(field).collect(),
    )
}

/// What targets: a spell or an ability, controlled by a seat.
#[derive(Debug, Clone, Copy)]
enum By {
    Spell(usize),
    Ability(usize),
}

/// The events of `hits` (what targets, and the name of the permanent it targets), each spell the seat's next one in hand.
fn events(pre: &GameState, hits: &[(By, &str)]) -> Vec<Event> {
    let mut cast = [0usize; 4];
    let mut abilities = 0;
    hits.iter()
        .map(|(by, at)| {
            let choices = Choices {
                targets: vec![vec![target(pre, at)]],
                ..Choices::default()
            };
            match *by {
                By::Spell(seat) => {
                    let name = format!("Spell {seat}.{}", cast[seat]);
                    cast[seat] += 1;
                    Event::SpellCast {
                        player: PlayerId(u8::try_from(seat).unwrap().into()),
                        object: id(pre, &name),
                        choices,
                    }
                }
                By::Ability(seat) => {
                    let pinger = format!("Pinger {seat}");
                    abilities += 1;
                    Event::AbilityPutOnStack(Box::new(PutOnStack {
                        origin: AbilitySource::default(),
                        object: ObjectId(pre.next_object().0 + abilities),
                        source: pre.object(id(pre, &pinger)).unwrap().reference(),
                        snapshot: snapshot(pre, id(pre, &pinger)).unwrap(),
                        ability: 0,
                        controller: PlayerId(u8::try_from(seat).unwrap().into()),
                        choices,
                        cause: None,
                        delayed: None,
                    }))
                }
            }
        })
        .collect()
}

/// How many abilities the last of `hits` triggers, once the ones before it have happened.
fn last_hit_fires(pre: &GameState, hits: &[(By, &str)]) -> usize {
    let all = events(pre, hits);
    let (before, last) = all.split_at(all.len() - 1);
    fired(&fold(pre, before), last)
}

const MINE_ONCE: &str = r#"{"Nth":{"event":{"BecomesTarget":{"what":["This"],"by":[{"ControlledBy":"You"}]}},"nth":1}}"#;
const MINE_TWICE: &str = r#"{"Nth":{"event":{"BecomesTarget":{"what":["This"],"by":[{"ControlledBy":"You"}]}},"nth":2}}"#;
const ANYONES_ONCE: &str = r#"{"Nth":{"event":{"BecomesTarget":{"what":["This"]}},"nth":1}}"#;
const ANYONES_TWICE: &str = r#"{"Nth":{"event":{"BecomesTarget":{"what":["This"]}},"nth":2}}"#;
const ANY_CREATURE_OF_MINE: &str = r#"{"Nth":{"event":{"BecomesTarget":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"by":[{"ControlledBy":"You"}]}},"nth":1}}"#;

#[rstest]
#[case::the_first_spell_of_mine(MINE_ONCE, &[], (By::Spell(0), "Watcher"), 1)]
#[case::the_first_ability_of_mine(MINE_ONCE, &[], (By::Ability(0), "Watcher"), 1)]
#[case::not_the_second_spell(MINE_ONCE, &[(By::Spell(0), "Watcher")], (By::Spell(0), "Watcher"), 0)]
#[case::not_the_second_whatever_it_is(MINE_ONCE, &[(By::Spell(0), "Watcher")], (By::Ability(0), "Watcher"), 0)]
#[case::not_a_spell_of_an_opponent(MINE_ONCE, &[], (By::Spell(1), "Watcher"), 0)]
#[case::not_an_ability_of_an_opponent(MINE_ONCE, &[], (By::Ability(1), "Watcher"), 0)]
#[case::an_opponents_does_not_use_up_mine(MINE_ONCE, &[(By::Spell(1), "Watcher")], (By::Spell(0), "Watcher"), 1)]
#[case::another_object_does_not_use_it_up(MINE_ONCE, &[(By::Spell(0), "Bear")], (By::Spell(0), "Watcher"), 1)]
#[case::another_object_does_not_trigger_it(MINE_ONCE, &[], (By::Spell(0), "Bear"), 0)]
#[case::the_second_time(MINE_TWICE, &[(By::Spell(0), "Watcher")], (By::Ability(0), "Watcher"), 1)]
#[case::not_the_first_of_two(MINE_TWICE, &[], (By::Spell(0), "Watcher"), 0)]
#[case::not_the_third(MINE_TWICE, &[(By::Spell(0), "Watcher"), (By::Spell(0), "Watcher")], (By::Ability(0), "Watcher"), 0)]
#[case::the_second_does_not_count_an_opponents(MINE_TWICE, &[(By::Spell(1), "Watcher")], (By::Spell(0), "Watcher"), 0)]
#[case::anyone_first(ANYONES_ONCE, &[], (By::Spell(1), "Watcher"), 1)]
#[case::not_the_first_of_anyone_when_an_opponent_was_first(ANYONES_ONCE, &[(By::Spell(1), "Watcher")], (By::Spell(0), "Watcher"), 0)]
#[case::not_the_first_of_anyone_when_i_was_first(ANYONES_ONCE, &[(By::Ability(0), "Watcher")], (By::Spell(1), "Watcher"), 0)]
#[case::anyones_second_is_mine_after_an_opponent(ANYONES_TWICE, &[(By::Spell(1), "Watcher")], (By::Spell(0), "Watcher"), 1)]
#[case::anyones_second_is_an_opponents_after_mine(ANYONES_TWICE, &[(By::Spell(0), "Watcher")], (By::Ability(1), "Watcher"), 1)]
#[case::each_creature_of_mine_alone(ANY_CREATURE_OF_MINE, &[(By::Spell(0), "Watcher")], (By::Spell(0), "Bear"), 1)]
#[case::a_creature_of_theirs_is_not_mine(ANY_CREATURE_OF_MINE, &[], (By::Spell(0), "Guest 1"), 0)]
fn becoming_a_target_is_counted_for_the_object_by_what_i_or_anyone_controls(
    #[case] when: &str,
    #[case] before: &[(By, &str)],
    #[case] hit: (By, &str),
    #[case] expected: usize,
    #[values(2, 3, 4)] seats: usize,
) {
    let mut hits = before.to_vec();
    hits.push(hit);
    // The creature of seat 1 named in a case exists at every size.
    assert_eq!(last_hit_fires(&table(seats, when), &hits), expected);
}

#[rstest]
#[case::mine_first_the_opponents_then(MINE_ONCE, true)]
#[case::anyones_first_the_opponents_then(ANYONES_ONCE, false)]
fn every_seat_counts_by_what_it_controls_alone(
    #[case] when: &str,
    #[case] mine_is_first_after_them: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    let pre = table(seats, when);
    for seat in 1..seats {
        let hits = [(By::Spell(seat), "Watcher"), (By::Spell(0), "Watcher")];
        let expected = usize::from(mine_is_first_after_them);
        assert_eq!(last_hit_fires(&pre, &hits), expected, "seat {seat}");
        // The second seat to target it is not the first for anyone, and is the first of its own.
        for other in (1..seats).filter(|other| *other != seat) {
            let hits = [(By::Spell(seat), "Watcher"), (By::Spell(other), "Watcher")];
            assert_eq!(last_hit_fires(&pre, &hits), 0, "seats {seat} then {other}");
        }
    }
}

#[rstest]
#[case::the_first_of_two_of_mine(MINE_ONCE, &[(By::Spell(0), "Watcher"), (By::Ability(0), "Watcher")], 1)]
#[case::the_second_of_two_of_mine(MINE_TWICE, &[(By::Spell(0), "Watcher"), (By::Ability(0), "Watcher")], 1)]
#[case::the_first_of_anyones(ANYONES_ONCE, &[(By::Spell(1), "Watcher"), (By::Ability(0), "Watcher")], 1)]
#[case::the_second_of_anyones(ANYONES_TWICE, &[(By::Spell(1), "Watcher"), (By::Ability(0), "Watcher")], 1)]
#[case::mine_when_an_opponent_came_first(MINE_ONCE, &[(By::Spell(1), "Watcher"), (By::Ability(0), "Watcher")], 1)]
#[case::not_the_second_of_mine_after_an_opponent(MINE_TWICE, &[(By::Spell(1), "Watcher"), (By::Ability(0), "Watcher")], 0)]
#[case::only_the_second_of_three(MINE_TWICE, &[(By::Spell(0), "Watcher"), (By::Ability(0), "Watcher"), (By::Spell(0), "Watcher")], 1)]
fn the_events_of_one_batch_are_counted_in_order(
    #[case] when: &str,
    #[case] hits: &[(By, &str)],
    #[case] expected: usize,
) {
    let pre = table(2, when);
    assert_eq!(fired(&pre, &events(&pre, hits)), expected);
}

#[test]
fn a_spell_that_names_a_permanent_twice_targets_it_once() {
    let pre = table(2, MINE_ONCE);
    let watcher = target(&pre, "Watcher");
    let cast = |name: &str, targets: Vec<Vec<Target>>| Event::SpellCast {
        player: ME,
        object: id(&pre, name),
        choices: Choices {
            targets,
            ..Choices::default()
        },
    };
    let twice = cast("Spell 0.0", vec![vec![watcher], vec![watcher]]);
    assert_eq!(fired(&pre, std::slice::from_ref(&twice)), 1);
    // It was the first time, not the second: the next spell is not.
    let after = fold(&pre, &[twice]);
    assert_eq!(
        fired(
            &after,
            &[cast("Spell 0.1", vec![vec![watcher], vec![watcher]])]
        ),
        0
    );
}

#[test]
fn a_new_turn_starts_the_count_again() {
    let pre = table(2, MINE_ONCE);
    let hits = events(
        &pre,
        &[(By::Spell(0), "Watcher"), (By::Spell(0), "Watcher")],
    );
    let first = fold(&pre, &hits[..1]);
    assert!(!first.tallies.objects.is_empty());
    assert_eq!(fired(&first, &hits[1..]), 0, "the second time");
    let turn = Event::TurnBegan {
        number: 2,
        active: FOE,
    };
    let next = fold(&first, &[turn]);
    assert!(next.tallies.objects.is_empty(), "the count is cleared");
    assert_eq!(fired(&next, &hits[1..]), 1, "the first of the new turn");
}

#[test]
fn a_permanent_that_changed_zones_is_a_new_object_and_counts_from_the_start() {
    let pre = table(2, MINE_ONCE);
    let object = id(&pre, "Watcher");
    let hits = events(
        &pre,
        &[(By::Spell(0), "Watcher"), (By::Spell(0), "Watcher")],
    );
    let first = fold(&pre, &hits[..1]);
    assert_eq!(fired(&first, &hits[1..]), 0, "the old object, the second");
    // The Watcher goes to hand and comes back.
    let leave = Event::ZoneChanged {
        object,
        from: Zone::Battlefield,
        to: Zone::Hand(ME),
        cause: ZoneCause::Moved,
    };
    let back = Event::ZoneChanged {
        object,
        from: Zone::Hand(ME),
        to: Zone::Battlefield,
        cause: ZoneCause::Moved,
    };
    let returned = fold(&first, &[leave, back]);
    // A spell at the object that came back is its first.
    let new = Event::SpellCast {
        player: ME,
        object: id(&pre, "Spell 0.1"),
        choices: Choices {
            targets: vec![vec![target(&returned, "Watcher")]],
            ..Choices::default()
        },
    };
    assert_eq!(fired(&returned, &[new]), 1);
}

const FIRST_ATTACK: &str =
    r#"{"Nth":{"event":{"Attacks":{"what":[{"Type":"Creature"}]}},"nth":1}}"#;
const SECOND_ATTACK: &str =
    r#"{"Nth":{"event":{"Attacks":{"what":[{"Type":"Creature"}]}},"nth":2}}"#;
const WATCHERS_FIRST_ATTACK: &str = r#"{"Nth":{"event":{"Attacks":{"what":["This"]}},"nth":1}}"#;

fn attack(pre: &GameState, names: &[&str]) -> Event {
    Event::AttackersDeclared {
        player: ME,
        attackers: names
            .iter()
            .map(|name| (id(pre, name), AttackTarget::Player(FOE)))
            .collect(),
    }
}

#[rstest]
#[case::one_attacker(FIRST_ATTACK, &["Bear"], 1)]
#[case::each_attacker_is_the_first_for_itself(FIRST_ATTACK, &["Bear", "Watcher"], 2)]
#[case::only_the_one_named(WATCHERS_FIRST_ATTACK, &["Bear"], 0)]
#[case::the_one_named(WATCHERS_FIRST_ATTACK, &["Bear", "Watcher"], 1)]
#[case::not_the_second(SECOND_ATTACK, &["Bear"], 0)]
fn an_attack_is_the_first_of_its_creature(
    #[case] when: &str,
    #[case] attackers: &[&str],
    #[case] expected: usize,
) {
    let pre = table(2, when);
    assert_eq!(fired(&pre, &[attack(&pre, attackers)]), expected);
}

#[rstest]
#[case::the_first_attack_is_not_the_second(FIRST_ATTACK, 0)]
#[case::the_second_attack_of_the_turn(SECOND_ATTACK, 1)]
fn a_second_combat_of_the_turn_is_the_second_attack_of_a_creature(
    #[case] when: &str,
    #[case] expected: usize,
) {
    let pre = table(2, when);
    let first = fold(&pre, &[attack(&pre, &["Bear"]), Event::CombatEnded]);
    // Only the Bear attacked before: the Watcher's first attack is its first.
    assert_eq!(fired(&first, &[attack(&first, &["Bear"])]), expected);
    let fresh = fired(&first, &[attack(&first, &["Watcher"])]);
    assert_eq!(fresh, usize::from(when == FIRST_ATTACK));
}

#[test]
fn the_attacks_of_a_turn_start_again_with_the_next_turn() {
    let pre = table(2, FIRST_ATTACK);
    let attacked = fold(&pre, &[attack(&pre, &["Bear"]), Event::CombatEnded]);
    assert_eq!(fired(&attacked, &[attack(&attacked, &["Bear"])]), 0);
    let turn = Event::TurnBegan {
        number: 2,
        active: FOE,
    };
    let next = fold(&attacked, &[turn]);
    assert_eq!(fired(&next, &[attack(&next, &["Bear"])]), 1);
}

const DEALT_ONCE: &str = r#"{"Nth":{"event":{"DealsDamage":{"source":[],"to":"This"}},"nth":1}}"#;
const DEALT_TWICE: &str = r#"{"Nth":{"event":{"DealsDamage":{"source":[],"to":"This"}},"nth":2}}"#;
const MY_CREATURES_DEALT_ONCE: &str = r#"{"Nth":{"event":{"DealsDamage":{"source":[],"to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},"nth":1}}"#;

fn hurt(pre: &GameState, source: &str, to: DamageTarget, amount: u32) -> Event {
    Event::DamageDealt {
        source: id(pre, source),
        target: to,
        amount,
        deathtouch: false,
    }
}

fn to(pre: &GameState, name: &str) -> DamageTarget {
    DamageTarget::Object(id(pre, name))
}

#[rstest]
#[case::the_first_damage(DEALT_ONCE, &[("Guest 1", "Watcher", 1)], 1)]
#[case::from_any_source(DEALT_ONCE, &[("Pinger 0", "Watcher", 1)], 1)]
#[case::to_another_creature(DEALT_ONCE, &[("Guest 1", "Bear", 1)], 0)]
#[case::to_a_player(DEALT_ONCE, &[("Guest 1", "Player", 1)], 0)]
#[case::damage_of_nothing(DEALT_ONCE, &[("Guest 1", "Watcher", 0)], 0)]
#[case::nothing_does_not_use_up_the_first(DEALT_ONCE, &[("Guest 1", "Watcher", 0), ("Guest 1", "Watcher", 1)], 1)]
#[case::only_the_first_of_two(DEALT_ONCE, &[("Guest 1", "Watcher", 1), ("Pinger 1", "Watcher", 1)], 1)]
#[case::the_second(DEALT_TWICE, &[("Guest 1", "Watcher", 1), ("Pinger 1", "Watcher", 1)], 1)]
#[case::only_the_second_of_three(DEALT_TWICE, &[("Guest 1", "Watcher", 1), ("Pinger 1", "Watcher", 1), ("Guest 1", "Watcher", 1)], 1)]
#[case::each_creature_of_mine_is_the_first_for_itself(MY_CREATURES_DEALT_ONCE, &[("Guest 1", "Bear", 1), ("Guest 1", "Watcher", 1)], 2)]
#[case::not_a_creature_of_theirs(MY_CREATURES_DEALT_ONCE, &[("Guest 1", "Guest 1", 1)], 0)]
fn damage_is_counted_for_the_object_that_is_dealt_it(
    #[case] when: &str,
    #[case] hits: &[(&str, &str, u32)],
    #[case] expected: usize,
    #[values(2, 4)] seats: usize,
) {
    let pre = table(seats, when);
    // All of the damage is one batch, in order.
    let batch: Vec<Event> = hits
        .iter()
        .map(|(source, at, amount)| {
            let at = if *at == "Player" {
                DamageTarget::Player(ME)
            } else {
                to(&pre, at)
            };
            hurt(&pre, source, at, *amount)
        })
        .collect();
    assert_eq!(fired(&pre, &batch), expected, "in one batch");
    // One event at a time triggers as often in all.
    let mut seen = pre.clone();
    let mut total = 0;
    for event in &batch {
        total += fired(&seen, std::slice::from_ref(event));
        seen = fold(&seen, std::slice::from_ref(event));
    }
    assert_eq!(total, expected, "one event at a time");
}

#[test]
fn damage_to_a_permanent_that_dies_and_returns_is_a_new_object_dealt_its_first() {
    let pre = table(2, DEALT_ONCE);
    let object = id(&pre, "Watcher");
    let first = fold(&pre, &[hurt(&pre, "Guest 1", to(&pre, "Watcher"), 1)]);
    let leave = Event::ZoneChanged {
        object,
        from: Zone::Battlefield,
        to: Zone::Hand(ME),
        cause: ZoneCause::Moved,
    };
    let back = Event::ZoneChanged {
        object,
        from: Zone::Hand(ME),
        to: Zone::Battlefield,
        cause: ZoneCause::Moved,
    };
    let returned = fold(&first, &[leave, back]);
    let again = hurt(&returned, "Guest 1", to(&returned, "Watcher"), 1);
    assert_eq!(fired(&returned, &[again]), 1);
}

#[test]
fn damage_in_a_new_turn_is_the_first_again() {
    let pre = table(2, DEALT_ONCE);
    let first = fold(&pre, &[hurt(&pre, "Guest 1", to(&pre, "Watcher"), 1)]);
    let again = hurt(&first, "Guest 1", to(&first, "Watcher"), 1);
    assert_eq!(fired(&first, std::slice::from_ref(&again)), 0);
    let turn = Event::TurnBegan {
        number: 2,
        active: FOE,
    };
    let next = fold(&first, &[turn]);
    assert_eq!(fired(&next, &[again]), 1);
}
