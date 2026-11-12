//! A target the caster can't choose as a spell is cast (CR 601.2c): "You can't choose an untapped creature as this
//! spell's target as you cast it."

use super::Reader;
use super::selectors::{Form, phrase, phrase_text};
use super::triggers::article;
use crate::ability::Ability;

/// The words after the objects.
const TAIL: &str = "as this spell's target as you cast it";

/// "You can't choose <an object> as this spell's target as you cast it."
pub(super) fn r405_cant_target(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("you can't choose")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let what = phrase(r, Form::Subject)?;
    r.need("as")?;
    // "this spell's" is the source and its possessive in the tokens.
    let own = r.words("this spell's") || (r.source() && r.punct('\'') && r.word("s"));
    own.then_some(())?;
    r.need("target as you cast it")?;
    Some(Ability::CantTargetAsCast { what })
}

/// The sentence of the restriction, without the period; empty for any other ability.
pub(super) fn r405_cant_target_texts(ability: &Ability) -> Vec<String> {
    let Ability::CantTargetAsCast { what } = ability else {
        return Vec::new();
    };
    phrase_text(what, Form::Subject)
        .map(|objects| format!("You can't choose {} {objects} {TAIL}", article(&objects)))
        .into_iter()
        .collect()
}
