//! A number of objects or mana counted by "for each" where the renderers write "a number of ... equal to the number
//! of" or "X ..., where X is the number of": "For each nontoken creature destroyed this way, you create a tapped
//! Treasure token.", "create a 3/2 red and white Spirit creature token for each nontoken creature you controlled that
//! was destroyed this way", "Add {B} or {G} for each permanent destroyed this way."

/// The tokens a count creates, as the renderers word them.
const W4F_NUMBER: &str = "reate a number of ";
/// The count of the destroyed objects.
const W4F_DESTROYED: &str = " destroyed this way";
/// Mana of two colors in any combination, as the renderers word it.
const W4F_COMBINATION: &str = "X mana in any combination of ";

/// Adds the "for each" wordings of tokens and mana counted by a number of objects.
pub(super) fn add(texts: &mut Vec<String>) {
    let mut said: Vec<String> = Vec::new();
    for text in texts.iter() {
        let worded = w4f_tokens_for_each(text)
            .into_iter()
            .chain(w4f_mana_for_each(text));
        for worded in worded {
            if worded != *text && !said.contains(&worded) {
                said.push(worded);
            }
        }
    }
    texts.extend(said);
}

/// "create a number of P tokens equal to the number of Q destroyed this way" as "create a P token for each Q destroyed
/// this way" and "For each Q destroyed this way, create a P token".
fn w4f_tokens_for_each(text: &str) -> Vec<String> {
    let Some(at) = text.find(W4F_NUMBER).filter(|at| *at > 0) else {
        return Vec::new();
    };
    let (lead, verb) = (&text[..at - 1], &text[at - 1..at]);
    let rest = &text[at + W4F_NUMBER.len()..];
    let Some((made, counted)) = rest.split_once(" equal to the number of ") else {
        return Vec::new();
    };
    let Some((counted, tail)) = counted.split_once(W4F_DESTROYED) else {
        return Vec::new();
    };
    let Some(made) = made.strip_suffix(" tokens") else {
        return Vec::new();
    };
    let one = format!("{} {made} token", w4f_article(made));
    let each = format!("for each {}{W4F_DESTROYED}", w4f_singular(counted));
    let mut worded = vec![format!("{lead}{verb}reate {one} {each}{tail}")];
    let opens = lead.rfind(". ").map_or(0, |end| end + 2);
    let subject = match &lead[opens..] {
        "" => Some(""),
        "You " | "you " => Some("you "),
        _ => None,
    };
    if let Some(subject) = subject {
        worded.push(format!(
            "{}F{}, {subject}create {one}{tail}",
            &lead[..opens],
            &each[1..]
        ));
    }
    worded
}

/// "Add X mana in any combination of {B} and/or {G}, where X is the number of Q." as "Add {B} or {G} for each Q."
fn w4f_mana_for_each(text: &str) -> Option<String> {
    let at = text.find(W4F_COMBINATION)?;
    let rest = &text[at + W4F_COMBINATION.len()..];
    let (colors, counted) = rest.split_once(", where X is the number of ")?;
    let (first, second) = colors.split_once(" and/or ")?;
    let end = counted.find('.').unwrap_or(counted.len());
    Some(format!(
        "{}{first} or {second} for each {}{}",
        &text[..at],
        w4f_singular(&counted[..end]),
        &counted[end..]
    ))
}

/// The group in the singular: its first plural word and "were" ("nontoken creatures you controlled that were" is
/// "nontoken creature you controlled that was").
fn w4f_singular(group: &str) -> String {
    let mut done = false;
    group
        .split(' ')
        .map(|word| match word {
            "were" => "was".to_owned(),
            _ if !done && word.len() > 1 && word.ends_with('s') && !word.ends_with("ss") => {
                done = true;
                word[..word.len() - 1].to_owned()
            }
            _ => word.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The indefinite article before the words.
fn w4f_article(words: &str) -> &'static str {
    if words.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U', '8']) {
        "an"
    } else {
        "a"
    }
}
