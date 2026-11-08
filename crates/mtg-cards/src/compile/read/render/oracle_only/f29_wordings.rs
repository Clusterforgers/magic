//! Oracle wordings of rules the renderers write another way (fix29): "Counter target spell unless its controller pays
//! {3}. If that spell is countered this way, exile it" for a toll that counters and exiles; "If you do," before a
//! delayed return in a choice; "it deals" for the card in a conditional second step; one sentence for two damages of
//! the card; "That player" for a target opponent; "is put into a graveyard" for the attached permanent; "that creature"
//! for the attacker that deals damage; and "a permanent other than ~" for "another permanent".

use serde_json::{Value, json};

use crate::ability::Ability;

/// Adds the oracle wordings of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(shape) = serde_json::to_value(ability) else {
        return;
    };
    let Some(body) = shape.as_object().and_then(|map| map.values().next()) else {
        return;
    };
    let effects = body.get("effects").cloned().unwrap_or(Value::Null);
    let rules: [(fn(&Value, &Value) -> bool, fn(&str) -> Option<String>); 8] = [
        (counter_exile_toll, toll_first),
        (choice_then_delayed, if_you_do),
        (damage_then_if_damage, it_deals),
        (two_damages, one_damage_sentence),
        (|_, _| true, that_player),
        (attached_to_graveyard, without_battlefield),
        (attacker_deals, that_creature_deals),
        (another_leaves, other_than_self),
    ];
    let found: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            rules
                .iter()
                .filter(|(applies, _)| applies(body, &effects))
                .filter_map(|(_, write)| write(text))
                .collect::<Vec<_>>()
        })
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// One toll whose unpaid steps are an exile shield for the target spell and its counter.
fn counter_exile_toll(_: &Value, effects: &Value) -> bool {
    let Some([toll]) = effects.as_array().map(Vec::as_slice) else {
        return false;
    };
    let Some(pay) = toll.get("Pay") else {
        return false;
    };
    let Some([shield, counter]) = pay
        .get("unpaid")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return false;
    };
    let target = counter.pointer("/Counter/what");
    pay.get("paid").is_none()
        && shield.pointer("/Shield/by") == Some(&json!({"MoveTo": "Exile"}))
        && target.is_some()
        && shield
            .pointer("/Shield/when/Countered/what")
            .and_then(Value::as_array)
            .is_some_and(|what| what.iter().any(|pred| Some(&pred["Is"]) == target))
}

/// "Counter X. If that spell is countered this way, exile it ... unless P pays {N}." with the toll after the counter.
fn toll_first(text: &str) -> Option<String> {
    const SHIELD: &str = "If that spell is countered this way, ";
    let body = text.strip_suffix('.')?;
    let (head, shield) = body
        .split_once(&format!(". {SHIELD}"))
        .or_else(|| body.split_once(&format!(". Then {}", SHIELD.to_lowercase())))?;
    let at = shield.rfind(" unless ")?;
    let (shield, unless) = shield.split_at(at);
    (!head.contains(". ") && !shield.contains(". ") && unless.contains(" pays "))
        .then(|| format!("{head}{unless}. {SHIELD}{shield}."))
}

/// A choice of your own of two steps, the second a delayed trigger.
fn choice_then_delayed(_: &Value, effects: &Value) -> bool {
    let Some([may]) = effects.as_array().map(Vec::as_slice) else {
        return false;
    };
    may.pointer("/May/who").is_none()
        && may
            .pointer("/May/effects")
            .and_then(Value::as_array)
            .is_some_and(|steps| steps.len() == 2 && steps[1].get("DelayedTrigger").is_some())
}

/// "you may A. B." as "you may A. If you do, b.": the text after the choice is two sentences, both in the choice.
fn if_you_do(text: &str) -> Option<String> {
    let (head, choice) = text.split_once("you may ")?;
    let (first, second) = choice.strip_suffix('.')?.split_once(". ")?;
    let mut letters = second.chars();
    let initial = letters.next().filter(char::is_ascii_uppercase)?;
    (!head.contains("may") && !second.contains(". ") && !second.starts_with("Then ")).then(|| {
        format!(
            "{head}you may {first}. If you do, {}{}.",
            initial.to_ascii_lowercase(),
            letters.as_str()
        )
    })
}

/// A spell that deals damage and then, under a condition, deals damage again, both from the card.
fn damage_then_if_damage(body: &Value, effects: &Value) -> bool {
    let Some([first, second]) = effects.as_array().map(Vec::as_slice) else {
        return false;
    };
    let then = second.pointer("/If/then").and_then(Value::as_array);
    body.get("when").is_none()
        && first.pointer("/DealDamage/source").is_none()
        && first.get("DealDamage").is_some()
        && then.is_some_and(|steps| {
            matches!(steps.as_slice(), [only] if only.get("DealDamage").is_some()
                && only.pointer("/DealDamage/source").is_none())
        })
}

/// "~ deals A. If X, ~ deals B" as "If X, it deals B": the card named again by a pronoun.
fn it_deals(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(". If ")?;
    let (condition, rest) = tail.split_once(", ~ deals ")?;
    (head.starts_with("~ deals ") && !head.contains(". ") && !condition.contains(". "))
        .then(|| format!("{head}. If {condition}, it deals {rest}"))
}

/// Two damages of the card in a row and nothing else.
fn two_damages(_: &Value, effects: &Value) -> bool {
    effects.as_array().is_some_and(|steps| {
        steps.len() == 2
            && steps.iter().all(|step| {
                step.get("DealDamage").is_some() && step.pointer("/DealDamage/source").is_none()
            })
    })
}

/// "~ deals 1 damage to A. ~ deals 1 damage to B." as "~ deals 1 damage to A and 1 damage to B.".
fn one_damage_sentence(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(". ~ deals ")?;
    let (_, first) = head.rsplit_once("~ deals ")?;
    (!first.contains(". ") && !tail.contains(". ") && tail.ends_with('.'))
        .then(|| format!("{head} and {tail}"))
}

/// "target opponent. That opponent ..." as "That player ...".
fn that_player(text: &str) -> Option<String> {
    const FROM: &str = " target opponent. That opponent ";
    text.contains(FROM)
        .then(|| text.replacen(FROM, " target opponent. That player ", 1))
        .map(|text| text.replacen("That player also ", "That player ", 1))
}

/// A trigger on the attached permanent going from the battlefield to a graveyard.
fn attached_to_graveyard(body: &Value, _: &Value) -> bool {
    body.pointer("/when/ZoneChange").is_some_and(|change| {
        change.get("from") == Some(&json!("Battlefield"))
            && change.get("to") == Some(&json!("Graveyard"))
            && change.get("what") == Some(&json!([{"Is": "Attached"}]))
    })
}

/// "When enchanted artifact is put into a graveyard from the battlefield" without "from the battlefield": the attached
/// permanent is on the battlefield.
fn without_battlefield(text: &str) -> Option<String> {
    const FROM: &str = " is put into a graveyard from the battlefield, ";
    let (head, _) = text.split_once(FROM)?;
    (head.starts_with("When enchanted ") || head.starts_with("When equipped "))
        .then(|| text.replacen(FROM, " is put into a graveyard, ", 1))
}

/// A choice of your own to have the object of the trigger deal damage.
fn attacker_deals(body: &Value, effects: &Value) -> bool {
    let Some([may]) = effects.as_array().map(Vec::as_slice) else {
        return false;
    };
    body.pointer("/when/Attacks").is_some()
        && may.pointer("/May/who").is_none()
        && may
            .pointer("/May/effects")
            .and_then(Value::as_array)
            .is_some_and(|steps| {
                matches!(steps.as_slice(), [only]
                    if only.pointer("/DealDamage/source") == Some(&json!("Triggering")))
            })
}

/// "you may have it deal" as "you may have that creature deal".
fn that_creature_deals(text: &str) -> Option<String> {
    const FROM: &str = " attacks, you may have it deal ";
    text.contains(FROM)
        .then(|| text.replacen(FROM, " attacks, you may have that creature deal ", 1))
}

/// A trigger on another permanent going from the battlefield to a graveyard.
fn another_leaves(body: &Value, _: &Value) -> bool {
    body.pointer("/when/ZoneChange").is_some_and(|change| {
        change.get("from") == Some(&json!("Battlefield"))
            && change.get("to") == Some(&json!("Graveyard"))
            && change.get("what") == Some(&json!(["Other"]))
    })
}

/// "Whenever another permanent is put into a graveyard" as "Whenever a permanent other than ~ is put into a graveyard".
fn other_than_self(text: &str) -> Option<String> {
    const FROM: &str = "Whenever another permanent is put into a graveyard from the battlefield, ";
    text.starts_with(FROM).then(|| {
        text.replacen(
            FROM,
            "Whenever a permanent other than ~ is put into a graveyard from the battlefield, ",
            1,
        )
    })
}
