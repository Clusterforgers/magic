//! Preparing (the `prepare` layout): "This creature enters prepared." is `ReplaceBy::EntersPrepared` and "this creature
//! becomes prepared" is `Effect::Prepare` of the card itself. The reminder text "(While it's prepared, you may cast a copy of
//! its spell. Doing so unprepares it.)" is dropped with every reminder. The writer's side is `becomes_prepared_text` and
//! the `~ enters prepared` text of the statics.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// "~ becomes prepared.", "Target creature becomes unprepared.", "It becomes unprepared.": one object becomes prepared
/// or unprepared.
pub fn becomes_prepared(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let (what, several) = super::effects::subject(r)?;
    (!several).then_some(())?;
    r.need("becomes")?;
    if r.word("prepared") {
        return Some(vec![Effect::Prepare { what }]);
    }
    r.need("unprepared")?;
    Some(vec![Effect::Unprepare { what }])
}

/// "This creature enters prepared.": the card enters prepared.
pub fn enters_prepared(r: &mut Reader<'_>) -> Option<Ability> {
    r.source().then_some(())?;
    r.need("enters prepared")?;
    Some(Ability::Replacement {
        when: ReplaceWhen::Enters {
            what: Selector::this(),
        },
        by: ReplaceBy::EntersPrepared,
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// The words of `Effect::Prepare` or `Effect::Unprepare` with the words of what it names: "~ becomes prepared".
pub fn becomes_prepared_text(effect: &Effect, what: &str) -> Option<String> {
    match effect {
        Effect::Prepare { .. } => Some(format!("{what} becomes prepared")),
        Effect::Unprepare { .. } => Some(format!("{what} becomes unprepared")),
        _ => None,
    }
}

/// "tap enchanted creature. It becomes unprepared." for "tap enchanted creature and enchanted creature becomes
/// unprepared": the object a step names is "it" in a sentence of its own after it.
pub fn r405_it_becomes(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter_map(|text| {
            let (head, tail) = text.split_once(" and enchanted creature becomes ")?;
            // The step before names the same enchanted object, which the second step calls a creature.
            let head = head
                .strip_suffix(" enchanted permanent")
                .or_else(|| head.strip_suffix(" enchanted creature"))?;
            Some(format!("{head} enchanted creature. It becomes {tail}"))
        })
        .collect()
}
