//! r47-01 wordings written from a twin another writer words: "When ~ enters, each opponent can't cast instant or sorcery
//! spells during that player's next turn." (Azor, the Lawbringer), a prohibition each opponent gets for their own next
//! turn, written from the twin whose one step draws a card.

use serde_json::json;

use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule, Prohibited};
use crate::refs::{Pred, Ref};

/// Adds the r47-01 wordings of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    r4701_their_next_turn(ability, texts);
}

/// "each opponent can't cast instant or sorcery spells during that player's next turn" for the one step of a triggered
/// ability: a pass over the opponents that bans the spells for that opponent's next turn.
fn r4701_their_next_turn(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let [
        Effect::RepeatEach {
            over: Ref::Opponents,
            bind,
            effects: inner,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [
        Effect::Rule {
            rule:
                GameRule::Cant {
                    who: Ref::Bound(who),
                    action:
                        Prohibited::Cast {
                            filter,
                            more_than: None,
                            unless: None,
                        },
                },
            duration: Duration::TheirNextTurn,
        },
    ] = inner.as_slice()
    else {
        return;
    };
    if who != bind {
        return;
    }
    // "instant or sorcery spells": a choice of card types, joined by "or".
    let [Pred::AnyOf(kinds)] = filter.0.as_slice() else {
        return;
    };
    let Some(words) = kinds
        .iter()
        .map(|kind| match kind {
            Pred::Type(card_type) => {
                Some(crate::compile::read::selectors::type_name(*card_type).to_owned())
            }
            _ => None,
        })
        .collect::<Option<Vec<String>>>()
    else {
        return;
    };
    let spells = format!("{} spells", words.join(" or "));
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(steps) = value
        .get_mut("Triggered")
        .and_then(|body| body.get_mut("effects"))
    else {
        return;
    };
    *steps = json!([{"Draw": {}}]);
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    let step = format!("each opponent can't cast {spells} during that player's next turn");
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.matches("draw a card").count() == 1)
        .map(|text| text.replacen("draw a card", &step, 1))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}
