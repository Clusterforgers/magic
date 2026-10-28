//! Spells of a group that can't be countered: "Creature spells you control can't be countered.", "Sliver spells can't be
//! countered.", "Instant and sorcery spells you control can't be countered." (the rule `CantBeCountered` of a group
//! of spells, from the battlefield). Read from the wording and written back the same way.

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// "Creature spells you control can't be countered.", "Green spells you control can't be countered."
pub(super) fn c11h_cant_be_countered(r: &mut Reader<'_>) -> Option<Ability> {
    let spells = phrase(r, Form::All)?;
    c11h_spells(&spells).then_some(())?;
    r.need("can't be countered")?;
    Some(Ability::Rule {
        rule: GameRule::CantBeCountered(Ref::Each(spells)),
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// Whether the group is spells, and of whom only "you control" when it names a controller.
fn c11h_spells(spells: &Selector) -> bool {
    spells.0.contains(&Pred::Spell)
        && spells.0.iter().all(|pred| match pred {
            Pred::ControlledBy(who) => *who == Ref::You,
            Pred::Other | Pred::This | Pred::In(_) => false,
            _ => true,
        })
}

/// What `c11h_cant_be_countered` reads, written back.
pub(super) fn c11h_cant_be_countered_texts(rule: &GameRule) -> Vec<String> {
    let GameRule::CantBeCountered(Ref::Each(spells)) = rule else {
        return Vec::new();
    };
    if !c11h_spells(spells) {
        return Vec::new();
    }
    phrase_text(spells, Form::All)
        .map(|group| format!("{group} can't be countered"))
        .into_iter()
        .collect()
}
