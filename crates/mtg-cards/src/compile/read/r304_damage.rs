//! Damage replaced by steps that read it (`ReplaceBy::Instead`, `ReplaceBy::PreventAnd`): "If damage would be dealt to
//! a player, that player exiles that many cards from the top of their library instead.", "If combat damage would be
//! dealt to ~, prevent that damage and exile that many cards from the top of your library."

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::ReplaceBy;
use crate::value::Amount;

/// The sentences of the replacement after each of its `heads` ("if a source would deal damage to ~"), without the
/// capital and closing period; empty for steps no writer reads.
pub(super) fn r304_damage_texts(
    heads: &[&String],
    by: &ReplaceBy,
    steps: &[Effect],
) -> Vec<String> {
    // Only steps that read the damage (its amount, recipient or source) are written here.
    let reads_it = serde_json::to_string(steps)
        .is_ok_and(|json| json.contains("\"Triggering\"") || json.contains("\"Dealer\""));
    if !reads_it {
        return Vec::new();
    }
    let mut found = Vec::new();
    for head in heads {
        match by {
            ReplaceBy::Instead(_) => {
                for said in steps_texts(steps) {
                    found.push(format!("{head}, {said} instead"));
                    found.push(format!("{head}, instead {said}"));
                }
            }
            ReplaceBy::PreventAnd(_) => {
                for said in steps_texts(steps) {
                    found.push(format!("{head}, prevent that damage and {said}"));
                    found.push(format!(
                        "{head}, prevent that damage. {}",
                        capitalized(&said)
                    ));
                }
                for said in prevented_texts(steps) {
                    found.push(format!(
                        "{head}, prevent that damage. {}",
                        capitalized(&said)
                    ));
                }
            }
            _ => {}
        }
    }
    found
}

/// The steps as one sentence says them, lowercase and without the period.
fn steps_texts(steps: &[Effect]) -> Vec<String> {
    if let [step] = steps
        && let Some(said) = step_text(step)
    {
        return vec![said.to_owned()];
    }
    super::render::variants(&Ability::Spell {
        targets: Vec::new(),
        effects: steps.to_vec(),
    })
    .into_iter()
    .filter_map(|text| {
        let text = text.strip_suffix('.')?;
        let mut chars = text.chars();
        let first = chars.next()?;
        Some(first.to_lowercase().chain(chars).collect())
    })
    .collect()
}

/// The steps the spell writers word otherwise: cards exiled from the top of a library, as many as the damage.
fn step_text(step: &Effect) -> Option<&'static str> {
    match step {
        Effect::ExileTop {
            who: Ref::You,
            count: Amount::Triggering,
            sealed: false,
            bottom: false,
            ..
        } => Some("exile that many cards from the top of your library"),
        Effect::ExileTop {
            who: Ref::Triggering,
            count: Amount::Triggering,
            sealed: false,
            bottom: false,
            ..
        } => Some("that player exiles that many cards from the top of their library"),
        _ => None,
    }
}

/// The steps after a prevention that name the damage prevented: "The source's controller draws cards equal to the
/// damage prevented this way."
fn prevented_texts(steps: &[Effect]) -> Vec<String> {
    match steps {
        [
            Effect::Draw {
                who: Ref::ControllerOf(whose),
                count: Amount::Triggering,
                bind: None,
            },
        ] if **whose == Ref::Dealer => {
            vec![
                "the source's controller draws cards equal to the damage prevented this way"
                    .to_owned(),
            ]
        }
        _ => Vec::new(),
    }
}

/// The sentence with a capital.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests {
    //! Goal: damage replaced by steps reads back as the printed sentence only when the steps say exactly that many cards
    //! (or the damage prevented) and the replacement is of the kind the sentence names.
    use super::*;
    use rstest::rstest;

    const EXILE_THEIRS: &str =
        r#"[{"ExileTop":{"who":"Triggering","count":"Triggering","as":"x"}}]"#;
    const EXILE_MINE: &str = r#"[{"ExileTop":{"count":"Triggering","as":"x"}}]"#;
    const EXILE_ONE: &str = r#"[{"ExileTop":{"count":1,"as":"x"}}]"#;
    const SWANS: &str = r#"[{"Draw":{"who":{"ControllerOf":"Dealer"},"count":"Triggering"}}]"#;
    const SWANS_YOU: &str = r#"[{"Draw":{"count":"Triggering"}}]"#;

    #[rstest]
    #[case::instead_theirs(
        true,
        EXILE_THEIRS,
        "if damage would be dealt to a player, that player exiles that many cards from the top of their library instead",
        true
    )]
    #[case::prevented_theirs(
        false,
        EXILE_THEIRS,
        "if damage would be dealt to a player, that player exiles that many cards from the top of their library instead",
        false
    )]
    #[case::prevent_mine(
        false,
        EXILE_MINE,
        "if damage would be dealt to a player, prevent that damage and exile that many cards from the top of your library",
        true
    )]
    #[case::prevent_one(
        false,
        EXILE_ONE,
        "if damage would be dealt to a player, prevent that damage and exile that many cards from the top of your library",
        false
    )]
    #[case::swans(
        false,
        SWANS,
        "if damage would be dealt to a player, prevent that damage. The source's controller draws cards equal to the damage prevented this way",
        true
    )]
    #[case::swans_you(
        false,
        SWANS_YOU,
        "if damage would be dealt to a player, prevent that damage. The source's controller draws cards equal to the damage prevented this way",
        false
    )]
    fn the_steps_read_after_the_damage(
        #[case] instead: bool,
        #[case] steps: &str,
        #[case] printed: &str,
        #[case] written: bool,
    ) {
        let steps: Vec<Effect> = serde_json::from_str(steps).expect("steps");
        let by = if instead {
            ReplaceBy::Instead(steps.clone())
        } else {
            ReplaceBy::PreventAnd(steps.clone())
        };
        let head = "if damage would be dealt to a player".to_owned();
        let texts = r304_damage_texts(&[&head], &by, &steps);
        assert_eq!(texts.iter().any(|t| t == printed), written, "{texts:?}");
    }
}
