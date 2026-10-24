//! A permission to cast spells at instant speed: "You may cast green creature spells as though they had flash." The
//! renderers write the permission for the cards of a zone only.

use super::super::{Form, phrase_text};
use crate::ability::Ability;
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Ref, Selector};

/// Adds the wordings of a rule that lets the controller cast the spells it names as though they had flash.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition: None,
        ..
    } = ability
    else {
        return;
    };
    let CastPermission {
        who,
        what,
        from,
        free,
        flash,
        only,
        play,
    } = permission;
    if *who != Ref::You || from.is_some() || *free || !*flash || only.is_some() || *play {
        return;
    }
    let mut spells = vec![Pred::Spell];
    spells.extend(what.0.iter().cloned());
    let Some(group) = phrase_text(&Selector(spells), Form::All) else {
        return;
    };
    texts.push(format!("You may cast {group} as though they had flash."));
    // A union of subtypes is "Aura and Equipment spells", which the phrase writes with "or".
    if group.contains(" or ") {
        let both = group.replace(" or ", " and ");
        texts.push(format!("You may cast {both} as though they had flash."));
    }
}
