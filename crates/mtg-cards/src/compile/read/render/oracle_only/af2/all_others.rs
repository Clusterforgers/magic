//! A change to every other creature for the turn: "All other creatures get -1/-1 until end of turn." The renderers
//! open with the duration and say "each other creature gets".

use super::super::super::{Form, phrase_text};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::{Pred, Ref};

/// Adds the wordings that put the duration last and name the group with "all".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for effect in ability.effects() {
        let Effect::Continuous {
            affected: Ref::Each(group),
            duration: Duration::UntilEndOfTurn,
            ..
        } = effect
        else {
            continue;
        };
        if !group.0.contains(&Pred::Other) {
            continue;
        }
        let (Some(each), Some(all)) = (
            phrase_text(group, Form::Each),
            phrase_text(group, Form::All),
        ) else {
            continue;
        };
        let from = format!("Until end of turn, each {each} gets ");
        let said: Vec<String> = texts
            .iter()
            .filter_map(|text| {
                let at = text.find(&from)?;
                let (head, rest) = (&text[..at], &text[at + from.len()..]);
                let (change, tail) = rest.split_once('.')?;
                Some(format!(
                    "{head}All {all} get {change} until end of turn.{tail}"
                ))
            })
            .collect();
        texts.extend(said);
    }
}
