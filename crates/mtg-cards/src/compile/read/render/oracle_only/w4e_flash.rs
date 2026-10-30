//! A standing permission to cast kinds of spells with flash: "You may cast sorcery spells and Dragon spells as though
//! they had flash.", each kind of the choice named before "spells".

use super::super::super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// The wordings of a standing permission to cast the spells of a kind or of a choice of kinds as though they had flash.
pub(super) fn wordings(ability: &Ability) -> Vec<String> {
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let CastPermission {
        who: Ref::You,
        what: Selector(preds),
        from: None,
        free: false,
        flash: true,
        only: None,
        play: false,
        once: false,
        additional: None,
    } = permission
    else {
        return Vec::new();
    };
    let [Pred::Spell, kinds] = preds.as_slice() else {
        return Vec::new();
    };
    let named: Option<Vec<String>> = match kinds {
        Pred::AnyOf(choice) => choice.iter().map(kind_spells).collect(),
        one => kind_spells(one).map(|spells| vec![spells]),
    };
    let Some(named) = named else {
        return Vec::new();
    };
    vec![format!(
        "You may cast {} as though they had flash.",
        named.join(" and ")
    )]
}

/// "sorcery spells", "Dragon spells": one kind of spell.
fn kind_spells(kind: &Pred) -> Option<String> {
    let noun = phrase_text(&Selector(vec![kind.clone()]), Form::Each)?;
    Some(format!("{noun} spells"))
}
