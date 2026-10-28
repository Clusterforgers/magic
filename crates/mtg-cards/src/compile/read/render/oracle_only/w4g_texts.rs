//! Printed wordings that say what a written wording says in other words: a sentence that ends in a quoted ability
//! without a second period, "with flying, menace, and "..."", a token "named N with "..."", "a number of tokens equal to
//! Y" and "with mana value equal to Y" for "X, where X is Y", "each creature with a counter on it" for "all creatures with
//! counters on them", "all opponents' graveyards", "put a counter on it" after the enchanted creature attacks, "When you
//! sacrifice ~", "it deals" for a kicked spell and "they control" after "they".

use crate::ability::Ability;
use crate::trigger::TriggerPattern;

/// The same group in the plural with "all" and in the singular with "each".
const W4G_EACH: [(&str, &str); 4] = [
    (
        "all creatures with mana value equal to ",
        "each creature with mana value equal to ",
    ),
    (
        "all creatures you control with counters on them",
        "each creature you control with a counter on it",
    ),
    (
        "all creatures with counters on them",
        "each creature with a counter on it",
    ),
    ("each opponent's graveyard", "all opponents' graveyards"),
];

/// Adds to `texts` the other printed wordings of each wording.
pub(super) fn w4g_add_texts(ability: &Ability, texts: &mut Vec<String>) {
    w4g_extend(texts, w4g_quote_ends_sentence);
    w4g_extend(texts, w4g_keywords_before_quote);
    w4g_extend(texts, w4g_name_before_quote);
    w4g_extend(texts, w4g_where_x_equal_to);
    w4g_extend(texts, |text| {
        let found = W4G_EACH.iter().fold(text.to_owned(), |found, (all, each)| {
            found.replace(all, each)
        });
        (found != text).then_some(found)
    });
    match ability {
        Ability::Triggered { when, .. } => {
            if matches!(when, TriggerPattern::Attacks { .. }) {
                w4g_extend(texts, w4g_counter_on_it);
            }
            if matches!(when, TriggerPattern::Sacrificed { .. }) {
                w4g_extend(texts, |text| {
                    let rest = text.strip_prefix("Whenever you sacrifice ~, ")?;
                    Some(format!("When you sacrifice ~, {rest}"))
                });
            }
            w4g_extend(texts, w4g_they_control);
        }
        // "it" is the spell only where no later "its" could be read as the spell's own.
        Ability::Spell { .. } => w4g_extend(texts, |text| {
            let kicked = "If this spell was kicked, ~ deals ";
            let (head, rest) = text.split_at(text.find(kicked)?);
            (!rest.contains(" its ")).then(|| {
                format!(
                    "{head}If this spell was kicked, it deals {}",
                    &rest[kicked.len()..]
                )
            })
        }),
        _ => {}
    }
}

/// Adds the rewrite of each text that `rewrite` changes.
fn w4g_extend(texts: &mut Vec<String>, rewrite: impl Fn(&str) -> Option<String>) {
    let mut found: Vec<String> = Vec::new();
    for text in texts.iter() {
        if let Some(new) = rewrite(text)
            && !texts.contains(&new)
            && !found.contains(&new)
        {
            found.push(new);
        }
    }
    texts.extend(found);
}

/// `with "~ can't block.". If X` is printed `with "This token can't block." If X`: the quote's period ends the sentence.
fn w4g_quote_ends_sentence(text: &str) -> Option<String> {
    text.contains(".\". ")
        .then(|| text.replace(".\". ", ".\" "))
}

/// `with flying and menace and "..."` is printed `with flying, menace, and "..."`.
fn w4g_keywords_before_quote(text: &str) -> Option<String> {
    let quote = text.find(" and \"")?;
    let (head, rest) = text.split_at(quote);
    let with = head.rfind(" with ")?;
    let keywords = &head[with + " with ".len()..];
    if keywords.contains(',') || keywords.contains('"') || keywords.matches(" and ").count() != 1 {
        return None;
    }
    let listed = keywords.replacen(" and ", ", ", 1);
    let rest = &rest[" and".len()..];
    Some(format!("{} with {listed}, and{rest}", &head[..with]))
}

/// `tokens with "..." named Blue Horror.` is printed `tokens named Blue Horror with "..."`.
fn w4g_name_before_quote(text: &str) -> Option<String> {
    let close = text.find("\" named ")?;
    let open = text[..close].rfind(" with \"")?;
    let quote = &text[open + 1..=close];
    let after = &text[close + "\" named ".len()..];
    let end = after.find(['.', ',']).unwrap_or(after.len());
    let (name, mut rest) = after.split_at(end);
    if quote.ends_with(".\"") {
        rest = rest.strip_prefix('.').unwrap_or(rest);
    }
    Some(format!("{} named {name} {quote}{rest}", &text[..open]))
}

/// `with mana value X, where X is Y.` is printed `with mana value equal to Y.`, and `Create X <tokens>, where X is Y.` is
/// printed `Create a number of <tokens> equal to Y.`; only where the text names X nowhere else.
fn w4g_where_x_equal_to(text: &str) -> Option<String> {
    let at = text.find(", where X is ")?;
    let (head, tail) = text.split_at(at);
    let rest = &tail[", where X is ".len()..];
    let xs = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| *word == "X")
        .count();
    if xs != 2 {
        return None;
    }
    if let Some(head) = head.strip_suffix(" with mana value X") {
        return Some(format!("{head} with mana value equal to {rest}"));
    }
    let create = head.rfind("reate X ")?;
    let tokens = &head[create + "reate X ".len()..];
    if !tokens.ends_with(" tokens") || tokens.contains(['.', '"']) {
        return None;
    }
    Some(format!(
        "{}reate a number of {tokens} equal to {rest}",
        &head[..create]
    ))
}

/// After "Whenever enchanted creature attacks," the counter is put "on it".
fn w4g_counter_on_it(text: &str) -> Option<String> {
    ["enchanted creature", "equipped creature"]
        .iter()
        .find_map(|holder| {
            let rest = text.strip_prefix(&format!("Whenever {holder} attacks, "))?;
            let on = format!(" on {holder}.");
            let at = rest.find(&on)?;
            let head = &rest[..at];
            (head.ends_with(" counter") || head.ends_with(" counters")).then(|| {
                format!(
                    "Whenever {holder} attacks, {head} on it.{}",
                    &rest[at + on.len()..]
                )
            })
        })
}

/// "they lose 1 life and you put a counter on target creature that player controls": "they control" once "they" names
/// the player.
fn w4g_they_control(text: &str) -> Option<String> {
    let they = text.find(" they ")?;
    let (head, tail) = text.split_at(they);
    tail.contains(" that player controls").then(|| {
        format!(
            "{head}{}",
            tail.replace(" that player controls", " they control")
        )
    })
}
