//! A draw the player may skip: "If you would draw a card, you may skip that draw instead." (a "may" with no steps in
//! place of the draw).

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// Adds the wording of a draw replaced by a "may" with no steps.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when:
            ReplaceWhen::Draw {
                who: Ref::You,
                except_draw_step: 0,
            },
        by: ReplaceBy::Instead(effects),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return;
    };
    if let [
        Effect::May {
            who: Ref::You,
            effects,
        },
    ] = effects.as_slice()
        && effects.is_empty()
    {
        texts.push("If you would draw a card, you may skip that draw instead.".to_owned());
    }
}

#[cfg(test)]
mod tests {
    //! Goal: only a "may" with no steps in place of the controller's every draw reads as skipping that draw.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::skip(r#"{"Replacement":{"when":{"Draw":{"who":"You","except_draw_step":0}},"by":{"Instead":[{"May":{"effects":[]}}]}}}"#, true)]
    #[case::may_draw(r#"{"Replacement":{"when":{"Draw":{"who":"You","except_draw_step":0}},"by":{"Instead":[{"May":{"effects":[{"Draw":{}}]}}]}}}"#, false)]
    #[case::opponents(r#"{"Replacement":{"when":{"Draw":{"who":"Opponents","except_draw_step":0}},"by":{"Instead":[{"May":{"effects":[]}}]}}}"#, false)]
    #[case::not_first(r#"{"Replacement":{"when":{"Draw":{"who":"You","except_draw_step":1}},"by":{"Instead":[{"May":{"effects":[]}}]}}}"#, false)]
    fn a_skipped_draw_is_written(#[case] json: &str, #[case] written: bool) {
        let ability: Ability = serde_json::from_str(json).expect("ability");
        let mut texts = Vec::new();
        add(&ability, &mut texts);
        assert_eq!(!texts.is_empty(), written, "{texts:?}");
    }
}
