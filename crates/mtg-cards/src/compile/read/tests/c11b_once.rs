//! Goal: a permission to cast a kind of spell from the graveyard once during each of its controller's turns reads for
//! each kind and writes back as the same text; a near-miss (another zone, another frequency, no kind, a player other than
//! you) stays unread.
use super::*;
use crate::modify::GameRule;
use crate::refs::ZoneKind;

#[rstest]
#[case::creature(
    "Once during each of your turns, you may cast a creature spell from your graveyard."
)]
#[case::zombie_creature(
    "Once during each of your turns, you may cast a Zombie creature spell from your graveyard."
)]
#[case::artifact(
    "Once during each of your turns, you may cast an artifact spell from your graveyard."
)]
#[case::instant_or_sorcery(
    "Once during each of your turns, you may cast an instant or sorcery spell from your graveyard."
)]
fn c11b_once_reads_and_writes_back(#[case] text: &str) {
    let read = golden(text, ClauseKind::Static, false);
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        from: ZoneKind::Battlefield,
        ..
    } = read
    else {
        panic!("{text}: {read:?}");
    };
    assert!(permission.once, "{text}");
    assert_eq!(permission.from, Some(ZoneKind::Graveyard), "{text}");
}

#[rstest]
#[case::exile("Once during each of your turns, you may cast a creature spell from exile.")]
#[case::each_turn("Once each turn, you may cast a creature spell from your graveyard.")]
#[case::any_spell("Once during each of your turns, you may cast a spell from your graveyard.")]
#[case::opponent(
    "Once during each of your turns, an opponent may cast a creature spell from your graveyard."
)]
fn c11b_once_near_misses_stay_unread(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    let written = read.as_ref().map(render_variants).unwrap_or_default();
    assert!(
        !written.iter().any(|w| same_text(w, text)),
        "{text}: {read:?}"
    );
}
