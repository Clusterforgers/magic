//! Oracle wordings of pack l291 that join or reword sentences the renderers already write: "exile it and each player
//! returns ..." for "exile it. Each player returns ...", "their next turn" for the turn of a target opponent, and "it"
//! for enchanted creature named again in the sentence after the one that taps it.

use crate::ability::Ability;

/// A sentence break or phrase the writer prints and the words a card says it with.
const JOINS: [(&str, &str); 2] = [
    (". Each player returns ", " and each player returns "),
    (
        "control target opponent during that player's next turn",
        "control target opponent during their next turn",
    ),
];

/// The sentence that names enchanted creature, after which the next sentence calls it "it".
const TAPPED: &str = "tap enchanted creature. ";

/// The object named again at the end of that next sentence, and the pronoun for it.
const AGAIN: (&str, &str) = (" on enchanted creature.", " on it.");

/// Adds the wordings of `texts` with one phrase of [`JOINS`] said the card's way, and "it" for enchanted creature
/// named again at the end of the sentence after [`TAPPED`].
pub(super) fn add(_ability: &Ability, texts: &mut Vec<String>) {
    let mut more: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            JOINS
                .iter()
                .filter(|(written, _)| text.matches(written).count() == 1)
                .map(|(written, joined)| text.replacen(written, joined, 1))
        })
        .collect();
    more.extend(texts.iter().filter_map(|text| it_again(text)));
    for text in more {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "tap enchanted creature. If ..., put three stun counters on it." for the writer's "... on enchanted creature.": the
/// second sentence ends the text and names nothing else.
fn it_again(text: &str) -> Option<String> {
    let (head, next) = text.split_once(TAPPED)?;
    let body = next.strip_suffix(AGAIN.0)?;
    (!body.contains(". ") && !body.contains("enchanted") && !body.contains(" it"))
        .then(|| format!("{head}{TAPPED}{body}{}", AGAIN.1))
}

#[cfg(test)]
mod tests {
    //! Goal: the l291 wordings rewrite only the shapes they name: a joined sentence, their next turn, and "it" for
    //! enchanted creature named again right after it is tapped; another object or a third sentence keeps its words.
    use super::it_again;
    use rstest::rstest;

    #[rstest]
    #[case::kitnap(
        "When ~ enters, tap enchanted creature. If the gift wasn't promised, put three stun counters on enchanted creature.",
        Some(
            "When ~ enters, tap enchanted creature. If the gift wasn't promised, put three stun counters on it."
        )
    )]
    #[case::another_object(
        "When ~ enters, tap enchanted creature. Put a +1/+1 counter on target creature.",
        None
    )]
    #[case::a_third_sentence(
        "When ~ enters, tap enchanted creature. Draw a card. Put a stun counter on enchanted creature.",
        None
    )]
    #[case::not_tapped("When ~ enters, put a stun counter on enchanted creature.", None)]
    fn l291_it_names_enchanted_creature_again(#[case] text: &str, #[case] said: Option<&str>) {
        assert_eq!(it_again(text).as_deref(), said);
    }
}
