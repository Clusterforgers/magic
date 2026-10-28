//! Goal: a target permanent that becomes a copy of a second target, and the card that becomes a copy, read with how long the copy lasts (until end of turn, until your next turn, for good) and written back as the card's text; a copy of a spell or of a card in a zone, and a duration no card says, stay with the table.
use super::*;
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::Ref;

fn copy_of(read: Ability) -> Effect {
    let effects = match read {
        Ability::Activated { effects, .. } | Ability::Spell { effects, .. } => effects,
        other => panic!("no effects: {other:?}"),
    };
    let [effect] = <[Effect; 1]>::try_from(effects).unwrap_or_else(|e| panic!("{e:?}"));
    effect
}

#[rstest]
#[case::a_target_until_your_next_turn(
    "{2}{U}: Target Shapeshifter becomes a copy of target creature until your next turn.",
    ClauseKind::Activated,
    (Ref::Target(0), Ref::Target(1), Duration::UntilYourNextTurn)
)]
#[case::a_target_until_end_of_turn(
    "Target creature you control becomes a copy of target creature until end of turn.",
    ClauseKind::Spell,
    (Ref::Target(0), Ref::Target(1), Duration::UntilEndOfTurn)
)]
#[case::another_target(
    "Target creature becomes a copy of another target creature until end of turn.",
    ClauseKind::Spell,
    (Ref::Target(0), Ref::Target(1), Duration::UntilEndOfTurn)
)]
#[case::a_target_for_good(
    "Target land becomes a copy of target land.",
    ClauseKind::Spell,
    (Ref::Target(0), Ref::Target(1), Duration::Forever)
)]
#[case::the_card_until_your_next_turn(
    "{1}{U}: This creature becomes a copy of target creature until your next turn.",
    ClauseKind::Activated,
    (Ref::This, Ref::Target(0), Duration::UntilYourNextTurn)
)]
fn a_permanent_becomes_a_copy_for_as_long_as_the_card_says(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] expected: (Ref, Ref, Duration),
) {
    let Effect::BecomeCopy {
        what,
        of,
        except,
        duration,
    } = copy_of(golden(text, kind, false))
    else {
        panic!("{text}: not a copy");
    };
    assert_eq!((what, of, duration), expected, "{text}");
    assert!(except.is_empty(), "{text}");
}

#[rstest]
#[case::a_spell("Target creature becomes a copy of target creature spell until end of turn.")]
#[case::a_card_in_a_graveyard(
    "Target creature becomes a copy of target creature card in a graveyard until end of turn."
)]
#[case::until_end_of_combat(
    "Target creature becomes a copy of target creature until end of combat."
)]
fn other_copies_stay_with_the_table(#[case] text: &str) {
    let read = clause(text, ClauseKind::Spell, false);
    assert!(
        read.is_none_or(|ability| !format!("{ability:?}").contains("BecomeCopy")),
        "{text}"
    );
}
