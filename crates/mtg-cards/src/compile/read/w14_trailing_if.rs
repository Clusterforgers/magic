//! A condition said after its effect: "Exile target creature if it has mana value 3 or less." says what "If it has mana
//! value 3 or less, exile target creature." says, so the sentence that opens with its condition is also worded with the
//! condition last, and a step after ", then" is also worded as a sentence of its own ("Mill two cards. Then return a card.").

/// The most places a condition may end at in one sentence ("if you control a Cleric, Druid, or Wizard, ..." has commas).
const COMMAS: usize = 3;

/// Adds, for each text, the wording that puts the condition of each sentence opening with "If " last and the wordings
/// that open a sentence with "Then".
pub(super) fn add(texts: &mut Vec<String>) {
    let found: Vec<String> = texts
        .iter()
        .flat_map(|text| trailing(text).into_iter().chain(sentence_then(text)))
        .collect();
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The text with one ", then " as a sentence end and "Then": "Mill two cards, then return a card." as "Mill two cards. Then return a card.".
fn sentence_then(text: &str) -> Vec<String> {
    if text.contains('"') {
        return Vec::new();
    }
    text.match_indices(", then ")
        .take(COMMAS)
        .map(|(at, _)| format!("{}. Then {}", &text[..at], &text[at + ", then ".len()..]))
        .collect()
}

/// The text with one conditional sentence reworded: "If C, E." as "E if C.".
fn trailing(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    if text.contains('"') || text.contains("Otherwise") || text.contains(" instead") {
        return out;
    }
    let mut starts = vec![0];
    starts.extend(
        text.match_indices(['.', ':'])
            .filter(|(at, _)| text[at + 1..].starts_with(' '))
            .map(|(at, _)| at + 2),
    );
    for start in starts {
        let Some(sentence) = text[start..].strip_prefix("If ") else {
            continue;
        };
        let end = sentence.find(". ").unwrap_or(sentence.len() - 1);
        let Some(body) = sentence.get(..end) else {
            continue;
        };
        if !sentence[end..].starts_with('.') {
            continue;
        }
        for (at, _) in body.match_indices(", ").take(COMMAS) {
            let (condition, effect) = (&body[..at], &body[at + 2..]);
            let blocked = effect.is_empty()
                || ["instead", "otherwise", "then "]
                    .iter()
                    .any(|word| effect.starts_with(word) || condition.contains(word));
            if blocked {
                continue;
            }
            let mut chars = effect.chars();
            let Some(first) = chars.next() else {
                continue;
            };
            out.push(format!(
                "{}{}{} if {condition}{}",
                &text[..start],
                first.to_uppercase(),
                chars.as_str(),
                &sentence[end..],
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    //! Goal: a sentence opening with its condition is also worded with the condition last, and only a plain "If C, E."
    //! is: a condition that holds an "instead", an "otherwise" or a quoted ability stays as it was.
    use rstest::rstest;

    use super::{sentence_then, trailing};

    #[rstest]
    #[case::alone("If it has mana value 3 or less, exile target creature.", &["Exile target creature if it has mana value 3 or less."])]
    #[case::second_sentence(
        "Draw a card. If you control a Hero, you gain 2 life.",
        &["Draw a card. You gain 2 life if you control a Hero."]
    )]
    #[case::after_the_cost(
        "{T}: If you attacked, draw a card.",
        &["{T}: Draw a card if you attacked."]
    )]
    #[case::commas_in_the_condition(
        "If you control a Cleric, Druid, or Wizard, copy it.",
        &["Copy it if you control a Cleric, Druid, or Wizard."]
    )]
    #[case::instead("If C, instead draw a card.", &[])]
    #[case::then("If C, then draw a card.", &[])]
    #[case::otherwise_later("If C, draw a card. Otherwise, gain 2 life.", &[])]
    #[case::quoted("If C, create a token with \"If D, draw.\"", &[])]
    #[case::no_condition("Draw a card.", &[])]
    fn the_condition_moves_last(#[case] text: &str, #[case] wanted: &[&str]) {
        let found = trailing(text);
        for wording in wanted {
            assert!(found.contains(&(*wording).to_owned()), "{text}: {found:?}");
        }
        if wanted.is_empty() {
            assert!(found.is_empty(), "{text}: {found:?}");
        }
    }

    #[rstest]
    #[case::mill("Mill two cards, then return a card.", &["Mill two cards. Then return a card."])]
    #[case::second("A, then B, then C.", &["A. Then B, then C.", "A, then B. Then C."])]
    #[case::none("Mill two cards and return a card.", &[])]
    #[case::quoted("Create a token with \"A, then B.\"", &[])]
    fn a_then_opens_a_sentence(#[case] text: &str, #[case] wanted: &[&str]) {
        assert_eq!(sentence_then(text), wanted);
    }
}
