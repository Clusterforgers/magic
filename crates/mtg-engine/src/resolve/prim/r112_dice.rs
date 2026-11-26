//! Die results stored on a permanent (CR 706): `Effect::R112RollStore` rolls dice and keeps the results in the source's
//! memory, `Effect::R112RerollStored` rerolls any number of them (Centaur of Attention).

use mtg_cards::effect::Effect;

use super::roll_dice::rolls;
use super::wrong_effect;
use crate::choose::{Questions, Stop};
use crate::events::Event;
use crate::memory::{Held, Remembered};
use crate::resolve::{Cx, ResolveError, Run};

/// The memory change that makes `results` the whole of the slot.
fn store(cx: &Cx<'_>, slot: &str, results: &[u32]) -> Event {
    Event::Remembered(Box::new(Remembered {
        source: cx.scope.source,
        slot: crate::memory::r112_lasting(slot),
        objects: Vec::new(),
        values: results
            .iter()
            .map(|result| Held::Number(i64::from(*result)))
            .collect(),
        replace: true,
    }))
}

/// The controller rolls the dice, each logged as `DieRolled` as the rules that change a roll leave it, and the results
/// replace what the source stored in the slot.
pub(super) fn run_store(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R112RollStore { sides, dice, slot } = effect else {
        return Err(wrong_effect("R112RollStore"));
    };
    let player = cx.scope.controller;
    let natural = rolls(cx.state(), player, *sides, *dice);
    let changed = match super::r6402_change_roll::r6402_change(cx, player, *sides, &natural)? {
        Ok(changed) => changed,
        Err(ask) => return Ok(ask),
    };
    let mut events = changed.events.clone();
    events.extend(changed.rolled(player, *sides));
    events.push(store(cx, slot, &changed.results));
    Ok(Run::events(events))
}

/// The controller is asked of each stored result in turn whether to reroll it; the dice chosen are rolled again, each
/// logged as `DieRolled`, and their new results take the old ones' places. Nothing stored asks nothing.
pub(super) fn run_reroll(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R112RerollStored { sides, slot } = effect else {
        return Err(wrong_effect("R112RerollStored"));
    };
    let player = cx.scope.controller;
    let stored: Vec<u32> = crate::memory::recall_numbers(
        cx.state(),
        cx.scope.source,
        &crate::memory::r112_lasting(slot),
    )
    .into_iter()
    .filter_map(|number| u32::try_from(number).ok())
    .collect();
    let asked = || -> Result<Vec<usize>, Stop> {
        let mut questions = Questions::new(cx);
        let mut chosen = Vec::new();
        for (at, result) in stored.iter().enumerate() {
            let text = format!("Reroll stored result {} ({result})?", at + 1);
            if questions.yes_no(player, text)? {
                chosen.push(at);
            }
        }
        questions.end()?;
        Ok(chosen)
    };
    let chosen = match asked() {
        Ok(chosen) => chosen,
        Err(stop) => return Stop::settle(Err(stop)),
    };
    if chosen.is_empty() {
        return Ok(Run::events(Vec::new()));
    }
    let count = u32::try_from(chosen.len()).unwrap_or(u32::MAX);
    let rerolled = rolls(cx.state(), player, *sides, count);
    let mut results = stored;
    let mut events = Vec::with_capacity(chosen.len() + 1);
    for (at, result) in chosen.iter().zip(&rerolled) {
        if let Some(old) = results.get_mut(*at) {
            *old = *result;
        }
        events.push(Event::DieRolled {
            player,
            sides: *sides,
            result: *result,
            natural: None,
        });
    }
    events.push(store(cx, slot, &results));
    Ok(Run::events(events))
}
