//! A word chosen as a permanent enters (the Sieges' "As this enchantment enters, choose Khans or Dragons."): the
//! `GenericChoice` with `SetChosenMode$ True` that an `ETBReplacement` names is `ReplaceBy::ChooseWord`, and a continuous
//! line of the card itself under `Affected$ Card.ChosenMode<word>` is the ability it adds, which works while that word
//! was chosen (`Condition::EnteredAs`).

use serde_json::{Value, json};

use super::ctx::{Ctx, Tr, unmapped};
use crate::forge::script::Params;

/// The printed word of a choice part: its description ("Khans"), or the part's name when it has none.
fn word_of(ctx: &Ctx, name: &str) -> Option<String> {
    let part = Params::parse(ctx.svar(name)?);
    let word = part.get("SpellDescription").unwrap_or(name).trim();
    let plain = !word.is_empty()
        && word.split(' ').count() <= 3
        && word
            .chars()
            .all(|c| c.is_alphanumeric() || c == ' ' || c == '\'' || c == '-');
    plain.then(|| word.to_owned())
}

/// `GenericChoice | Choices$ Khans,Dragons | SetChosenMode$ True`: the words the controller chooses among as the
/// permanent enters; `None` for any other part.
pub fn choose_word(ctx: &Ctx, part: &Params) -> Option<Tr<Value>> {
    if part.get("DB") != Some("GenericChoice") || part.get("SetChosenMode") != Some("True") {
        return None;
    }
    Some(chosen_words(ctx, part))
}

fn chosen_words(ctx: &Ctx, part: &Params) -> Tr<Value> {
    super::ctx::check_keys(
        part,
        "ETBReplacement",
        &["DB", "Choices", "Defined", "SetChosenMode"],
        |key, _| matches!(key, "ShowChoice" | "LockInText"),
    )?;
    if part.get("Defined").is_some_and(|who| who != "You") {
        return unmapped("key:ETBReplacement.Defined");
    }
    let names: Vec<&str> = part.get("Choices").unwrap_or_default().split(',').collect();
    let words: Option<Vec<String>> = names.iter().map(|name| word_of(ctx, name.trim())).collect();
    match words {
        Some(words) if words.len() >= 2 => Ok(json!({ "ChooseWord": words })),
        _ => unmapped("key:ETBReplacement.GenericChoice-words"),
    }
}

/// `Mode$ Continuous | AffectedDefined$ Self | Affected$ Card.ChosenMode<word> | AddTrigger$ <part>` (or
/// `AddStaticAbility$`): the added ability with the condition that the word was chosen as the permanent entered; `None`
/// for any other line.
pub fn entered_as_line(ctx: &mut Ctx, p: &Params) -> Option<Tr<Value>> {
    let word = p
        .get("Affected")?
        .strip_prefix("Card.ChosenMode")
        .filter(|word| !word.is_empty())?;
    if p.get("Mode") != Some("Continuous") || p.get("AffectedDefined") != Some("Self") {
        return None;
    }
    Some(entered_as(ctx, p, word))
}

fn entered_as(ctx: &mut Ctx, p: &Params, name: &str) -> Tr<Value> {
    super::ctx::check_keys(
        p,
        "Static.ChosenMode",
        &[
            "Mode",
            "AffectedDefined",
            "Affected",
            "AddTrigger",
            "AddStaticAbility",
        ],
        |key, _| matches!(key, "Description" | "LockInText"),
    )?;
    let Some(word) = word_of(ctx, name) else {
        return unmapped("key:Static.ChosenMode-word");
    };
    let (kind, part) = match (p.get("AddTrigger"), p.get("AddStaticAbility")) {
        (Some(part), None) => ("if", part),
        (None, Some(part)) => ("condition", part),
        _ => return unmapped("key:Static.ChosenMode-shape"),
    };
    let Some(text) = ctx.svar(part.trim()).map(str::to_owned) else {
        return unmapped("svar:missing");
    };
    let mut ability = if kind == "if" {
        super::triggers::triggered(ctx, &text)?
    } else {
        super::statics::static_line(ctx, &text)?
    };
    let Some(inner) = ability
        .as_object_mut()
        .and_then(|map| map.values_mut().next())
        .and_then(Value::as_object_mut)
    else {
        return unmapped("key:Static.ChosenMode-shape");
    };
    let held = json!({ "EnteredAs": word });
    let condition = match inner.remove(kind) {
        Some(existing) => json!({"All": [held, existing]}),
        None => held,
    };
    inner.insert(kind.to_owned(), condition);
    Ok(ability)
}
