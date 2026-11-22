//! r116-01 abilities whose wordings no other reader or writer joins (the one-offs of the Star Trek cards): each
//! entry of `r116_01_wholes.json` is an ability, or a list of two that one sentence holds, with the words the card prints for
//! it (the card's name as `~`), read when a clause says exactly those words and written back from the abilities.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::ZoneKind;
use crate::trigger::TriggerPattern;

/// The entries, as JSON: a list of `[ability, wording]` pairs; a list of two abilities is one sentence that holds both.
const R116_01_WHOLES: &str = include_str!("r116_01_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static R116_01_SHAPES: LazyLock<Vec<(Vec<Ability>, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R116_01_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| {
            let shape = match shape {
                serde_json::Value::Array(list) => list
                    .into_iter()
                    .map(serde_json::from_value)
                    .collect::<Result<Vec<Ability>, _>>()
                    .ok()?,
                one => vec![serde_json::from_value(one).ok()?],
            };
            Some((shape, text))
        })
        .collect()
});

/// The openings of a Rulebreaker sentence (an ability word): what a deck with the card as its commander may hold.
const R116_RULEBREAKER_OPENINGS: [&str; 2] =
    ["A deck with this commander ", "If ~ is your commander, "];

/// Reads a Rulebreaker deck rule: one sentence that opens with "A deck with this commander" or "If ~ is your commander,".
fn r116_rulebreaker_read(text: &str) -> Option<Ability> {
    let text = text.trim();
    let sentence = text
        .strip_prefix("Rulebreaker \u{2014} ")
        .unwrap_or(text)
        .strip_suffix('.')?;
    let opens = R116_RULEBREAKER_OPENINGS
        .iter()
        .any(|opening| sentence.starts_with(opening) && sentence.len() > opening.len());
    (opens && !sentence.contains(['.', '"'])).then(|| Ability::Rule {
        rule: GameRule::R116Rulebreaker(sentence.to_owned()),
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// Reads a clause that is the wording of one ability of the table, or a Rulebreaker deck rule.
pub(super) fn r116_01_whole_read(text: &str) -> Option<Ability> {
    if let Some(rule) = r116_rulebreaker_read(text) {
        return Some(rule);
    }
    R116_01_SHAPES
        .iter()
        .find_map(|(shape, wording)| match shape.as_slice() {
            [one] if same_text(wording, text) => Some(one.clone()),
            _ => None,
        })
}

/// Adds the wording of an ability that one of the entries matches whole, a Saga chapter (CR 714.2) as the sentence of
/// its spell.
pub(super) fn r116_01_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    let spell = match ability {
        Ability::Triggered {
            when: TriggerPattern::Chapter { .. },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets,
            effects,
        } => Some(Ability::Spell {
            targets: targets.clone(),
            effects: effects.clone(),
        }),
        _ => None,
    };
    if let Ability::Rule {
        rule: GameRule::R116Rulebreaker(sentence),
        condition: None,
        ..
    } = ability
    {
        texts.push(format!("{sentence}."));
    }
    for (shape, text) in R116_01_SHAPES.iter() {
        if matches!(shape.as_slice(), [one] if one == ability || spell.as_ref() == Some(one))
            && !texts.contains(text)
        {
            texts.push(text.clone());
        }
    }
}

/// Reads a sentence of the table that holds two abilities.
pub(super) fn r116_01_pair_read(text: &str) -> Option<Vec<Ability>> {
    R116_01_SHAPES
        .iter()
        .find(|(shape, wording)| shape.len() == 2 && same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// The wordings of the table for two abilities in one sentence, in that order.
pub(super) fn r116_01_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    R116_01_SHAPES
        .iter()
        .filter(|(shape, _)| matches!(shape.as_slice(), [a, b] if a == first && b == second))
        .map(|(_, wording)| wording.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, the same ability with
    //! any one number changed is not written as that wording, and a near miss of the words reads as nothing.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::compile::read::render::written_variants;

    #[test]
    fn r116_01_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R116_01_WHOLES).unwrap();
        assert_eq!(R116_01_SHAPES.len(), entries.len());
        for (shape, text) in R116_01_SHAPES.iter() {
            match shape.as_slice() {
                [one] => {
                    assert_eq!(r116_01_whole_read(text).as_ref(), Some(one), "{text}");
                    assert!(written_variants(one).contains(text), "{text}");
                }
                [first, second] => {
                    assert_eq!(r116_01_pair_read(text).as_ref(), Some(shape), "{text}");
                    assert_eq!(
                        r116_01_pair_texts(first, second),
                        vec![text.clone()],
                        "{text}"
                    );
                    assert!(r116_01_pair_texts(second, first).is_empty(), "{text}");
                }
                _ => panic!("an entry holds one or two abilities: {text}"),
            }
        }
    }

    #[test]
    fn r116_01_a_number_changed_is_not_written_whole() {
        for (shape, text) in R116_01_SHAPES.iter() {
            let [one] = shape.as_slice() else {
                continue;
            };
            let json = serde_json::to_string(one).unwrap();
            for (at, digit) in json.char_indices().filter(|(_, c)| c.is_ascii_digit()) {
                let raised = char::from_digit((digit.to_digit(10).unwrap() + 1) % 10, 10).unwrap();
                let changed = format!("{}{raised}{}", &json[..at], &json[at + 1..]);
                if let Ok(near) = serde_json::from_str::<Ability>(&changed) {
                    assert!(!written_variants(&near).contains(text), "{changed}");
                }
            }
        }
    }

    #[rstest::rstest]
    #[case("A deck with this commander can have any land cards.")]
    #[case("A deck with this commander has no maximum deck size.")]
    #[case(
        "If ~ is your commander, the color identity of instant and sorcery cards in your deck can include one color of your choice not in your commander's color identity, and your deck can have any basic land cards."
    )]
    fn r116_rulebreaker_reads_and_writes_back(#[case] text: &str) {
        let rule = r116_01_whole_read(text).unwrap();
        assert!(matches!(
            &rule,
            Ability::Rule {
                rule: GameRule::R116Rulebreaker(_),
                ..
            }
        ));
        assert!(
            written_variants(&rule)
                .iter()
                .any(|written| written == text),
            "{text}"
        );
    }

    #[rstest::rstest]
    #[case("A deck with this commander.")]
    #[case("A deck can have any number of cards named ~.")]
    #[case("A deck with this commander can have any land cards")]
    #[case("A deck with this commander can have any land cards. Draw a card.")]
    #[case("If ~ is your commander, draw a card")]
    fn r116_a_near_miss_reads_as_no_rulebreaker(#[case] text: &str) {
        assert_eq!(r116_01_whole_read(text), None, "{text}");
    }
}
