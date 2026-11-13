//! Whole abilities of card pack r55-02 whose steps the renderers do not join: a Case solved by the number of sources
//! that dealt damage this turn, and its solved ability that exiles cards and lets its controller play one of them. Each
//! is matched whole as data and worded as the card prints it, so another count, condition or duration never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording.
const R5502_WHOLES: [(&str, &str); 2] = [
    // case_of_the_burning_masks
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"if":{"All":[{"Not":{"Matches":{"subject":"This","selector":["Solved"]}}},{"Compare":{"left":{"SourcesDealtDamage":"You"},"op":"Ge","right":3}}]},"effects":[{"Solve":{"what":"This"}}]}}"#,
        "To solve \u{2014} Three or more sources you controlled dealt damage this turn.",
    ),
    // case_of_the_burning_masks
    (
        r#"{"Activated":{"cost":["SacrificeSelf"],"condition":{"Matches":{"subject":"This","selector":["Solved"]}},"effects":[{"ExileTop":{"count":3,"as":"exiled"}},{"Choose":{"from":{"Bound":"exiled"},"as":"chosen"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"chosen"},"play":true}},"duration":"UntilEndOfTurn"}}]}}"#,
        "Solved \u{2014} Sacrifice this Case: Exile the top three cards of your library. Choose one of them. You may play that card this turn.",
    ),
];

/// The parsed entries; one that does not parse is dropped (a test holds that none is).
static R5502_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    R5502_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r5502_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R5502_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r55-02 table parses and reads back as its wording, and the same ability with any one
    //! number changed reads back as something else.
    #![allow(clippy::unwrap_used)]
    use super::{R5502_SHAPES, R5502_WHOLES};
    use crate::ability::Ability;
    use crate::compile::read::render::written_variants;

    #[test]
    fn r5502_every_whole_parses_and_reads_back() {
        assert_eq!(R5502_SHAPES.len(), R5502_WHOLES.len());
        for (shape, text) in R5502_SHAPES.iter() {
            assert!(
                written_variants(shape).iter().any(|said| said == text),
                "{text}"
            );
        }
    }

    #[test]
    fn r5502_a_changed_number_does_not_read_back() {
        for (json, text) in R5502_WHOLES {
            let shape: Ability = serde_json::from_str(json).unwrap();
            for (at, digit) in json.char_indices().filter(|(_, c)| c.is_ascii_digit()) {
                let raised = char::from_digit((digit.to_digit(10).unwrap() + 1) % 10, 10).unwrap();
                let changed = format!("{}{raised}{}", &json[..at], &json[at + 1..]);
                let Ok(miss) = serde_json::from_str::<Ability>(&changed) else {
                    continue;
                };
                if miss != shape {
                    assert!(
                        !written_variants(&miss).iter().any(|said| said == text),
                        "{text} read back for {miss:?}"
                    );
                }
            }
        }
    }
}
