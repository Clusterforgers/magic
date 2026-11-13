//! Writers of a choice whose chosen objects differ in a quality ("choose any number of creatures with different
//! powers"): the ability is worded without the rule, and the rule follows the noun phrase of the choice.

use serde_json::json;

use super::selectors::{self, Form};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, TargetSet};

/// The words of a rule a choice keeps; `None` for a rule a choice cannot keep.
fn rule_words(rule: &TargetSet) -> Option<&'static str> {
    match rule {
        TargetSet::DifferentNames => Some("with different names"),
        TargetSet::DifferentPowers => Some("with different powers"),
        TargetSet::DifferentManaValues => Some("with different mana values"),
        _ => None,
    }
}

/// Takes the rule out of the only choice of the JSON that keeps one; `None` when there is none or several.
fn take_rule(value: &mut serde_json::Value, found: &mut Vec<serde_json::Value>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Object(choose)) = map.get_mut("Choose")
                && let Some(rule) = choose.remove("differ")
            {
                found.push(rule);
            }
            for inner in map.values_mut() {
                take_rule(inner, found);
            }
        }
        serde_json::Value::Array(items) => {
            for inner in items {
                take_rule(inner, found);
            }
        }
        _ => {}
    }
}

/// The words of an ability with one choice that keeps a rule, from the words of the ability without it: the rule
/// follows the noun phrase after the only "any number of"; `None` when no choice of the ability keeps a rule, and no
/// words when the ability cannot be worded so.
pub(super) fn r4709_differ_texts(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    let mut value = serde_json::to_value(ability).ok()?;
    let mut found = Vec::new();
    take_rule(&mut value, &mut found);
    let [rule] = found.as_slice() else {
        return (!found.is_empty()).then(Vec::new);
    };
    let words = serde_json::from_value::<TargetSet>(rule.clone())
        .ok()
        .as_ref()
        .and_then(rule_words);
    let (Some(words), Ok(plain)) = (words, serde_json::from_value::<Ability>(value)) else {
        return Some(Vec::new());
    };
    let mut texts: Vec<String> = written(&plain)
        .iter()
        .filter_map(|text| with_rule(text, words))
        .collect();
    texts.extend(chosen_as_targets(&plain, words, written));
    let joined: Vec<String> = texts.iter().filter_map(|text| either_type(text)).collect();
    texts.extend(joined);
    Some(texts)
}

/// "creature or Vehicle cards" after the only "any number of" as "creature and/or Vehicle cards".
fn either_type(text: &str) -> Option<String> {
    const LEAD: &str = "any number of ";
    let start = text.find(LEAD)? + LEAD.len();
    let rest = &text[start..];
    let end = rest.find(" with different ")?;
    let phrase = &rest[..end];
    phrase.contains(" or ").then(|| {
        format!(
            "{}{}{}",
            &text[..start],
            phrase.replacen(" or ", " and/or ", 1),
            &rest[end..]
        )
    })
}

/// The words of "choose any number of <objects> <rule>. <step on them>" from the words of the ability with the
/// choice as a target: one target for "Each of them" and "it", any number for "Those <objects>". Only an ability with
/// no targets whose last two steps are the choice and the one step that names it.
fn chosen_as_targets(
    plain: &Ability,
    words: &str,
    written: fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    let Some((selector, single, many)) = twins(plain) else {
        return Vec::new();
    };
    let (Some(one), Some(several)) = (
        selectors::phrase_text(&selector, Form::Target),
        selectors::phrase_text(&selector, Form::Targets),
    ) else {
        return Vec::new();
    };
    let mut texts = Vec::new();
    for named in std::iter::once(several.clone()).chain(each_kind(&selector)) {
        let lead = format!("choose any number of {named} {words}. ");
        let mut said = Vec::new();
        for text in written(&single) {
            let copy = format!("reate a token that's a copy of target {one}");
            if let Some(at) = text.find(&copy) {
                let capital = text[..at].ends_with('C');
                let lead = if capital {
                    capitalized(&lead)
                } else {
                    lead.clone()
                };
                said.push(format!(
                    "{}{lead}For each of them, create a token that's a copy of it{}",
                    &text[..at - 1],
                    &text[at + copy.len()..]
                ));
            } else {
                said.push(text.replacen(
                    &format!("target {one} "),
                    &format!("{lead}Each of them "),
                    1,
                ));
            }
        }
        for text in written(&many) {
            let plural = format!("any number of target {several} ");
            if text.contains(&plural) && !several.contains(' ') {
                said.push(text.replacen(&plural, &format!("{lead}Those {several} "), 1));
            }
        }
        // "that creature's power" after "Each of them" would name each chosen creature.
        said.retain(|text| {
            !text.contains("that creature's")
                && text.to_lowercase().contains(&lead)
                && !text
                    .to_lowercase()
                    .contains(&format!("until end of turn, {lead}"))
        });
        let capitals: Vec<String> = said
            .iter()
            .filter(|text| text.contains(&format!("\u{2014} {lead}")))
            .map(|text| text.replacen(&lead, &capitalized(&lead), 1))
            .collect();
        said.extend(capitals);
        texts.extend(said);
    }
    texts
}

/// The text with its first letter in capitals.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// "artifact tokens and/or creature tokens you control" for a selector whose first predicate is one of two kinds:
/// each kind as its own plural phrase, the controller once at the end.
fn each_kind(selector: &Selector) -> Option<String> {
    let (Some(Pred::AnyOf(kinds)), rest) = (selector.0.first(), selector.0.get(1..)?) else {
        return None;
    };
    let [first, second] = kinds.as_slice() else {
        return None;
    };
    let own = rest.contains(&Pred::ControlledBy(Ref::You));
    let others: Vec<Pred> = rest
        .iter()
        .filter(|pred| **pred != Pred::ControlledBy(Ref::You))
        .cloned()
        .collect();
    let phrase = |kind: &Pred| {
        let mut preds = vec![kind.clone()];
        preds.extend(others.iter().cloned());
        selectors::phrase_text(&Selector(preds), Form::Targets)
    };
    let joined = format!("{} and/or {}", phrase(first)?, phrase(second)?);
    Some(if own {
        format!("{joined} you control")
    } else {
        joined
    })
}

/// The selector of the choice and the ability with the choice as one target and as any number of targets.
fn twins(plain: &Ability) -> Option<(Selector, Ability, Ability)> {
    let value = serde_json::to_value(plain).ok()?;
    let (kind, inner) = value.as_object()?.iter().next()?;
    let inner = inner.as_object()?;
    if inner.contains_key("targets") {
        return None;
    }
    let effects = inner.get("effects")?.as_array()?;
    let [before @ .., choose, step] = effects.as_slice() else {
        return None;
    };
    let choose = choose.get("Choose")?.as_object()?;
    if choose.len() != 3 || choose.get("count")? != &json!({"min": 0, "max": null}) {
        return None;
    }
    let bind = choose.get("as")?.as_str()?;
    let each = choose.get("from")?.get("Each")?;
    let selector: Selector = serde_json::from_value(each.clone()).ok()?;
    let bound = json!({ "Bound": bind });
    if contains(&serde_json::Value::Array(before.to_vec()), &bound) {
        return None;
    }
    let mut step = step.clone();
    if !replace(&mut step, &bound, &json!({ "Target": 0 })) {
        return None;
    }
    let build = |bounds: Option<serde_json::Value>| -> Option<Ability> {
        let mut inner = inner.clone();
        let mut spec = json!({ "what": { "Object": each } });
        if let (Some(bounds), Some(spec)) = (bounds, spec.as_object_mut()) {
            spec.insert("bounds".to_owned(), bounds);
        }
        inner.insert("targets".to_owned(), json!([spec]));
        let mut steps = before.to_vec();
        steps.push(step.clone());
        inner.insert("effects".to_owned(), serde_json::Value::Array(steps));
        serde_json::from_value(json!({ kind: inner })).ok()
    };
    Some((
        selector,
        build(None)?,
        build(Some(json!({"min": 0, "max": null})))?,
    ))
}

/// Whether the JSON holds the value anywhere.
fn contains(value: &serde_json::Value, wanted: &serde_json::Value) -> bool {
    value == wanted
        || match value {
            serde_json::Value::Object(map) => map.values().any(|inner| contains(inner, wanted)),
            serde_json::Value::Array(items) => items.iter().any(|inner| contains(inner, wanted)),
            _ => false,
        }
}

/// Replaces every copy of `from` in the JSON by `to`; whether there was one.
fn replace(
    value: &mut serde_json::Value,
    from: &serde_json::Value,
    to: &serde_json::Value,
) -> bool {
    if value == from {
        *value = to.clone();
        return true;
    }
    let mut any = false;
    match value {
        serde_json::Value::Object(map) => {
            for inner in map.values_mut() {
                any |= replace(inner, from, to);
            }
        }
        serde_json::Value::Array(items) => {
            for inner in items {
                any |= replace(inner, from, to);
            }
        }
        _ => {}
    }
    any
}

/// The text with the words after the noun phrase that follows its only "any number of".
fn with_rule(text: &str, words: &str) -> Option<String> {
    const LEAD: &str = "any number of ";
    let start = text.find(LEAD)? + LEAD.len();
    if text[start..].contains(LEAD) {
        return None;
    }
    let rest = &text[start..];
    let end = [" from among", ".", ","]
        .iter()
        .filter_map(|stop| rest.find(stop))
        .min()?;
    let at = start + end;
    Some(format!("{} {words}{}", &text[..at], &text[at..]))
}

#[cfg(test)]
mod tests {
    //! Goal: the rule of a choice follows the noun phrase after its only "any number of", before a period, comma or
    //! "from among", and a text with no such phrase, or two, takes no rule.
    use rstest::rstest;

    use super::with_rule;

    #[rstest]
    #[case(
        "Choose any number of creatures. Those creatures gain double strike until end of turn.",
        Some(
            "Choose any number of creatures with different powers. Those creatures gain double strike until end of turn."
        )
    )]
    #[case(
        "Put any number of creature cards from among them into your hand.",
        Some(
            "Put any number of creature cards with different powers from among them into your hand."
        )
    )]
    #[case("Choose a creature.", None)]
    #[case("Choose any number of creatures and any number of lands.", None)]
    fn the_rule_follows_the_noun_phrase(#[case] text: &str, #[case] wanted: Option<&str>) {
        assert_eq!(with_rule(text, "with different powers").as_deref(), wanted);
    }
}
