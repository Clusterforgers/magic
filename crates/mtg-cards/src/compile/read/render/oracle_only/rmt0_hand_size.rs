//! A spell that ends by shrinking its caster's maximum hand size for good: "Draw three cards. Your maximum hand size is
//! reduced by three for the rest of the game."

use super::super::super::amounts::count_word;
use super::super::variants;
use super::twin_json::{inner_mut, twin};
use crate::ability::Ability;
use serde_json::{Value, json};

/// Adds the wordings of a last step that reduces the controller's maximum hand size for the rest of the game.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut by = None;
    let plain = twin(ability, |value| {
        let effects = inner_mut(value)?.get_mut("effects")?.as_array_mut()?;
        let last = effects.last()?;
        let change = last.pointer("/Rule/rule/HandSizeChange")?;
        let reduced = change
            .get("by")
            .and_then(Value::as_i64)
            .filter(|n| *n < 0)?;
        let whole = json!({"Rule": {"rule": {"HandSizeChange": {"who": "You", "by": reduced}}, "duration": "Forever"}});
        (effects.len() > 1 && *last == whole).then_some(())?;
        by = u32::try_from(-reduced).ok();
        effects.pop().map(|_| ())
    });
    let (Some(plain), Some(words)) = (plain, by.and_then(count_word)) else {
        return;
    };
    texts.extend(variants(&plain).iter().map(|text| {
        format!("{text} Your maximum hand size is reduced by {words} for the rest of the game.")
    }));
}

#[cfg(test)]
mod tests {
    use crate::ability::Ability;
    use crate::compile::written_variants;
    use rstest::rstest;

    fn spell(rule: &str) -> Ability {
        serde_json::from_str(&format!(
            r#"{{"Spell":{{"effects":[{{"Draw":{{"count":3}}}},{{"Rule":{{"rule":{rule},"duration":"Forever"}}}}]}}}}"#
        ))
        .unwrap()
    }

    #[test]
    fn a_hand_size_reduced_for_good_is_written_after_the_spell() {
        let written = written_variants(&spell(r#"{"HandSizeChange":{"who":"You","by":-3}}"#));
        let wanted = "Draw three cards. Your maximum hand size is reduced by three for the rest of the game.";
        assert!(written.contains(&wanted.to_owned()), "{written:?}");
    }

    #[rstest]
    #[case::grown(r#"{"HandSizeChange":{"who":"You","by":3}}"#)]
    #[case::each_player(r#"{"HandSizeChange":{"who":"EachPlayer","by":-3}}"#)]
    fn another_change_is_not_written_as_a_reduction(#[case] rule: &str) {
        assert!(
            !written_variants(&spell(rule))
                .iter()
                .any(|text| text.contains("Your maximum hand size is reduced"))
        );
    }
}
