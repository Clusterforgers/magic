//! A card that is a person calls itself "he" or "she" where the renderers write "it" and "~": "put a +1/+1 counter on
//! him. He gains indestructible until end of turn", "~ enters with a +1/+1 counter on him for each time he was kicked".

/// The words after which "it" is an object ("on it", "to it").
const W4F_PREPOSITIONS: [&str; 9] = [
    "on", "to", "from", "with", "of", "at", "onto", "into", "for",
];

/// The words after "it" that make it a subject although they do not end in "s" ("it can't block").
const W4F_AUXILIARIES: [&str; 6] = ["can't", "can", "would", "may", "must", "will"];

/// The pronouns of each gender: subject, object, possessive and reflexive.
const W4F_GENDERS: [[&str; 4]; 2] = [
    ["he", "him", "his", "himself"],
    ["she", "her", "her", "herself"],
];

/// Adds each wording of an ability that names its card with the pronouns of a person, in both genders.
pub(super) fn add(texts: &mut Vec<String>) {
    let mut said: Vec<String> = Vec::new();
    for text in texts.iter().filter(|text| text.contains('~')) {
        for pronouns in W4F_GENDERS {
            let worded = w4f_gendered(text, pronouns);
            if worded != *text && !said.contains(&worded) {
                said.push(worded);
            }
        }
    }
    texts.extend(said);
}

/// The text with "it", "its", "itself" and each "~" after the first that follows a preposition in the given pronouns.
fn w4f_gendered(text: &str, [subject, object, possessive, reflexive]: [&str; 4]) -> String {
    let words: Vec<&str> = text.split(' ').collect();
    let mut named = false;
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for (at, word) in words.iter().enumerate() {
        let core = word.trim_end_matches(['.', ',', ':', ';', '"']);
        let tail = &word[core.len()..];
        let after_preposition = at
            .checked_sub(1)
            .and_then(|before| words.get(before))
            .is_some_and(|before| W4F_PREPOSITIONS.contains(before));
        let next = words.get(at + 1).copied();
        let subject_here = !after_preposition
            && tail.is_empty()
            && next.is_some_and(|next| next.ends_with('s') || W4F_AUXILIARIES.contains(&next));
        let replaced = match core {
            "it" if subject_here => Some(subject.to_owned()),
            "It" if subject_here => Some(w4f_capital(subject)),
            "it" => Some(object.to_owned()),
            "its" => Some(possessive.to_owned()),
            "Its" => Some(w4f_capital(possessive)),
            "itself" => Some(reflexive.to_owned()),
            "~" if named && after_preposition => Some(object.to_owned()),
            _ => None,
        };
        named |= core.starts_with('~');
        out.push(replaced.map_or_else(|| (*word).to_owned(), |pronoun| pronoun + tail));
    }
    out.join(" ")
}

/// The word with a capital first letter.
fn w4f_capital(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
