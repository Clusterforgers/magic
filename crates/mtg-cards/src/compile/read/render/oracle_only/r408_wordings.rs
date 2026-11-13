//! Wordings of the r40-8 pack that print a list or a cost change of one kind of cast: "If damage would be dealt to ~,
//! prevent that damage, remove that many +1/+1 counters from ~, and create that many 1/1 colorless Spirit creature
//! tokens." (two steps after a prevention, as one list) and "Flashback costs you pay cost {2} less." (the spells a
//! player casts with flashback).

use crate::ability::{Ability, CostChange};
use crate::refs::{Pred, Ref};
use crate::trigger::ReplaceBy;
use crate::value::CastWay;

/// The head every prevention with steps is written with.
const PREVENTED: &str = "If damage would be dealt to ~, prevent that damage and ";

/// Adds the list wording of a prevention followed by two steps, and the flashback cost change.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let found = match ability {
        Ability::Replacement {
            by: ReplaceBy::PreventAnd(steps),
            ..
        } if steps.len() == 2 => listed(texts),
        Ability::CostModifier { .. } => flashback(ability).into_iter().collect(),
        _ => Vec::new(),
    };
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "prevent that damage and A. B." as "prevent that damage, A, and b.".
fn listed(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter_map(|text| {
            let rest = text.strip_prefix(PREVENTED)?.strip_suffix('.')?;
            let (first, second) = rest.split_once(". ")?;
            (!first.contains(". ") && !second.contains(". ")).then_some(())?;
            let mut letters = second.chars();
            let head = letters.next()?.to_lowercase().collect::<String>();
            Some(format!(
                "If damage would be dealt to ~, prevent that damage, {first}, and {head}{}.",
                letters.as_str()
            ))
        })
        .collect()
}

/// "Flashback costs you pay cost {2} less.", "Flashback costs your opponents pay cost {2} more.": a cost change of the
/// spells one group of players casts with flashback, with no other condition.
fn flashback(ability: &Ability) -> Option<String> {
    let Ability::CostModifier {
        applies_to,
        change,
        mana,
        per,
        condition: None,
        activating: None,
        targeting: None,
        least: 0,
        life: 0,
    } = ability
    else {
        return None;
    };
    per.is_one().then_some(())?;
    let [
        Pred::Spell,
        Pred::ControlledBy(who),
        Pred::CastWith(CastWay::Flashback),
    ] = applies_to.0.as_slice()
    else {
        return None;
    };
    let payer = match who {
        Ref::You => "you",
        Ref::Opponents => "your opponents",
        _ => return None,
    };
    let more = match change {
        CostChange::Reduce => "less",
        CostChange::Increase => "more",
    };
    Some(format!("Flashback costs {payer} pay cost {mana} {more}."))
}

#[cfg(test)]
mod tests {
    //! Goal: the list wording is made only from a prevention's two-sentence wording, and the flashback wording only for
    //! a cost change of the spells cast with flashback by you or your opponents.
    use super::{add, listed};
    use crate::ability::Ability;
    use rstest::rstest;

    #[test]
    fn r408_two_steps_become_one_list() {
        let texts = vec![
            "If damage would be dealt to ~, prevent that damage and remove that many +1/+1 counters from ~. Create that many 1/1 colorless Spirit creature tokens.".to_owned(),
        ];
        assert_eq!(
            listed(&texts),
            [
                "If damage would be dealt to ~, prevent that damage, remove that many +1/+1 counters from ~, and create that many 1/1 colorless Spirit creature tokens."
            ]
        );
        assert_eq!(
            listed(&[
                "If damage would be dealt to ~, prevent that damage and draw a card.".to_owned()
            ])
            .len(),
            0
        );
    }

    #[rstest]
    #[case::yours(r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"You"},{"CastWith":"Flashback"}],"change":"Reduce","mana":"{2}"}}"#, Some("Flashback costs you pay cost {2} less."))]
    #[case::theirs(r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"Opponents"},{"CastWith":"Flashback"}],"change":"Increase","mana":"{2}"}}"#, Some("Flashback costs your opponents pay cost {2} more."))]
    #[case::another_way(r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"You"},{"CastWith":"Escape"}],"change":"Reduce","mana":"{2}"}}"#, None)]
    #[case::every_spell(r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"You"}],"change":"Reduce","mana":"{2}"}}"#, None)]
    fn r408_flashback_costs(#[case] json: &str, #[case] expected: Option<&str>) {
        let ability: Ability = serde_json::from_str(json).unwrap();
        let mut texts = Vec::new();
        add(&ability, &mut texts);
        assert_eq!(texts.first().map(String::as_str), expected);
    }
}
