//! r55-00 wordings of whole abilities: a cost permission that works once each turn on any turn ("Once each turn, you may
//! pay {0} rather than pay the mana cost for a spell you cast with mana value X or less, where X is the number of time
//! counters on this enchantment.").

use std::sync::LazyLock;

use crate::ability::Ability;
use crate::compile::read::selectors::{Form, phrase_text};
use crate::modify::GameRule;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, StatKind};

/// Whole abilities worded as the card prints them, matched as data (`r5500_wholes.json`: a list of `[ability, wording]`
/// pairs, the card's name as `~`), so another count, step or event in the same ability never reads back.
const R5500_WHOLES: &str = include_str!("r5500_wholes.json");

/// The entries parsed once.
static R5500_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R5500_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wordings of the r55-00 whole abilities.
pub(super) fn r5500_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R5500_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
    texts.extend(r5500_once_rather(ability));
    if let Ability::Rule {
        rule: GameRule::TaxAsLife(life),
        ..
    } = ability
        && crate::compile::read::r5500_reads::r5500_tax_as_life(*life) == *ability
    {
        texts.push(format!(
            "Rather than pay {{2}} for each previous time you've cast this spell from the command zone this game, pay \
             {life} life that many times."
        ));
    }
}

/// "Once each turn, you may pay <mana> rather than pay the mana cost for a <spell> you cast with mana value X or less,
/// where X is the number of <kind> counters on ~."
fn r5500_once_rather(ability: &Ability) -> Option<String> {
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        ..
    } = ability
    else {
        return None;
    };
    let (last, rest) = permission.what.0.split_last()?;
    let Pred::Compare {
        stat: StatKind::ManaValue,
        op: Cmp::Le,
        to: Amount::Counters {
            on: Ref::This,
            counter,
        },
    } = last
    else {
        return None;
    };
    let cost = permission.rather.clone()?;
    let what = Selector(rest.to_vec());
    let read =
        crate::compile::read::r5500_reads::r5500_once_rather(&what, cost.clone(), counter.clone());
    (read == *ability).then_some(())?;
    let [crate::cost::CostItem::Mana(mana)] = cost.0.as_slice() else {
        return None;
    };
    let mut preds = vec![Pred::Spell];
    preds.extend(rest.iter().cloned());
    let spell = phrase_text(&Selector(preds), Form::Subject)?;
    let article = if spell.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    let kind = crate::compile::read::counters::kind_text(counter)?;
    Some(format!(
        "Once each turn, you may pay {mana} rather than pay the mana cost for {article} {spell} you cast with mana value \
         X or less, where X is the number of {kind} counters on ~."
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r55-00 table parses as an ability, so none is silently dropped.
    use super::{R5500_SHAPES, R5500_WHOLES};

    #[test]
    fn r5500_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R5500_WHOLES).unwrap();
        assert_eq!(R5500_SHAPES.len(), entries.len());
    }
}
