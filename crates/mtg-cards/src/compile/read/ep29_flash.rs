//! A card's permission to cast itself as though it had flash for more mana: "You may cast this spell as though it had flash
//! if you pay {2} more to cast it." It is a `MayCast` rule of the card that works from its owner's hand and adds the mana
//! to the cast it allows.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::mana::ManaCostX;
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// The permission: the card itself, cast by its owner from a hand as though it had flash for `mana` more.
fn ep29_permission(mana: ManaCostX) -> CastPermission {
    CastPermission {
        rather: None,
        who: Ref::You,
        what: Selector(vec![Pred::This]),
        from: None,
        free: false,
        flash: true,
        only: None,
        play: false,
        once: false,
        additional: Some(Cost(vec![CostItem::Mana(mana)])),
        spend_as: crate::modify::SpendAs::Normal,
        when: None,
        any_cast: false,
        riders: Vec::new(),
    }
}

/// "You may cast ~ as though it had flash if you pay {2} more to cast it".
pub(super) fn ep29_flash(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("you may cast")?;
    r.source().then_some(())?;
    r.need("as though it had flash if you pay")?;
    let mana = super::costs::mana(r)?;
    (!mana.has_x()).then_some(())?;
    r.need("more to cast it")?;
    Some(Ability::Rule {
        rule: GameRule::MayCast(ep29_permission(mana)),
        condition: None,
        from: ZoneKind::Hand,
    })
}

/// The wording of a rule `ep29_flash` reads; empty for another rule.
pub(super) fn ep29_flash_texts(
    rule: &GameRule,
    condition: Option<&crate::value::Condition>,
) -> Vec<String> {
    let GameRule::MayCast(permission) = rule else {
        return Vec::new();
    };
    let Some(Cost(items)) = &permission.additional else {
        return Vec::new();
    };
    let mut plain = permission.clone();
    plain.additional = Some(Cost(vec![CostItem::Mana(ManaCostX::default())]));
    if condition.is_some() || plain != ep29_permission(ManaCostX::default()) {
        return Vec::new();
    }
    match items.as_slice() {
        [CostItem::Mana(mana)] if !mana.has_x() => vec![format!(
            "You may cast ~ as though it had flash if you pay {mana} more to cast it"
        )],
        // "by tapping three untapped creatures you control with flying in addition to paying its other costs".
        [tap @ CostItem::TapPermanents { .. }] => super::costs::text(&Cost(vec![tap.clone()]))
            .and_then(|said| said.strip_prefix("Tap ").map(str::to_owned))
            .map(|rest| {
                vec![format!(
                    "You may cast ~ as though it had flash by tapping {rest} in addition to paying its other costs"
                )]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}
