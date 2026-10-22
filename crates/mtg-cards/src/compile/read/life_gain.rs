//! Life gain that a card changes: "If you would gain life, you gain that much life plus 1 instead.", "If you would gain
//! life, you gain twice that much life instead." (a `Replacement` of `LifeGain`), and "Players can't gain life.", "Your
//! opponents can't gain life." (the rule `CantGainLife`).

use super::tokens::Reader;
use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// "If you would gain life, you gain that much life plus 1 instead.", "... you gain twice that much life instead."
pub(super) fn replacement(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("if you would gain life")?;
    r.punct(',').then_some(())?;
    r.need("you gain")?;
    let by = if r.word("twice") {
        r.need("that much life")?;
        ReplaceBy::Multiply(2)
    } else {
        r.need("that much life plus")?;
        ReplaceBy::Add(r.number().filter(|more| *more > 0)?)
    };
    r.word("instead").then_some(())?;
    Some(Ability::Replacement {
        when: ReplaceWhen::LifeGain { who: Ref::You },
        by,
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// What `replacement` reads, written back; empty for any other ability.
pub(super) fn replacement_sentences(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when: ReplaceWhen::LifeGain { who: Ref::You },
        by,
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    match by {
        ReplaceBy::Multiply(2) => {
            vec!["if you would gain life, you gain twice that much life instead".to_owned()]
        }
        ReplaceBy::Add(more) if *more > 0 => vec![format!(
            "if you would gain life, you gain that much life plus {more} instead"
        )],
        _ => Vec::new(),
    }
}

/// "Players can't gain life.", "Your opponents can't gain life.", "You can't gain life."
pub(super) fn rule(r: &mut Reader<'_>) -> Option<GameRule> {
    let who = if r.words("players can't gain life") {
        Ref::EachPlayer
    } else if r.words("your opponents can't gain life") {
        Ref::Opponents
    } else if r.words("you can't gain life") {
        Ref::You
    } else {
        return None;
    };
    Some(GameRule::CantGainLife(who))
}

/// What `rule` reads, written back.
pub(super) fn rule_sentences(who: &Ref) -> Vec<String> {
    let text = match who {
        Ref::EachPlayer => "players can't gain life",
        Ref::Opponents => "your opponents can't gain life",
        Ref::You => "you can't gain life",
        _ => return Vec::new(),
    };
    vec![text.to_owned()]
}
