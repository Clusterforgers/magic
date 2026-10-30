//! Statics of an Aura that depend on who controls the enchanted creature: the condition "another player controls
//! enchanted creature", and the sentence that follows a conditional static and takes the condition back: "As long as
//! another player controls enchanted creature, it can't attack or block. Otherwise, this Aura has umbra armor."

use super::selectors::{granted_keywords, granted_text};
use super::statics::{self, statics};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Condition;

/// The nouns an Aura or Equipment calls the creature it is attached to.
const NOUNS: [&str; 2] = ["enchanted creature", "equipped creature"];

/// Who controls the attached creature, as the predicate a condition tests: "another player" (not the card's
/// controller), "an opponent", "you".
fn controllers() -> [(&'static str, Pred); 3] {
    [
        (
            "another player controls",
            Pred::Not(Box::new(Pred::ControlledBy(Ref::You))),
        ),
        ("an opponent controls", Pred::ControlledBy(Ref::Opponents)),
        ("you control", Pred::ControlledBy(Ref::You)),
    ]
}

/// "another player controls enchanted creature", "an opponent controls equipped creature", "you control enchanted
/// creature".
pub(super) fn controls_read(r: &mut Reader<'_>) -> Option<Condition> {
    let (_, pred) = controllers()
        .into_iter()
        .find(|(words, _)| r.words(words))?;
    NOUNS.iter().any(|noun| r.words(noun)).then_some(())?;
    Some(Condition::Matches {
        subject: Ref::Attached,
        selector: Selector(vec![pred]),
    })
}

/// What `controls_read` reads, written with each noun.
pub(super) fn controls_texts(condition: &Condition) -> Vec<String> {
    let Condition::Matches {
        subject: Ref::Attached,
        selector: Selector(preds),
    } = condition
    else {
        return Vec::new();
    };
    let [pred] = preds.as_slice() else {
        return Vec::new();
    };
    controllers()
        .into_iter()
        .filter(|(_, wanted)| wanted == pred)
        .flat_map(|(words, _)| NOUNS.iter().map(move |noun| format!("{words} {noun}")))
        .collect()
}

/// Whether the condition's wording ends with the attached creature, which "it" then stands for.
pub(super) fn ends_with_attached(held: &str) -> bool {
    NOUNS.iter().any(|noun| held.ends_with(noun))
}

/// A conditional static and the sentence "Otherwise, this Aura has umbra armor.": the card has the keywords under the
/// opposite condition.
pub(super) fn otherwise_read(r: &mut Reader<'_>) -> Option<Vec<Ability>> {
    let first = statics::ability(r)?;
    let Ability::Static {
        condition: Some(held),
        ..
    } = &first
    else {
        return None;
    };
    let held = held.clone();
    r.need("otherwise")?;
    r.punct(',').then_some(())?;
    r.source().then_some(())?;
    r.word("has").then_some(())?;
    let keywords = keyword_list(r)?;
    r.sentence_end().then_some(())?;
    let second = statics(
        Ref::This,
        keywords.into_iter().map(Modification::AddKeyword).collect(),
        Some(Condition::Not(Box::new(held))),
    );
    Some(vec![first, second])
}

/// "umbra armor", "flying and lifelink".
fn keyword_list(r: &mut Reader<'_>) -> Option<Vec<Keyword>> {
    if r.words("umbra armor") {
        return Some(vec![Keyword::UmbraArmor]);
    }
    let mut found = granted_keywords(r)?;
    while let Some(more) = r.attempt(|r| {
        r.word("and").then_some(())?;
        granted_keywords(r)
    }) {
        found.extend(more);
    }
    Some(found)
}

/// Every wording of a conditional static and the card's keywords under the opposite condition, as the two sentences.
pub(super) fn otherwise_texts(first: &Ability, second: &Ability) -> Vec<String> {
    let (
        Ability::Static {
            condition: Some(held),
            ..
        },
        Ability::Static {
            affected: Ref::This,
            mods,
            condition: Some(Condition::Not(opposite)),
            defining: false,
            from: ZoneKind::Battlefield,
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    if held != &**opposite {
        return Vec::new();
    }
    let keywords = match mods.as_slice() {
        [Modification::AddKeyword(Keyword::UmbraArmor)] => "umbra armor".to_owned(),
        _ => match granted_text(mods) {
            Some(text) => text,
            None => return Vec::new(),
        },
    };
    statics::texts(first)
        .into_iter()
        .map(|sentence| format!("{sentence} Otherwise, ~ has {keywords}."))
        .collect()
}

/// "this Aura entered this turn", "~ entered the battlefield this turn": the card itself came onto the battlefield this
/// turn.
pub(super) fn entered_read(r: &mut Reader<'_>) -> Option<Condition> {
    r.source().then_some(())?;
    (r.words("entered this turn") || r.words("entered the battlefield this turn")).then_some(())?;
    Some(Condition::Matches {
        subject: Ref::This,
        selector: Selector(vec![Pred::EnteredThisTurn]),
    })
}

/// What `entered_read` reads, written back.
pub(super) fn entered_texts(condition: &Condition) -> Vec<String> {
    let Condition::Matches {
        subject: Ref::This,
        selector: Selector(preds),
    } = condition
    else {
        return Vec::new();
    };
    if preds.as_slice() != [Pred::EnteredThisTurn] {
        return Vec::new();
    }
    vec![
        "~ entered this turn".to_owned(),
        "~ entered the battlefield this turn".to_owned(),
    ]
}

/// A static ability a permanent is given in quotes that gives keywords to what that permanent is attached to: "Equipped
/// creature has lifelink."
pub(super) fn granted_static(r: &mut Reader<'_>) -> Option<Ability> {
    let found = statics::ability(r)?;
    keywords_for_attached(&found).then_some(found)
}

/// Whether the ability is "Equipped creature has K." without a condition: the one static a quote may hold.
fn keywords_for_attached(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Static {
            affected: Ref::Attached,
            mods,
            condition: None,
            defining: false,
            from: ZoneKind::Battlefield,
        } if !mods.is_empty() && mods.iter().all(|m| matches!(m, Modification::AddKeyword(_)))
    )
}

/// The quoted wording of a granted static ability that `granted_static` reads.
pub(super) fn granted_static_text(ability: &Ability) -> Option<String> {
    if !keywords_for_attached(ability) {
        return None;
    }
    let sentence = statics::texts(ability).into_iter().next()?;
    Some(format!("\"{sentence}\""))
}
