//! A permission to cast spells at instant speed for the turn: "You may cast spells this turn as though they had flash."

use super::super::{Duration, Effect, Ref, Selector};
use crate::modify::GameRule;

/// The sentence for the permission to cast any spell with flash this turn, and the one effect it covers.
pub(super) fn text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Rule {
            rule: GameRule::MayCast(permission),
            duration: Duration::UntilEndOfTurn,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    let plain = permission.who == Ref::You
        && permission.what == Selector::default()
        && permission.from.is_none()
        && permission.only.is_none()
        && !permission.free
        && !permission.play;
    (plain && permission.flash).then(|| {
        (
            "you may cast spells this turn as though they had flash".to_owned(),
            1,
        )
    })
}
