//! Keywords given to spells: "Nonartifact spells you cast have improvise.", "Artifact spells you cast have convoke.",
//! "Noncreature spells have convoke.", "Enchantment spells you cast have affinity for Auras." A `Static` over the spells a
//! selector names whose changes are the keywords that help pay for a spell (convoke, improvise, delve, affinity), the ones
//! the engine reads on a spell.

use super::selectors::{Form, phrase, phrase_text};
use super::statics::{cost_subject, cost_subject_text, statics};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector};

/// The keywords without data a static ability gives to spells, as the words of the card.
const HELPING: [(&str, Keyword); 3] = [
    ("convoke", Keyword::Convoke),
    ("improvise", Keyword::Improvise),
    ("delve", Keyword::Delve),
];

/// "Nonartifact spells you cast have improvise": the spells named, then the keywords given.
pub(super) fn ability(r: &mut Reader<'_>) -> Option<Ability> {
    let (applies_to, itself) = cost_subject(r)?;
    (!itself).then_some(())?;
    r.word("have").then_some(())?;
    let mut mods = vec![keyword(r)?];
    while r.word("and") {
        mods.push(keyword(r)?);
    }
    Some(statics(Ref::Each(applies_to), mods, None))
}

fn keyword(r: &mut Reader<'_>) -> Option<Modification> {
    if r.words("affinity for") {
        return phrase(r, Form::All).map(|kind| Modification::AddKeyword(Keyword::Affinity(kind)));
    }
    HELPING
        .iter()
        .find(|(name, _)| r.word(name))
        .map(|(_, keyword)| Modification::AddKeyword(keyword.clone()))
}

/// Every wording of the ability above, when `affected` names spells.
pub(super) fn texts(affected: &Ref, mods: &[Modification]) -> Vec<String> {
    let Ref::Each(selector @ Selector(preds)) = affected else {
        return Vec::new();
    };
    if preds.first() != Some(&Pred::Spell) {
        return Vec::new();
    }
    let (Some(subject), Some(given)) = (cost_subject_text(selector), given_text(mods)) else {
        return Vec::new();
    };
    vec![format!("{subject} have {given}")]
}

/// The keywords given, joined with "and".
fn given_text(mods: &[Modification]) -> Option<String> {
    let names: Option<Vec<String>> = mods
        .iter()
        .map(|modification| match modification {
            Modification::AddKeyword(Keyword::Affinity(kind)) => {
                Some(format!("affinity for {}", phrase_text(kind, Form::All)?))
            }
            Modification::AddKeyword(keyword) => HELPING
                .iter()
                .find(|(_, helping)| helping == keyword)
                .map(|(name, _)| (*name).to_owned()),
            _ => None,
        })
        .collect();
    names
        .filter(|names| !names.is_empty())
        .map(|names| names.join(" and "))
}
