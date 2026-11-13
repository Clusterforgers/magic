//! Goal: a cast permission followed by the sentence of its riders ("If you cast a spell this way, that artifact enters
//! tapped", "... enters with a finality counter on it", "... gains haste until end of turn", "If a spell cast this way
//! would be put into your graveyard, exile it instead") reads as the permission with those riders and writes back as the
//! same text; a near-miss (a noun that is not the permission's, two counters, a duration other than the turn, another
//! zone for the exile, a rider with no permission) stays unread, and a permission with riders is never worded without them.
use super::*;
use crate::modify::{CastRider, GameRule};
use crate::value::CounterKind;

#[rstest]
#[case::enters_tapped(
    "Once during each of your turns, you may cast an artifact spell from your graveyard. If you cast a spell this way, that artifact enters tapped.",
    vec![CastRider::EntersTapped]
)]
#[case::exiled_instead(
    "Once during each of your turns, you may cast an instant or sorcery spell from your graveyard. If a spell cast this way would be put into your graveyard, exile it instead.",
    vec![CastRider::ExiledInstead]
)]
#[case::finality(
    "Once during each of your turns, you may cast a creature spell from your graveyard. If you cast a spell this way, that creature enters with a finality counter on it.",
    vec![CastRider::EntersWithCounter(CounterKind::Other("finality".into()))]
)]
#[case::haste_from_the_top(
    "You may cast creature spells with power 4 or greater from the top of your library. If you cast a creature spell this way, it gains haste until end of turn.",
    vec![CastRider::GainsThisTurn(crate::keyword::Keyword::Haste)]
)]
#[case::during_your_turn(
    "During your turn, you may cast creature spells with power or toughness 1 or less from your graveyard. If you cast a spell this way, that creature enters with a finality counter on it.",
    vec![CastRider::EntersWithCounter(CounterKind::Other("finality".into()))]
)]
#[case::finality_and_a_subtype(
    "Once during each of your turns, you may cast a creature spell from your graveyard. If you cast a spell this way, that creature enters with a finality counter on it and is a Vampire in addition to its other types.",
    vec![
        CastRider::EntersWithCounter(CounterKind::Other("finality".into())),
        CastRider::AlsoSubtype(mtg_core::card::Subtype("Vampire".into())),
    ]
)]
fn r401_a_permission_with_riders_reads_and_writes_back(
    #[case] text: &str,
    #[case] riders: Vec<CastRider>,
) {
    let read = golden(text, ClauseKind::Static, false);
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        ..
    } = read
    else {
        panic!("{text}: {read:?}");
    };
    assert_eq!(permission.riders, riders, "{text}");
}

#[rstest]
#[case::another_noun(
    "Once during each of your turns, you may cast an artifact spell from your graveyard. If you cast a spell this way, that creature enters tapped."
)]
#[case::two_counters(
    "Once during each of your turns, you may cast a creature spell from your graveyard. If you cast a spell this way, that creature enters with two finality counters on it."
)]
#[case::other_duration(
    "You may cast creature spells with power 4 or greater from the top of your library. If you cast a creature spell this way, it gains haste until your next turn."
)]
#[case::exiled_from_elsewhere(
    "Once during each of your turns, you may cast an instant or sorcery spell from your graveyard. If a spell cast this way would be put into a graveyard, exile it instead."
)]
#[case::no_permission("Draw a card. If you cast a spell this way, that artifact enters tapped.")]
fn r401_near_misses_stay_unread(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    let written = read.as_ref().map(render_variants).unwrap_or_default();
    assert!(
        !written.iter().any(|w| same_text(w, text)),
        "{text}: {read:?}"
    );
}

#[test]
fn r401_a_permission_with_riders_is_never_worded_without_them() {
    let text =
        "Once during each of your turns, you may cast an artifact spell from your graveyard.";
    let mut read = golden(text, ClauseKind::Static, false);
    if let Ability::Rule {
        rule: GameRule::MayCast(permission),
        ..
    } = &mut read
    {
        permission.riders = vec![CastRider::EntersTapped];
    }
    assert!(
        !render_variants(&read).iter().any(|w| same_text(w, text)),
        "{read:?}"
    );
}
