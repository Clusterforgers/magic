//! Preparing (the `prepare` layout): "This creature enters prepared." is `ReplaceBy::EntersPrepared` and "this creature
//! becomes prepared" is `Effect::Prepare` of the card itself. The reminder text "(While it's prepared, you may cast a copy of
//! its spell. Doing so unprepares it.)" is dropped with every reminder. The writer's side is `becomes_prepared_text` and
//! the `~ enters prepared` text of the statics.

use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// "~ becomes prepared.": the card itself becomes prepared.
pub fn becomes_prepared(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.source().then_some(())?;
    r.need("becomes prepared")?;
    Some(vec![Effect::Prepare { what: Ref::This }])
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

/// The words of `Effect::Prepare` of the card itself: "~ becomes prepared".
pub fn becomes_prepared_text(effect: &Effect) -> Option<String> {
    matches!(effect, Effect::Prepare { what: Ref::This }).then(|| "~ becomes prepared".to_owned())
}
