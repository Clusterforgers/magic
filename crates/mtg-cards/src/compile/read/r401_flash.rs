//! A card's permission to cast itself as though it had flash under a condition on the spell: "You may cast this spell as
//! though it had flash if it targets a permanent you control.", "... if X is 3 or less." The condition is the
//! permission's `when`, judged as the announcement ends (`Condition::R401OnCast`); "... if it's cast using teamwork."
//! is the optional additional cost of teamwork announced as paid (`Condition::AdditionalCostPaid`).

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::ability::Ability;
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition};

/// The permission of the card to cast itself as though it had flash under the condition on the spell.
fn r401_permission(condition: Condition) -> CastPermission {
    CastPermission {
        what: Selector(vec![Pred::This]),
        flash: true,
        when: Some(Condition::R401OnCast(Box::new(condition))),
        ..CastPermission::default()
    }
}

/// The permission as a rule that works from its owner's hand.
fn r401_rule(condition: Condition) -> Ability {
    Ability::Rule {
        rule: GameRule::MayCast(r401_permission(condition)),
        condition: None,
        from: ZoneKind::Hand,
    }
}

/// "You may cast this spell as though it had flash if it targets a permanent you control." or "... if X is 3 or less."
pub(super) fn r401_flash(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("you may cast")?;
    (r.source() || r.words("this spell")).then_some(())?;
    r.need("as though it had flash if")?;
    if r.words("it's cast using teamwork") {
        return Some(r401_rule(Condition::AdditionalCostPaid));
    }
    if r.words("it targets") {
        (r.word("a") || r.word("an")).then_some(())?;
        let what = phrase(r, Form::Subject)?;
        return Some(r401_rule(Condition::R401Targets(what)));
    }
    r.word("x").then_some(())?;
    r.need("is")?;
    let most = r.number()?;
    r.need("or less")?;
    Some(r401_rule(Condition::Compare {
        left: Amount::X,
        op: Cmp::Le,
        right: Amount::Fixed(i32::try_from(most).ok()?),
    }))
}

/// The wordings of a rule `r401_flash` reads; empty for another.
pub(super) fn r401_flash_texts(
    rule: &GameRule,
    condition: Option<&Condition>,
    from: ZoneKind,
) -> Vec<String> {
    let (GameRule::MayCast(permission), None, ZoneKind::Hand) = (rule, condition, from) else {
        return Vec::new();
    };
    let Some(Condition::R401OnCast(inner)) = &permission.when else {
        return Vec::new();
    };
    if *permission != r401_permission((**inner).clone()) {
        return Vec::new();
    }
    let tail = match inner.as_ref() {
        Condition::R401Targets(what) => match phrase_text(what, Form::Subject) {
            Some(named) => format!("it targets {} {named}", article(&named)),
            None => return Vec::new(),
        },
        Condition::Compare {
            left: Amount::X,
            op: Cmp::Le,
            right: Amount::Fixed(most),
        } => format!("X is {most} or less"),
        Condition::AdditionalCostPaid => "it's cast using teamwork".to_owned(),
        _ => return Vec::new(),
    };
    ["this spell", "~"]
        .iter()
        .map(|spell| format!("You may cast {spell} as though it had flash if {tail}"))
        .collect()
}
