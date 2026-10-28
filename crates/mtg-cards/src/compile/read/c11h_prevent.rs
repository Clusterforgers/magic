//! Prevention for a group and during your turn: "Prevent all noncombat damage that would be dealt to other creatures you
//! control.", "Prevent all combat damage that would be dealt to attacking creatures you control." (a `Replacement` of
//! `Damage` to `Each` of the group by `Prevent`), and "During your turn, prevent all damage that would be dealt to you."
//! (`to` you or the card, under `YourTurn`). Read from the wording and written back the same way.

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Condition;

/// "Prevent all [combat|noncombat] damage that would be dealt to <group>.", and the same for you or the card after
/// "During your turn,".
pub(super) fn c11h_read(r: &mut Reader<'_>) -> Option<Ability> {
    let during = r.attempt(|r| {
        r.need("during your turn")?;
        r.punct(',').then_some(())
    });
    r.need("prevent all")?;
    let combat = c11h_combat(r);
    r.need("damage that would be dealt to")?;
    let to = if during.is_some() && r.word("you") {
        Ref::You
    } else if during.is_some() && r.source() {
        Ref::This
    } else {
        let group = phrase(r, Form::All)?;
        c11h_permanents(&group).then_some(())?;
        Ref::Each(group)
    };
    Some(Ability::Replacement {
        when: ReplaceWhen::Damage {
            to: Some(to),
            source: Selector::default(),
            combat,
        },
        by: ReplaceBy::Prevent,
        condition: during.map(|()| Condition::YourTurn),
        from: ZoneKind::Battlefield,
    })
}

/// "combat", "noncombat" or nothing: only combat damage, only the rest, or both.
fn c11h_combat(r: &mut Reader<'_>) -> Option<bool> {
    if r.word("combat") {
        Some(true)
    } else if r.word("noncombat") {
        Some(false)
    } else {
        None
    }
}

/// Whether a group is permanents on the battlefield, read as a plural noun.
fn c11h_permanents(group: &Selector) -> bool {
    !group.0.is_empty()
        && group
            .0
            .iter()
            .all(|pred| !matches!(pred, Pred::Spell | Pred::In(_) | Pred::This))
}

/// What `c11h_read` reads, written back; empty for any other ability.
pub(super) fn c11h_texts(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when:
            ReplaceWhen::Damage {
                to: Some(to),
                source,
                combat,
            },
        by: ReplaceBy::Prevent,
        condition,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    if !source.0.is_empty() {
        return Vec::new();
    }
    let kind = match combat {
        Some(true) => "combat damage",
        Some(false) => "noncombat damage",
        None => "damage",
    };
    let (whom, lead) = match (to, condition) {
        (Ref::Each(group), None) if c11h_permanents(group) => (phrase_text(group, Form::All), ""),
        (Ref::You, Some(Condition::YourTurn)) => (Some("you".to_owned()), "during your turn, "),
        (Ref::This, Some(Condition::YourTurn)) => (Some("~".to_owned()), "during your turn, "),
        _ => return Vec::new(),
    };
    whom.map(|whom| format!("{lead}prevent all {kind} that would be dealt to {whom}"))
        .into_iter()
        .collect()
}
