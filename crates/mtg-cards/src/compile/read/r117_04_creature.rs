//! A rule of a permanent that holds only while the permanent itself is a creature (a bestowed Aura, an uncrewed Vehicle):
//! "As long as this permanent is a creature, it has prowess." and "{T}: Add one mana of any color. Activate only if this
//! permanent is a creature." (CR 702.103, 702.122): the sentences of the rule without the condition, and the condition.

use mtg_core::card::CardType;

use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector};
use crate::value::Condition;

/// Whether the condition is that the permanent itself is a creature.
fn itself_a_creature(condition: Option<&Condition>) -> bool {
    condition
        == Some(&Condition::Matches {
            subject: Ref::This,
            selector: Selector(vec![Pred::Type(CardType::Creature)]),
        })
}

/// Adds the wordings of a static of the permanent itself, or an activated ability, under that condition.
pub(super) fn r117_04_creature_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let mut plain = ability.clone();
    let found: Vec<String> = match &mut plain {
        Ability::Static {
            affected: Ref::This,
            condition,
            defining: false,
            ..
        } if itself_a_creature(condition.as_ref()) => {
            // A static of the permanent itself has no wording alone; the writers give "As long as C, ~ has prowess."
            const STAND_IN: &str = "As long as you have the city's blessing, ~ ";
            *condition = Some(Condition::CitysBlessing(Ref::You));
            written(&plain)
                .iter()
                .filter_map(|text| text.strip_prefix(STAND_IN))
                .map(|rest| format!("As long as ~ is a creature, it {rest}"))
                .collect()
        }
        Ability::Activated { condition, .. } if itself_a_creature(condition.as_ref()) => {
            *condition = None;
            written(&plain)
                .iter()
                .map(|text| match text.strip_suffix('.') {
                    // "Activate only once each turn and only if this permanent is a creature."
                    Some(head) if head.contains(". Activate only ") => {
                        format!("{head} and only if ~ is a creature.")
                    }
                    _ => format!("{text} Activate only if ~ is a creature."),
                })
                .collect()
        }
        _ => return,
    };
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a rule that holds only while the permanent itself is a creature is written with that condition, and the rule
    //! without it, under another condition or of another object is not.
    #![allow(clippy::unwrap_used)]
    use crate::ability::Ability;
    use crate::compile::read::render::written_variants;
    use crate::compile::same_text;
    use rstest::rstest;

    const PROWESS: &str = r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Prowess"}],"condition":{"Matches":{"subject":"This","selector":[{"Type":"Creature"}]}}}}"#;
    const MANA: &str = r#"{"Activated":{"cost":["Tap"],"condition":{"Matches":{"subject":"This","selector":[{"Type":"Creature"}]}},"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#;
    const PROWESS_TEXT: &str = "As long as ~ is a creature, it has prowess.";
    const MANA_TEXT: &str = "{T}: Add one mana of any color. Activate only if ~ is a creature.";

    fn writes(json: &str, text: &str) -> bool {
        let ability: Ability = serde_json::from_str(json).unwrap();
        written_variants(&ability)
            .iter()
            .any(|written| same_text(written, text))
    }

    #[rstest]
    #[case::a_keyword(PROWESS, PROWESS_TEXT)]
    #[case::a_mana_ability(MANA, MANA_TEXT)]
    #[case::once_each_turn(
        &MANA.replace(r#""effects""#, r#""limit":"OncePerTurn","effects""#),
        "{T}: Add one mana of any color. Activate only once each turn and only if ~ is a creature."
    )]
    fn r117_04_a_rule_while_a_creature_is_written(#[case] json: &str, #[case] text: &str) {
        assert!(writes(json, text), "{text}");
    }

    #[rstest]
    #[case::no_condition(&PROWESS.replace(r#","condition":{"Matches":{"subject":"This","selector":[{"Type":"Creature"}]}}"#, ""), PROWESS_TEXT)]
    #[case::an_artifact(&PROWESS.replace(r#"{"Type":"Creature"}"#, r#"{"Type":"Artifact"}"#), PROWESS_TEXT)]
    #[case::the_attached(&PROWESS.replace(r#""affected":"This""#, r#""affected":"Attached""#), PROWESS_TEXT)]
    #[case::mana_without(&MANA.replace(r#""condition":{"Matches":{"subject":"This","selector":[{"Type":"Creature"}]}},"#, ""), MANA_TEXT)]
    fn r117_04_another_rule_is_not_the_text(#[case] json: &str, #[case] text: &str) {
        assert!(!writes(json, text), "{text}");
    }
}
