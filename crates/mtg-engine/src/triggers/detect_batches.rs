//! Goal: a subject that follows an attachment ("equipped creature") triggers for the host alone, as it was when it left; the events of one batch ("one or more") trigger once for each player they are about and count themselves; the nth card, spell or life gain of a turn triggers once for its player, together or apart, and starts again with each turn; the cause carries the amount of the event.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use rstest::rstest;

use super::detect;
use super::detect_events::{
    FOE, ME, board, damage, fired, fires, fold, id, in_combat_damage, pattern, target,
};
use crate::combat::{AttackTarget, DamageTarget};
use crate::counters::CountersChanged;
use crate::events::{DeathCause, Event, ZoneCause};
use crate::fixtures::{creature, state};
use crate::state::GameState;
use crate::zones::Zone;

/// Seat 0 controls a Blade (an Equipment with "when `when`, gain 1 life"), a Bear and a Cat; seat 1 controls an Ogre.
/// The Blade is attached to the Bear when `attached` is true.
fn armed(when: &str, attached: bool) -> GameState {
    let blade = Arc::new(
        CardDefinition::new("Blade")
            .with_type(CardType::Artifact)
            .with_subtype("Equipment")
            .with_rules(Ability::Triggered {
                when: pattern(when),
                intervening_if: None,
                limit: None,
                from: ZoneKind::Battlefield,
                targets: Vec::new(),
                effects: vec![Effect::GainLife {
                    who: Ref::You,
                    amount: Amount::Fixed(1),
                }],
            }),
    );
    let pre = state(
        vec![Vec::new(), Vec::new()],
        vec![
            vec![blade, creature("Bear", 2, 2), creature("Cat", 1, 1)],
            vec![creature("Ogre", 3, 4)],
        ],
    );
    if !attached {
        return pre;
    }
    let attach = Event::Attached {
        object: id(&pre, "Blade"),
        to: id(&pre, "Bear"),
    };
    fold(&pre, &[attach])
}

const EQUIPPED: &str = r#"{"Is":"Attached"}"#;

fn died(object: ObjectId) -> Event {
    Event::CreatureDied {
        object,
        cause: DeathCause::LethalDamage,
    }
}

#[rstest]
#[case::the_host_dies("dies", "Bear", true, 1)]
#[case::another_creature_dies("dies", "Cat", true, 0)]
#[case::an_opponents_creature_dies("dies", "Ogre", true, 0)]
#[case::no_host_dies("dies", "Bear", false, 0)]
#[case::the_host_leaves("leaves", "Bear", true, 1)]
#[case::the_host_attacks("attacks", "Bear", true, 1)]
#[case::another_creature_attacks("attacks", "Cat", true, 0)]
#[case::no_host_attacks("attacks", "Bear", false, 0)]
#[case::the_host_deals_combat_damage("damages", "Bear", true, 1)]
#[case::another_creature_deals_damage("damages", "Cat", true, 0)]
#[case::the_host_becomes_tapped("taps", "Bear", true, 1)]
#[case::another_creature_becomes_tapped("taps", "Cat", true, 0)]
fn a_subject_that_follows_the_attachment_triggers_for_the_host_alone(
    #[case] what: &str,
    #[case] subject: &str,
    #[case] attached: bool,
    #[case] expected: usize,
) {
    let on = format!(r#""what":[{EQUIPPED}]"#);
    let when = match what {
        "dies" => format!(r#"{{"ZoneChange":{{{on},"from":"Battlefield","to":"Graveyard"}}}}"#),
        "leaves" => format!(r#"{{"ZoneChange":{{{on},"from":"Battlefield"}}}}"#),
        "attacks" => format!(r#"{{"Attacks":{{{on}}}}}"#),
        "damages" => format!(
            r#"{{"DealsDamage":{{"source":[{EQUIPPED}],"to":"EachPlayer","combat":true}}}}"#
        ),
        _ => format!(r#"{{"Tapped":{{{on}}}}}"#),
    };
    let pre = armed(&when, attached);
    let object = id(&pre, subject);
    let event = match what {
        "dies" => died(object),
        "leaves" => Event::ZoneChanged {
            object,
            from: Zone::Battlefield,
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Destroyed,
        },
        "attacks" => Event::AttackersDeclared {
            player: ME,
            attackers: vec![(object, AttackTarget::Player(FOE))],
        },
        "damages" => damage(&pre, subject, DamageTarget::Player(FOE)),
        _ => Event::PermanentTapped { object },
    };
    let pre = if what == "damages" {
        in_combat_damage(&pre)
    } else {
        pre
    };
    assert_eq!(fired(&pre, &[event]), expected);
}

#[test]
fn a_host_that_dies_with_its_equipment_is_still_the_subject_as_it_was() {
    let when = format!(
        r#"{{"ZoneChange":{{"what":[{EQUIPPED}],"from":"Battlefield","to":"Graveyard"}}}}"#
    );
    let pre = armed(&when, true);
    let both = [
        died(id(&pre, "Bear")),
        Event::ZoneChanged {
            object: id(&pre, "Blade"),
            from: Zone::Battlefield,
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Destroyed,
        },
    ];
    let found = fires(&pre, &both);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].cause.subjects, [target(&pre, "Bear")]);
}

const DAMAGE_BATCH: &str = r#"{"Batch":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}}}"#;

#[rstest]
#[case::two_creatures_one_player_are_one_batch(&[("Bear", FOE), ("Watcher", FOE)], &[4])]
#[case::one_creature_is_enough(&[("Bear", FOE)], &[2])]
#[case::an_opponents_creature_is_not_mine(&[("Ogre", ME)], &[])]
#[case::each_player_hurt_is_a_batch_of_its_own(&[("Bear", FOE), ("Watcher", ME), ("Bear", ME)], &[2, 4])]
#[case::mine_among_theirs_counts_mine_only(&[("Ogre", FOE), ("Bear", FOE)], &[2])]
fn creatures_that_deal_damage_together_trigger_once_for_each_player_they_hurt(
    #[case] hits: &[(&str, PlayerId)],
    #[case] damage_dealt: &[u32],
) {
    let pre = in_combat_damage(&board(DAMAGE_BATCH));
    let events: Vec<Event> = hits
        .iter()
        .map(|(source, victim)| damage(&pre, source, DamageTarget::Player(*victim)))
        .collect();
    let counted: Vec<u32> = fires(&pre, &events)
        .iter()
        .map(|t| t.cause.amount)
        .collect();
    assert_eq!(
        counted, damage_dealt,
        "the amount of a batch of damage is the damage dealt in all"
    );
}

#[test]
fn damage_in_two_batches_triggers_a_batch_trigger_twice() {
    let pre = in_combat_damage(&board(DAMAGE_BATCH));
    let hit = |name| damage(&pre, name, DamageTarget::Player(FOE));
    assert_eq!(fired(&pre, &[hit("Bear"), hit("Watcher")]), 1);
    assert_eq!(
        fired(&pre, &[hit("Bear")]) + fired(&pre, &[hit("Watcher")]),
        2
    );
}

#[rstest]
#[case::three_milled(3, 1, 3)]
#[case::one_milled(1, 1, 1)]
#[case::nothing_milled(0, 0, 0)]
fn cards_put_into_your_graveyard_from_your_library_together_trigger_once_and_count_them(
    #[case] milled: usize,
    #[case] expected: usize,
    #[case] amount: u32,
) {
    let when = r#"{"Batch":{"ZoneChange":{"what":[{"OwnedBy":"You"}],"from":"Library","to":"Graveyard"}}}"#;
    let pre = board(when);
    let top: Vec<ObjectId> = pre
        .zone(Zone::Library(ME))
        .iter()
        .rev()
        .take(milled)
        .copied()
        .collect();
    let events: Vec<Event> = top
        .iter()
        .map(|object| Event::ZoneChanged {
            object: *object,
            from: Zone::Library(ME),
            to: Zone::Graveyard(ME),
            cause: ZoneCause::Milled,
        })
        .collect();
    let found = fires(&pre, &events);
    assert_eq!(found.len(), expected);
    if expected == 1 {
        assert_eq!(found[0].cause.amount, amount);
    }
}

const COUNTER_BATCH: &str = r#"{"Batch":{"CountersPut":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"counter":"+1/+1"}}}"#;

#[rstest]
#[case::two_creatures_are_one_batch(&[("Bear", 1), ("Watcher", 2)], 1)]
#[case::one_counter_is_enough(&[("Bear", 1)], 1)]
#[case::an_opponents_creature(&[("Ogre", 1)], 0)]
#[case::mine_among_theirs(&[("Ogre", 1), ("Bear", 1)], 1)]
fn counters_put_together_trigger_once(#[case] puts: &[(&str, i32)], #[case] expected: usize) {
    let pre = board(COUNTER_BATCH);
    let events: Vec<Event> = puts
        .iter()
        .map(|(name, delta)| {
            Event::CountersChanged(CountersChanged {
                on: target(&pre, name),
                counter: CounterKind::PlusOnePlusOne,
                delta: *delta,
            })
        })
        .collect();
    assert_eq!(fires(&pre, &events).len(), expected);
}

const SECOND_DRAW: &str = r#"{"Nth":{"event":{"Drew":{"who":"You"}},"nth":2}}"#;

fn draw(state: &GameState, player: PlayerId) -> Event {
    Event::CardDrawn {
        player,
        object: *state.zone(Zone::Library(player)).last().unwrap(),
    }
}

#[test]
fn the_second_card_of_a_turn_triggers_once_whether_drawn_together_or_apart() {
    let pre = board(SECOND_DRAW);
    assert_eq!(fired(&pre, &[draw(&pre, ME)]), 0);
    let first = fold(&pre, &[draw(&pre, ME)]);
    assert_eq!(fired(&first, &[draw(&first, ME)]), 1);
    let second = fold(&first, &[draw(&first, ME)]);
    assert_eq!(fired(&second, &[draw(&second, ME)]), 0, "the third");
    // Two cards in one batch: the second is the one.
    let two = [draw(&pre, ME), draw(&first, ME)];
    assert_eq!(fired(&pre, &two), 1);
}

#[test]
fn the_first_draw_of_a_turn_names_the_card_it_drew() {
    let first =
        r#"{"Nth":{"event":{"Drew":{"who":"You","what":[{"Named":"Library 0"}]}},"nth":1}}"#;
    let pre = board(first);
    assert_eq!(
        fired(&pre, &[draw(&pre, ME)]),
        1,
        "the card named is drawn first"
    );
    let after = fold(&pre, &[draw(&pre, ME)]);
    assert_eq!(fired(&after, &[draw(&after, ME)]), 0, "the second draw");
    let lands = r#"{"Nth":{"event":{"Drew":{"who":"You","what":[{"Named":"Spell"}]}},"nth":1}}"#;
    let pre = board(lands);
    assert_eq!(
        fired(&pre, &[draw(&pre, ME)]),
        0,
        "the first card is another"
    );
}

#[test]
fn a_new_turn_starts_the_count_again_and_each_player_counts_alone() {
    let pre = board(SECOND_DRAW);
    let one = fold(&pre, &[draw(&pre, FOE)]);
    let theirs = fold(&one, &[draw(&one, FOE)]);
    assert_eq!(fired(&theirs, &[draw(&theirs, ME)]), 0, "their draws");
    let first = fold(&theirs, &[draw(&theirs, ME)]);
    let turn = Event::TurnBegan {
        number: 2,
        active: FOE,
        extra: false,
    };
    let next = fold(&first, &[turn]);
    assert_eq!(fired(&next, &[draw(&next, ME)]), 0, "the first of the turn");
    let again = fold(&next, &[draw(&next, ME)]);
    assert_eq!(fired(&again, &[draw(&again, ME)]), 1);
}

#[rstest]
#[case::the_first_gain(1, 0, 1)]
#[case::not_the_second(1, 1, 0)]
#[case::the_second_gain(2, 1, 1)]
fn life_gained_is_counted_for_each_event(
    #[case] nth: u32,
    #[case] before: usize,
    #[case] expected: usize,
) {
    let when = format!(r#"{{"Nth":{{"event":{{"LifeGained":{{}}}},"nth":{nth}}}}}"#);
    let mut pre = board(&when);
    for _ in 0..before {
        let gain = Event::LifeGained {
            player: ME,
            amount: 1,
        };
        pre = fold(&pre, &[gain]);
    }
    let gain = Event::LifeGained {
        player: ME,
        amount: 3,
    };
    assert_eq!(fired(&pre, &[gain]), expected);
}

#[rstest]
#[case::damage(r#"{"DealsDamage":{"source":[]}}"#, "damage", 2)]
#[case::life_gained(r#"{"LifeGained":{}}"#, "gain", 3)]
#[case::life_lost(r#"{"LifeLost":{"who":"You"}}"#, "loss", 3)]
#[case::counters_put(r#"{"CountersPut":{"what":[]}}"#, "put", 2)]
#[case::counters_removed(r#"{"CountersRemoved":{"what":[]}}"#, "removed", 2)]
#[case::attackers(r#"{"GroupAttacks":{"what":[]}}"#, "attack", 2)]
fn the_cause_carries_the_amount_of_the_event(
    #[case] when: &str,
    #[case] what: &str,
    #[case] amount: u32,
) {
    let pre = board(when);
    let counters = |delta| {
        Event::CountersChanged(CountersChanged {
            on: target(&pre, "Bear"),
            counter: CounterKind::PlusOnePlusOne,
            delta,
        })
    };
    let event = match what {
        "damage" => damage(&pre, "Bear", DamageTarget::Player(FOE)),
        "gain" => Event::LifeGained {
            player: ME,
            amount: 3,
        },
        "loss" => Event::LifeLost {
            player: ME,
            amount: 3,
        },
        "put" => counters(2),
        "removed" => counters(-2),
        _ => Event::AttackersDeclared {
            player: ME,
            attackers: vec![
                (id(&pre, "Bear"), AttackTarget::Player(FOE)),
                (id(&pre, "Watcher"), AttackTarget::Player(FOE)),
            ],
        },
    };
    let found = detect(&pre, &[event], &pre);
    assert_eq!(found.first().map(|t| t.cause.amount), Some(amount));
}
