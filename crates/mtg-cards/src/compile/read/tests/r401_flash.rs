//! Goal: a card's permission to cast itself as though it had flash if a condition on the spell holds ("if it targets a
//! permanent you control", "if it targets a commander", "if X is 3 or less") reads as a flash permission whose condition
//! is judged as the spell is announced and writes back as the same text; a near-miss (another comparison, a condition of
//! the board, another caster) does not read as one.
use super::*;
use crate::modify::GameRule;
use crate::value::Condition;

#[rstest]
#[case::targets_yours(
    "You may cast this spell as though it had flash if it targets a permanent you control."
)]
#[case::targets_a_commander(
    "You may cast this spell as though it had flash if it targets a commander."
)]
#[case::x_at_most("You may cast this spell as though it had flash if X is 3 or less.")]
fn r401_flash_reads_and_writes_back(#[case] text: &str) {
    let read = golden(text, ClauseKind::Static, true);
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition: None,
        ..
    } = read
    else {
        panic!("{text}: {read:?}");
    };
    assert!(permission.flash, "{text}");
    assert!(
        matches!(permission.when, Some(Condition::R401OnCast(_))),
        "{text}"
    );
}

#[rstest]
#[case::x_at_least("You may cast this spell as though it had flash if X is 3 or more.")]
#[case::targets_with_another_caster(
    "Your opponents may cast this spell as though it had flash if it targets a permanent you control."
)]
#[case::without_flash("You may cast this spell if it targets a permanent you control.")]
fn r401_flash_near_misses_are_not_judged_on_the_spell(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, true);
    let on_cast = matches!(
        &read,
        Some(Ability::Rule {
            rule: GameRule::MayCast(permission),
            ..
        }) if matches!(permission.when, Some(Condition::R401OnCast(_)))
    );
    assert!(!on_cast, "{text}: {read:?}");
}
