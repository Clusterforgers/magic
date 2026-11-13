//! A card's permission to cast itself as though it had flash for an additional cost that is not mana: "You may cast this
//! spell as though it had flash if you behold a Dragon as an additional cost to cast it." and "... by tapping three
//! untapped creatures you control with flying in addition to paying its other costs." Both are a `MayCast` rule of the
//! card from its owner's hand whose additional cost is the one part.

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Selector, ZoneKind};
use crate::value::{Amount, Condition};

/// The permission: the card itself, cast by its owner from a hand as though it had flash for the one extra part.
fn r4701_permission(part: CostItem) -> CastPermission {
    CastPermission {
        what: Selector(vec![Pred::This]),
        flash: true,
        additional: Some(Cost(vec![part])),
        ..CastPermission::default()
    }
}

/// "You may cast ~ as though it had flash if you behold a Dragon as an additional cost to cast it" and "... by tapping
/// three untapped creatures you control with flying in addition to paying its other costs".
pub(super) fn r4701_flash(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("you may cast")?;
    r.source().then_some(())?;
    r.need("as though it had flash")?;
    let part = if r.words("by tapping") {
        let tap = super::costs::r4701_tap(r)?;
        r.need("in addition to paying its other costs")?;
        tap
    } else {
        r.need("if you behold")?;
        (r.word("a") || r.word("an")).then_some(())?;
        let filter = phrase(r, Form::Subject)?;
        r.need("as an additional cost to cast it")?;
        CostItem::Behold {
            filter,
            count: Amount::one(),
            exile: false,
        }
    };
    Some(Ability::Rule {
        rule: GameRule::MayCast(r4701_permission(part)),
        condition: None,
        from: ZoneKind::Hand,
    })
}

/// The wording of a rule `r4701_flash` reads; empty for another rule.
pub(super) fn r4701_flash_texts(rule: &GameRule, condition: Option<&Condition>) -> Vec<String> {
    let GameRule::MayCast(permission) = rule else {
        return Vec::new();
    };
    let Some(Cost(items)) = &permission.additional else {
        return Vec::new();
    };
    if let [part @ CostItem::TapPermanents { filter, count }] = items.as_slice() {
        return r4701_tapping_texts(permission, condition, part, filter, count);
    }
    let [
        part @ CostItem::Behold {
            filter,
            count,
            exile: false,
        },
    ] = items.as_slice()
    else {
        return Vec::new();
    };
    if condition.is_some() || *permission != r4701_permission(part.clone()) || !count.is_one() {
        return Vec::new();
    }
    let Some(what) = phrase_text(filter, Form::Subject) else {
        return Vec::new();
    };
    vec![format!(
        "You may cast ~ as though it had flash if you behold {} {what} as an additional cost to cast it",
        article(&what)
    )]
}

/// "... by tapping three untapped creatures you control with flying in addition to paying its other costs": the keyword
/// after "you control", where the cost writer puts it before.
fn r4701_tapping_texts(
    permission: &CastPermission,
    condition: Option<&Condition>,
    part: &CostItem,
    filter: &Selector,
    count: &Amount,
) -> Vec<String> {
    if condition.is_some() || *permission != r4701_permission(part.clone()) {
        return Vec::new();
    }
    let [kinds @ .., Pred::HasKeyword(keyword)] = filter.0.as_slice() else {
        return Vec::new();
    };
    let plain = CostItem::TapPermanents {
        filter: Selector(kinds.to_vec()),
        count: count.clone(),
    };
    let Some(said) = super::costs::text(&Cost(vec![plain])) else {
        return Vec::new();
    };
    let (Some(rest), Some(word)) = (
        said.strip_prefix("Tap "),
        super::selectors::keyword_text(keyword),
    ) else {
        return Vec::new();
    };
    vec![format!(
        "You may cast ~ as though it had flash by tapping {rest} with {} in addition to paying its other costs",
        word.to_lowercase()
    )]
}
