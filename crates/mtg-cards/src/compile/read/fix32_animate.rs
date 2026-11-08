//! An artifact that becomes an artifact creature, said as the cards print it: "becomes an artifact creature with base
//! power and toughness 4/5.", "becomes a Shapeshifter artifact creature with base power and toughness 3/3 until end of
//! turn." and "becomes a 4/4 Angel artifact creature and gains flying.", with "Draw a card." as a line of its own. The
//! writers say these as "becomes a 4/5 artifact creature that's still a land", which an artifact never is.

use crate::ability::Ability;

const END: &str = " until end of turn";

/// Adds the wordings of an ability that makes a permanent an artifact creature.
pub(super) fn wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(json) = serde_json::to_string(ability) else {
        return;
    };
    if !json.contains(r#"{"AddType":"Artifact"}"#) || !json.contains(r#"{"AddType":"Creature"}"#) {
        return;
    }
    let mut more = Vec::new();
    // "... and loses flying": written as the keyword given, which the form of the card says after "and".
    if let Some(start) = json.find(r#"{"RemoveKeyword":""#)
        && let Some(end) = json[start..].find('}')
    {
        let removal = &json[start..=start + end];
        let given = removal.replace("RemoveKeyword", "AddKeyword");
        if let Ok(stand) = serde_json::from_str::<Ability>(&json.replace(removal, &given)) {
            for text in super::render::written_variants(&stand) {
                let cleaned = text
                    .replace(". It's still a land", "")
                    .replace(" that's still a land", "");
                more.extend(base_forms(&cleaned, "loses"));
            }
        }
    }
    for text in texts.iter() {
        let cleaned = text
            .replace(". Then it's still a land", "")
            .replace(". It's still a land", "")
            .replace(" that's still a land", "");
        if cleaned == *text || !cleaned.contains("artifact creature") {
            continue;
        }
        more.push(cleaned.clone());
        more.extend(base_forms(&cleaned, "gains"));
    }
    let lines: Vec<String> = more
        .iter()
        .filter(|text| text.contains(", then draw a card."))
        .map(|text| text.replace(", then draw a card.", ".\nDraw a card."))
        .collect();
    texts.extend(more);
    texts.extend(lines);
}

/// The sentence with its size said as "base power and toughness", or after the keywords as "and gains".
fn base_forms(text: &str, verb: &str) -> Vec<String> {
    let Some(at) = text.find(" becomes a") else {
        return Vec::new();
    };
    let (subject, rest) = text.split_at(at + " becomes ".len());
    let rest = rest.strip_prefix("an ").or_else(|| rest.strip_prefix("a "));
    let Some((size, rest)) = rest.and_then(|rest| rest.split_once(' ')) else {
        return Vec::new();
    };
    if !size.contains('/') || !size.chars().all(|c| c.is_ascii_digit() || c == '/') {
        return Vec::new();
    }
    let Some((noun, after)) = rest.split_once(" creature") else {
        return Vec::new();
    };
    let Some(kinds) = noun.strip_suffix("artifact").map(str::trim) else {
        return Vec::new();
    };
    if kinds
        .split(' ')
        .any(|word| word.starts_with(|c: char| c.is_ascii_lowercase()))
    {
        return Vec::new();
    }
    let (given, tail) = match after.strip_prefix(" with ") {
        Some(with) => {
            let end = [with.find(END), with.find('.'), with.find(", then")]
                .into_iter()
                .flatten()
                .min()
                .unwrap_or(with.len());
            (Some(&with[..end]), &with[end..])
        }
        None => (None, after),
    };
    let kind = if kinds.is_empty() {
        String::new()
    } else {
        format!("{kinds} ")
    };
    let mut found = Vec::new();
    match given {
        None => {
            let article = if kinds.is_empty() { "an" } else { "a" };
            found.push(format!(
                "{subject}{article} {kind}artifact creature with base power and toughness {size}{tail}"
            ));
        }
        Some(given) if kinds.is_empty() => found.push(format!(
            "{subject}an artifact creature with base power and toughness {size} and {verb} {given}{tail}"
        )),
        Some(given) => found.push(format!(
            "{subject}a {size} {kind}artifact creature and {verb} {given}{tail}"
        )),
    }
    found
}
