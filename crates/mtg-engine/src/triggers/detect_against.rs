//! Goal: an attack names who or what it goes against and a block names the blocker and the attacker, so `Ref::Against` and `Ref::Dealer` read them; every other event names nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rstest::rstest;

use super::detect;
use super::detect_events::{FOE, ME, board, id, target};
use crate::choose::Target;
use crate::combat::AttackTarget;
use crate::events::Event;

/// What one `event` of the board makes the watcher see: its cause's subjects, dealer and `against`.
fn seen(
    when: &str,
    event: impl Fn(&crate::state::GameState) -> Event,
) -> Vec<crate::abilities::Cause> {
    let pre = board(when);
    let event = event(&pre);
    detect(&pre, &[event], &pre)
        .into_iter()
        .map(|t| t.cause)
        .collect()
}

#[rstest]
#[case::a_player(false)]
#[case::a_planeswalker(true)]
fn an_attacker_goes_against_what_it_attacks(#[case] walker: bool) {
    let causes = seen(r#"{"Attacks":{"what":[]}}"#, |pre| {
        Event::AttackersDeclared {
            player: ME,
            attackers: vec![(
                id(pre, "Bear"),
                if walker {
                    AttackTarget::Planeswalker(id(pre, "Their Forest"))
                } else {
                    AttackTarget::Player(FOE)
                },
            )],
        }
    });
    assert_eq!(causes.len(), 1);
    let expected = if walker {
        let pre = board(r#"{"Attacks":{"what":[]}}"#);
        target(&pre, "Their Forest")
    } else {
        Target::Player(FOE)
    };
    assert_eq!(causes[0].against, Some(expected));
    assert_eq!(causes[0].source, None);
}

#[test]
fn a_block_names_the_blocker_as_dealer_and_the_attacker_as_against() {
    let causes = seen(r#"{"Blocks":{"what":[]}}"#, |pre| Event::BlockersDeclared {
        player: FOE,
        blocks: vec![(id(pre, "Ogre"), id(pre, "Bear"))],
    });
    let pre = board(r#"{"Blocks":{"what":[]}}"#);
    assert_eq!(causes.len(), 1);
    assert_eq!(causes[0].against, Some(target(&pre, "Bear")));
    assert_eq!(
        causes[0].source.map(|s| s.id),
        Some(id(&pre, "Ogre")),
        "the dealer of a block is the blocker"
    );
    assert_eq!(
        causes[0].subjects.len(),
        2,
        "the subjects stay as they were"
    );
}

#[rstest]
#[case::at_the_player_they_attack(
    r#"{"GroupAttacksAt":{"what":[],"at":"Opponents"}}"#,
    Some(Target::Player(FOE))
)]
#[case::a_group_at_no_one_in_particular(r#"{"GroupAttacks":{"what":[]}}"#, None)]
fn a_group_goes_against_the_one_player_it_attacks(
    #[case] when: &str,
    #[case] expected: Option<Target>,
) {
    let causes = seen(when, |pre| Event::AttackersDeclared {
        player: ME,
        attackers: vec![(id(pre, "Bear"), AttackTarget::Player(FOE))],
    });
    assert_eq!(causes.len(), 1);
    assert_eq!(causes[0].against, expected);
}

#[test]
fn an_event_that_is_no_attack_or_block_names_nothing_to_go_against() {
    let causes = seen(r#"{"LifeGained":{"who":"You"}}"#, |_| Event::LifeGained {
        player: ME,
        amount: 1,
    });
    assert!(causes.iter().all(|c| c.against.is_none()));
}
