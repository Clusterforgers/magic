//! Other Oracle wordings of what a trigger does and who it is about: "its controller" for the controller of the object
//! of the event, "a source" for a damage source of any kind, "card ... from anywhere" for a nontoken object, and "until
//! end of turn" after the change it lasts for, in either order of "gets" and "gains".

use crate::trigger::TriggerPattern;

/// Every one-step rewrite of a text that depends on the rule or on what the effect says.
pub(super) fn rewrites(text: &str, when: &TriggerPattern, json: &str) -> Vec<String> {
    let mut out = Vec::new();
    out.extend(its_controller(text, json));
    out.extend(kind_controller(text, json));
    out.extend(any_source(text, when));
    if text.contains("graveyard from anywhere") && text.contains("nontoken permanent is put into ")
    {
        out.push(text.replacen("nontoken permanent is put into ", "card is put into ", 1));
    }
    out.extend(until_after(text));
    out.extend(gains_first(text));
    out.extend(creatures_get(text));
    out
}

/// "that player" as "its controller" when the only player the rule names after the event is the controller of the
/// object of the event.
fn its_controller(text: &str, json: &str) -> Option<String> {
    let controller = r#"{"ControllerOf":"Triggering"}"#;
    (json.matches(controller).count() == 1
        && !json.contains(r#""who":"Triggering""#)
        && text.matches("that player").count() == 1)
        .then(|| text.replacen("that player", "its controller", 1))
}

/// "its controller" as "that creature's controller" when the rule names the controller of the object of the event.
fn kind_controller(text: &str, json: &str) -> Vec<String> {
    let named = [
        r#"{"ControllerOf":"Triggering"}"#,
        r#"{"ControllerOf":"Attached"}"#,
    ]
    .iter()
    .filter(|found| json.contains(*found))
    .count();
    if named != 1
        || json.contains("ControllerOf\":\"This")
        || text.matches(" its controller").count() != 1
    {
        return Vec::new();
    }
    let head = text.split(", ").next().unwrap_or_default();
    ["creature", "land", "permanent", "artifact"]
        .iter()
        .filter(|kind| head.contains(*kind))
        .map(|kind| text.replacen(" its controller", &format!(" that {kind}'s controller"), 1))
        .collect()
}

/// A damage source the rule does not narrow to a kind is any source (a spell as well as a permanent).
fn any_source(text: &str, when: &TriggerPattern) -> Vec<String> {
    let (pattern, batch) = match when {
        TriggerPattern::Batch(inner) => (inner.as_ref(), true),
        other => (other, false),
    };
    let TriggerPattern::DealsDamage { source, to, .. } = pattern else {
        return Vec::new();
    };
    let source = serde_json::to_string(source).unwrap_or_default();
    let any_kind = [
        "[]",
        r#"[{"ControlledBy":"You"}]"#,
        r#"[{"ControlledBy":"Opponents"}]"#,
    ]
    .contains(&source.as_str());
    if !any_kind {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (written, printed) in [
        (
            "Whenever a permanent you control deals ",
            "Whenever a source you control deals ",
        ),
        (
            "Whenever a permanent an opponent controls deals ",
            "Whenever a source an opponent controls deals ",
        ),
        ("Whenever a permanent deals ", "Whenever a source deals "),
    ] {
        if let Some(rest) = text.strip_prefix(written) {
            out.push(format!("{printed}{rest}"));
        }
    }
    let to_you = to
        .as_ref()
        .is_some_and(|to| serde_json::to_string(to).is_ok_and(|to| to == r#""You""#));
    if batch
        && source == "[]"
        && to_you
        && let Some(rest) = text.strip_prefix("Whenever one or more permanents deal damage to you,")
    {
        out.push(format!("Whenever you're dealt damage,{rest}"));
    }
    out
}

/// Where the clause after a point ends: a new sentence or "where X is".
fn clause_end(after: &str) -> usize {
    [after.find(". "), after.find(", where "), after.rfind('.')]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(after.len())
}

/// ", until end of turn, ~ gets +1/+1." as ", ~ gets +1/+1 until end of turn.".
fn until_after(text: &str) -> Option<String> {
    let at = text.find(", until end of turn, ")?;
    let after = &text[at + ", until end of turn, ".len()..];
    let end = clause_end(after);
    let clause = &after[..end];
    let changes = [
        " gets ", " gains ", " get ", " gain ", " has ", " have ", " loses ", " lose ",
    ]
    .iter()
    .any(|verb| clause.contains(verb));
    (changes
        && !clause.contains(", ")
        && !clause.contains("until")
        && !clause.contains("this turn"))
    .then(|| {
        format!(
            "{}, {clause} until end of turn{}",
            &text[..at],
            &after[end..]
        )
    })
}

/// "gets +1/+1 and gains flying" as "gains flying and gets +1/+1": both last as long as each other.
fn gains_first(text: &str) -> Option<String> {
    let (gets, gains) = [(" gets ", " and gains "), (" get ", " and gain ")]
        .into_iter()
        .find(|(gets, gains)| text.contains(gets) && text.contains(gains))?;
    let at = text.find(gets)?;
    let after = &text[at + gets.len()..];
    let middle = after.find(gains)?;
    let change = &after[..middle];
    if change.contains(' ') || change.contains(',') {
        return None;
    }
    let rest = &after[middle + gains.len()..];
    let end = [
        rest.find(" until end of turn"),
        Some(clause_end(rest)),
        rest.find(','),
    ]
    .into_iter()
    .flatten()
    .min()
    .unwrap_or(rest.len());
    let keyword = &rest[..end];
    Some(format!(
        "{}{}{keyword} and {}{change}{}",
        &text[..at],
        gains.replacen(" and", "", 1),
        gets.trim_start(),
        &rest[end..]
    ))
}

/// "each creature you control gets +1/+1 and gains trample" as "creatures you control get +1/+1 and gain trample".
fn creatures_get(text: &str) -> Option<String> {
    let at = text.find("each creature you control gets ")?;
    let after = &text[at + "each creature you control gets ".len()..];
    let end = clause_end(after);
    let clause = after[..end].replacen(" and gains ", " and gain ", 1);
    Some(format!(
        "{}creatures you control get {clause}{}",
        &text[..at],
        &after[end..]
    ))
}
