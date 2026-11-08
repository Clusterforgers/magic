//! A keyword an Aura or Equipment gives under a condition of what it is attached to ("As long as enchanted creature is a
//! Human, it has trample.", "Enchanted creature has shroud as long as it's untapped.", "Equipped creature has double
//! strike as long as two or more Equipment are attached to it."): the ability is written under "it's your turn" and the
//! condition's words are put in its place.

use super::super::super::triggers::article;
use super::w4b_stand::{json, stand, swapped, word};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector};
use serde_json::{Value, json};

/// Adds the wordings of a static ability over what the card is attached to under such a condition.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Static {
        affected,
        condition,
        mods,
        ..
    } = ability
    else {
        return;
    };
    if !super::w21_words::only_keywords(mods) {
        return;
    }
    match (affected, condition) {
        // The condition is part of what is affected: the attached creature that is a Human.
        (Ref::Each(Selector(preds)), None) if preds.contains(&Pred::Is(Ref::Attached)) => {
            let rest: Vec<&Pred> = preds
                .iter()
                .filter(|pred| **pred != Pred::Is(Ref::Attached))
                .collect();
            let Some(state) = kinds(&rest) else {
                return;
            };
            let Some(stand) = stand(ability, |value| {
                let fields = value.pointer_mut("/Static")?.as_object_mut()?;
                fields.insert("affected".to_owned(), json!("Attached"));
                fields.insert("condition".to_owned(), json!("YourTurn"));
                Some(())
            }) else {
                return;
            };
            about_it(&stand, &state, texts);
        }
        (Ref::Attached, Some(condition)) => {
            let Some(stand) = stand(ability, |value| {
                *value.pointer_mut("/Static/condition")? = json!("YourTurn");
                Some(())
            }) else {
                return;
            };
            let condition = json(condition);
            if let Some(state) = untapped(&condition) {
                about_it(&stand, state, texts);
            }
            if let Some(held) = equipment_attached(&condition) {
                let held = format!("as long as {held}");
                texts.extend(swapped(&stand, &[("as long as it's your turn", &held)]));
            }
        }
        _ => {}
    }
}

/// "as long as it's a Human" and "As long as enchanted creature is a Human, it has trample."
fn about_it(stand: &Ability, state: &str, texts: &mut Vec<String>) {
    texts.extend(swapped(
        stand,
        &[(
            "as long as it's your turn",
            &format!("as long as it's {state}"),
        )],
    ));
    for noun in ["equipped creature", "enchanted creature"] {
        let from = format!("As long as it's your turn, {noun}");
        let to = format!("As long as {noun} is {state}, it");
        texts.extend(swapped(stand, &[(&from, &to)]));
    }
}

/// "a Human", "a Human or an Angel": the subtypes the attached creature has one of.
fn kinds(preds: &[&Pred]) -> Option<String> {
    let alternatives: Vec<&Pred> = match preds {
        [Pred::Subtype(_)] => preds.to_vec(),
        [Pred::AnyOf(any)] => any.iter().collect(),
        _ => return None,
    };
    let names: Option<Vec<String>> = alternatives
        .iter()
        .map(|pred| match pred {
            Pred::Subtype(name) => Some(format!("{} {}", article(&name.0), name.0)),
            _ => None,
        })
        .collect();
    names.map(|names| names.join(" or "))
}

/// "untapped" for a condition that the attached creature is untapped.
fn untapped(condition: &Value) -> Option<&'static str> {
    let matches = condition.get("Matches")?;
    (matches.get("subject")? == "Attached").then_some(())?;
    match matches.get("selector")?.to_string().as_str() {
        r#"[{"Type":"Creature"},"Untapped"]"# => Some("untapped"),
        r#"[{"Type":"Creature"},"Tapped"]"# => Some("tapped"),
        _ => None,
    }
}

/// "two or more Equipment are attached to it": Equipment attached to the equipped creature, at least twice.
fn equipment_attached(condition: &Value) -> Option<String> {
    let compare = condition.get("Compare")?;
    (compare.get("op")? == "Ge").then_some(())?;
    let counted = json!([
        {"Subtype": "Equipment"},
        {"AttachedTo": {"Each": [{"Type": "Creature"}, {"Is": "Attached"}]}}
    ]);
    (*compare.pointer("/left/Count")? == counted).then_some(())?;
    let many = word(compare.get("right")?).filter(|many| *many != "one")?;
    Some(format!("{many} or more Equipment are attached to it"))
}
