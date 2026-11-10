//! "As an additional cost to cast this spell, you may sacrifice any number of artifacts and/or creatures. This spell costs
//! {2} less to cast for each permanent sacrificed this way and {2} less to cast for each other artifact or creature you've
//! sacrificed this turn.": the required cost, the reduction for what it pays and a second reduction for what was
//! sacrificed before the cast, one clause of three abilities.

use super::selectors::{Form, phrase_text};
use crate::ability::{Ability, CostChange};
use crate::refs::{Pred, Ref};
use crate::value::{Amount, TallyEvent};

/// Every wording of the three abilities as one clause; empty for any other three.
pub fn ep23_texts(first: &Ability, second: &Ability, third: &Ability) -> Vec<String> {
    let Some((less, noun)) = sacrificed_before(third) else {
        return Vec::new();
    };
    super::ep22_pairs::texts(first, second)
        .into_iter()
        .filter_map(|text| {
            let head = text.strip_suffix(" this way.")?;
            Some(format!(
                "{head} this way and {{{less}}} less to cast for each other {noun} you've sacrificed this turn."
            ))
        })
        .collect()
}

/// The generic mana a reduction of the spell itself takes off for each permanent of a kind its controller sacrificed
/// this turn, and the kind.
fn sacrificed_before(ability: &Ability) -> Option<(u32, String)> {
    let Ability::CostModifier {
        applies_to,
        change: CostChange::Reduce,
        mana,
        per,
        condition: None,
        targeting: None,
        activating: None,
        least: 0,
    } = ability
    else {
        return None;
    };
    if applies_to.0.as_slice() != [Pred::This] || mana.has_x() {
        return None;
    }
    let generic = mana.rest().generic();
    if generic == 0 || !mana.rest().is_generic_only(generic) {
        return None;
    }
    let (times, tally) = match per {
        Amount::Times { by, of } if *by > 0 => (u32::try_from(*by).ok()?, &**of),
        other => (1, other),
    };
    let Amount::Tally {
        event: TallyEvent::Sacrificed,
        who: Ref::You,
        filter,
    } = tally
    else {
        return None;
    };
    if filter.is_empty() {
        return None;
    }
    Some((generic * times, phrase_text(filter, Form::Count)?))
}
