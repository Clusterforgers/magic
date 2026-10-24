//! A pump of a group and the untap of the same group: "creatures target player controls get +2/+2 until end of turn.
//! Untap them." The renderers write the duration first, "each" and "all", and "Then untap".

use crate::ability::Ability;
use crate::effect::Effect;

/// Adds the wordings of a group pumped until end of turn and untapped.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let effects = ability.effects();
    let both = effects.windows(2).any(|pair| {
        matches!(
            pair,
            [
                Effect::Continuous { affected: crate::refs::Ref::Each(pumped), .. },
                Effect::Untap { what: crate::refs::Ref::Each(untapped) },
            ] if pumped == untapped
        )
    });
    if !both {
        return;
    }
    let said: Vec<String> = texts.iter().filter_map(|text| grouped(text)).collect();
    texts.extend(said);
}

/// "Until end of turn, each N W controls gets S. Then untap all Ns W controls" as "Ns W controls get S until end of
/// turn. Untap them".
fn grouped(text: &str) -> Option<String> {
    let (head, rest) = text.split_once("Until end of turn, each ")?;
    let (pumped, rest) = rest.split_once(" gets ")?;
    let (stats, rest) = rest.split_once(". Then untap all ")?;
    let controls = pumped.split_once(' ')?.1;
    let plural_group = rest.split_once(controls)?.0;
    let tail = rest.split_once(controls)?.1;
    Some(format!(
        "{head}{plural_group}{controls} get {stats} until end of turn. Untap them{tail}"
    ))
}
