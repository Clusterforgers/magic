//! Counters a permanent enters with that a count decides: "~ enters with a +1/+1 counter on it for each Zombie card in
//! your graveyard." The renderers word the replacement for a fixed number of counters.

use super::super::{counted_parts, variants};
use crate::ability::Ability;
use crate::trigger::ReplaceBy;

/// Adds the wordings of an "enters with" replacement whose number of counters is counted.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when,
        by:
            ReplaceBy::WithCounters {
                counter,
                count,
                tapped,
            },
        condition,
        from,
    } = ability
    else {
        return;
    };
    let Some((each, tail)) = counted_parts(count) else {
        return;
    };
    let single = Ability::Replacement {
        when: when.clone(),
        by: ReplaceBy::WithCounters {
            counter: counter.clone(),
            count: each,
            tapped: *tapped,
        },
        condition: condition.clone(),
        from: *from,
    };
    texts.extend(
        variants(&single)
            .iter()
            .filter_map(|text| text.strip_suffix('.'))
            .map(|sentence| format!("{sentence} {tail}.")),
    );
}
