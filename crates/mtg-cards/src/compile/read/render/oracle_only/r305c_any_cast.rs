//! Wordings of a cast permission that opens no zone (`CastPermission::any_cast`): "You may cast Dragon spells without
//! paying their mana costs.", "You may pay {W}{U}{B}{R}{G} rather than pay the mana cost for spells you cast."

use super::super::super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// Adds the wording of a permission that offers a free cast or another cost for the casts its player makes anyway.
pub(super) fn r305c_add_any_cast(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return;
    };
    texts.extend(r305c_any_cast_text(permission));
}

/// The wording of the permission, or none when it is not a plain one for its controller.
fn r305c_any_cast_text(permission: &CastPermission) -> Option<String> {
    let plain = permission.any_cast
        && permission.who == Ref::You
        && permission.from.is_none()
        && !permission.flash
        && permission.only.is_none()
        && !permission.play
        && !permission.once
        && permission.additional.is_none()
        && permission.spend_as.is_normal()
        && permission.when.is_none();
    if !plain {
        return None;
    }
    let mut preds = vec![Pred::Spell];
    preds.extend(permission.what.0.iter().cloned());
    let spells = phrase_text(&Selector(preds), Form::All)?;
    match (&permission.rather, permission.free) {
        (None, true) => Some(format!(
            "You may cast {spells} without paying their mana costs."
        )),
        (Some(Cost(items)), false) => {
            let [CostItem::Mana(mana)] = items.as_slice() else {
                return None;
            };
            Some(format!(
                "You may pay {mana} rather than pay the mana cost for {spells} you cast."
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a permission that opens no zone is written as the cards print it, and a near miss is not.
    use super::*;
    use crate::modify::SpendAs;
    use mtg_core::card::Subtype;
    use rstest::rstest;

    fn permission(
        what: Vec<Pred>,
        free: bool,
        rather: Option<&str>,
        any_cast: bool,
    ) -> CastPermission {
        CastPermission {
            who: Ref::You,
            what: Selector(what),
            from: None,
            free,
            flash: false,
            only: None,
            play: false,
            once: false,
            additional: None,
            rather: rather.map(|mana| Cost(vec![CostItem::Mana(mana.parse().unwrap())])),
            spend_as: SpendAs::Normal,
            when: None,
            any_cast,
            riders: Vec::new(),
            limit: None,
        }
    }

    #[rstest]
    #[case::dragons_free(permission(vec![Pred::Subtype(Subtype("Dragon".into()))], true, None, true), Some("You may cast Dragon spells without paying their mana costs."))]
    #[case::five_colors(permission(vec![], false, Some("{W}{U}{B}{R}{G}"), true), Some("You may pay {W}{U}{B}{R}{G} rather than pay the mana cost for spells you cast."))]
    #[case::opens_a_zone(permission(vec![], true, None, false), None)]
    #[case::both_free_and_cost(permission(vec![], true, Some("{1}"), true), None)]
    fn r305c_any_cast_wordings(#[case] permission: CastPermission, #[case] text: Option<&str>) {
        assert_eq!(r305c_any_cast_text(&permission).as_deref(), text);
    }
}
