//! You and a target player draw together: "{T}: You and target opponent each draw a card." The renderers write the two
//! draws as "You draw a card and target opponent draws a card."

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wordings of a draw by you and by the target that name them together.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let together = matches!(
        ability.effects(),
        [
            Effect::Draw { who: Ref::You, count: a },
            Effect::Draw { who: Ref::Target(0), count: b },
        ] if a == b && a.is_one()
    );
    if !together {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("You draw a card and target "))
        .filter_map(|text| {
            let (head, tail) = text.split_once("You draw a card and target ")?;
            let (who, rest) = tail.split_once(" draws a card")?;
            Some(format!("{head}You and target {who} each draw a card{rest}"))
        })
        .collect();
    texts.extend(said);
}
