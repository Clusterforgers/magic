//! Goal: a copy of a spell triggers what waits for a player copying a spell (never a cast), and an attacker its defending player does not block triggers "attacks and isn't blocked" once that player has declared blockers, for the players and attackers each pattern names.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use rstest::rstest;

use super::detect_events::{FOE, ME, board, fired, fold, id};
use crate::choose::Choices;
use crate::combat::AttackTarget;
use crate::copies::SpellCopy;
use crate::events::Event;
use crate::state::GameState;

/// `pre` after "Spell", an instant in my hand, is cast.
fn cast(pre: &GameState) -> (GameState, Event) {
    let event = Event::SpellCast {
        player: ME,
        object: id(pre, "Spell"),
        choices: Choices::default(),
    };
    (fold(pre, std::slice::from_ref(&event)), event)
}

/// A copy of "Spell" on the stack, controlled by `controller`.
fn copied(state: &GameState, controller: PlayerId) -> Event {
    let original = id(state, "Spell");
    Event::SpellCopied(Box::new(SpellCopy {
        object: state.next_object(),
        of: state.object(original).unwrap().reference(),
        controller,
        definition: Arc::new(CardDefinition::new("Spell").with_type(CardType::Instant)),
        choices: Choices::default(),
    }))
}

#[rstest]
#[case::i_copy_a_spell(r#"{"SpellCopied":{"by":"You"}}"#, ME, 1)]
#[case::they_copy_not_me(r#"{"SpellCopied":{"by":"You"}}"#, FOE, 0)]
#[case::an_opponent_copies(r#"{"SpellCopied":{"by":"Opponents"}}"#, FOE, 1)]
#[case::anyone_copies(r#"{"SpellCopied":{}}"#, FOE, 1)]
#[case::an_instant_copied(
    r#"{"SpellCopied":{"what":[{"Type":"Instant"}],"by":"You"}}"#,
    ME,
    1
)]
#[case::not_a_sorcery(
    r#"{"SpellCopied":{"what":[{"Type":"Sorcery"}],"by":"You"}}"#,
    ME,
    0
)]
#[case::a_copy_is_not_cast(r#"{"SpellCast":{"by":"You"}}"#, ME, 0)]
#[case::cast_or_copy(
    r#"{"AnyOf":[{"SpellCast":{"by":"You"}},{"SpellCopied":{"by":"You"}}]}"#,
    ME,
    1
)]
fn copying_a_spell_triggers_for_the_player_who_copies_it(
    #[case] when: &str,
    #[case] controller: PlayerId,
    #[case] expected: usize,
) {
    let (pre, _) = cast(&board(when));
    assert_eq!(fired(&pre, &[copied(&pre, controller)]), expected);
}

#[rstest]
#[case::cast(r#"{"SpellCast":{"by":"You"}}"#)]
#[case::cast_or_copy(r#"{"AnyOf":[{"SpellCast":{"by":"You"}},{"SpellCopied":{"by":"You"}}]}"#)]
fn casting_a_spell_still_triggers_a_cast_or_copy(#[case] when: &str) {
    let pre = board(when);
    let (_, event) = cast(&pre);
    assert_eq!(fired(&pre, &[event]), 1);
}

/// `when` on the board after Bear and Watcher attacked my opponent.
fn attacked(when: &str) -> GameState {
    let pre = board(when);
    let attackers = ["Bear", "Watcher"]
        .iter()
        .map(|name| (id(&pre, name), AttackTarget::Player(FOE)))
        .collect();
    fold(
        &pre,
        &[Event::AttackersDeclared {
            player: ME,
            attackers,
        }],
    )
}

#[rstest]
#[case::itself_unblocked(r#"{"Unblocked":{"what":["This"]}}"#, FOE, &["Bear"], 1)]
#[case::itself_blocked(r#"{"Unblocked":{"what":["This"]}}"#, FOE, &["Watcher"], 0)]
#[case::each_of_mine_unblocked(r#"{"Unblocked":{"what":[{"ControlledBy":"You"}]}}"#, FOE, &[], 2)]
#[case::one_of_mine_blocked(
    r#"{"Unblocked":{"what":[{"ControlledBy":"You"}]}}"#,
    FOE,
    &["Bear"],
    1
)]
#[case::not_their_attackers(
    r#"{"Unblocked":{"what":[{"ControlledBy":"Opponents"}]}}"#,
    FOE,
    &[],
    0
)]
#[case::another_player_declared(r#"{"Unblocked":{"what":["This"]}}"#, ME, &[], 0)]
#[case::blocked_is_not_unblocked(r#"{"BecomesBlocked":{"what":["This"]}}"#, FOE, &[], 0)]
fn an_attacker_its_defender_does_not_block_is_unblocked(
    #[case] when: &str,
    #[case] declarer: PlayerId,
    #[case] blocked: &[&str],
    #[case] expected: usize,
) {
    let pre = attacked(when);
    let blockers = ["Ogre", "Wolf"];
    let blocks = blocked
        .iter()
        .zip(blockers)
        .map(|(attacker, blocker)| (id(&pre, blocker), id(&pre, attacker)))
        .collect();
    let declared = Event::BlockersDeclared {
        player: declarer,
        blocks,
    };
    assert_eq!(fired(&pre, &[declared]), expected);
}
