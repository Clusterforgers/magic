//! A draw that another player makes instead: "If an opponent would draw a card except the first one they draw in each
//! of their draw steps, instead that player skips that draw and you draw a card."

use crate::ability::Ability;
use crate::compile::read::amounts::count_word;
use crate::effect::Effect;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// The exception for the first `except` draws of each draw step, in each wording.
fn exceptions(except: u32) -> Vec<String> {
    match except {
        0 => vec![String::new()],
        1 => vec![" except the first one they draw in each of their draw steps".to_owned()],
        n => count_word(n)
            .map(|word| format!(" except the first {word} they draw in each of their draw steps"))
            .into_iter()
            .collect(),
    }
}

/// Adds the wording of the replacement of an opponent's draw by a card the controller draws.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when:
            ReplaceWhen::Draw {
                who: Ref::Opponents,
                except_draw_step,
            },
        by: ReplaceBy::Instead(effects),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return;
    };
    let [
        Effect::Draw {
            who: Ref::You,
            count,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    if !count.is_one() {
        return;
    }
    texts.extend(exceptions(*except_draw_step).into_iter().map(|exception| {
        format!(
            "If an opponent would draw a card{exception}, instead that player skips that draw and you draw a card."
        )
    }));
}
