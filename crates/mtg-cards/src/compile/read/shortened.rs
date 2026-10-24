//! Wordings the oracle shortens or varies and the data does not say: "investigate" for creating a Clue token ("Whenever ~
//! dies, you may investigate."), "it" for "that card" after an exile ("Exile target creature. Return it to the
//! battlefield ...") and the object whose characteristic an amount is ("You lose life equal to that card's mana value.").

/// The sentence of a Clue token, as the keyword action words it.
const CLUE: (&str, &str) = ("create a Clue token", "investigate");

/// The words before a characteristic of the target object that an amount equals.
const EQUAL_TO: [&str; 3] = [
    "life equal to its ",
    "damage equal to its ",
    "cards equal to its ",
];

/// How a card names the object whose characteristic an amount equals.
const OWNERS: [&str; 3] = ["that card's ", "that creature's ", "that permanent's "];

/// Adds to `texts` the shortened wordings of the same ability.
pub(super) fn add(texts: &mut Vec<String>) {
    let found: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            let mut found = vec![investigate(text), pronoun(text)];
            found.extend(owners(text));
            found
        })
        .flatten()
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// "you may investigate" for "you may create a Clue token".
fn investigate(text: &str) -> Option<String> {
    text.contains(CLUE.0).then(|| text.replace(CLUE.0, CLUE.1))
}

/// "Return it to the battlefield" for "Return that card to the battlefield" in the sentence after an exile.
fn pronoun(text: &str) -> Option<String> {
    let at = text.find(". Return that card to the battlefield")?;
    let (before, after) = text.split_at(at);
    Some(format!(
        "{before}{}",
        after.replacen("Return that card", "Return it", 1)
    ))
}

/// "life equal to that card's mana value" for "life equal to its mana value".
fn owners(text: &str) -> Vec<Option<String>> {
    EQUAL_TO
        .iter()
        .filter(|marker| text.contains(**marker))
        .flat_map(|marker| {
            OWNERS.iter().map(move |owner| {
                let named = marker.replace("its ", owner);
                Some(text.replacen(*marker, &named, 1))
            })
        })
        .collect()
}
