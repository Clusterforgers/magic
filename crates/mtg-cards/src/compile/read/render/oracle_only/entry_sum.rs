//! Counters a permanent enters with that two counts decide: "~ enters with a +1/+1 counter on it for each other Zombie
//! you control and each Zombie card in your graveyard." The renderers write the sum with "where X is".

use super::super::{counted_parts, variants};
use crate::ability::Ability;
use crate::trigger::ReplaceBy;
use crate::value::Amount;

/// Adds the wordings of an "enters with" replacement whose number of counters is the sum of two counts.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when,
        by:
            ReplaceBy::WithCounters {
                counter,
                count: Amount::Sum(parts),
                tapped,
            },
        condition,
        from,
    } = ability
    else {
        return;
    };
    let [first, second] = parts.as_slice() else {
        return;
    };
    let (Some((Amount::Fixed(1), each)), Some((Amount::Fixed(1), then))) =
        (counted_parts(first), counted_parts(second))
    else {
        return;
    };
    let Some(then) = then.strip_prefix("for ") else {
        return;
    };
    let single = Ability::Replacement {
        when: when.clone(),
        by: ReplaceBy::WithCounters {
            counter: counter.clone(),
            count: Amount::Fixed(1),
            tapped: *tapped,
        },
        condition: condition.clone(),
        from: *from,
    };
    texts.extend(
        variants(&single)
            .iter()
            .filter_map(|text| text.strip_suffix('.'))
            .map(|sentence| format!("{sentence} {each} and {then}.")),
    );
}
