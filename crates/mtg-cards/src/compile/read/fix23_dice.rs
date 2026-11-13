//! A die roll worded inline as the card prints it: a table of one row that holds every result is the roll and its effects
//! after it ("Roll a six-sided die. You gain life equal to the result."), and a six-sided die is printed as such.

use crate::ability::Ability;
use crate::effect::{DieRow, Effect};

/// The effects of the ability that a roll of it may end.
fn effects_of(ability: &mut Ability) -> Option<&mut Vec<Effect>> {
    match ability {
        Ability::Spell { effects, .. }
        | Ability::Triggered { effects, .. }
        | Ability::Activated { effects, .. }
        | Ability::Loyalty { effects, .. } => Some(effects),
        _ => None,
    }
}

/// The twin of an ability whose last effect rolls one die with a table of one row that holds every result: the roll with
/// no table and the row's effects after it.
pub fn fix23_inline_roll(ability: &Ability) -> Option<Ability> {
    let mut twin = ability.clone();
    let effects = effects_of(&mut twin)?;
    let Some(Effect::RollDice {
        dice: 1,
        choose: false,
        plus,
        results,
        count: None,
        ..
    }) = effects.last_mut()
    else {
        return None;
    };
    plus.is_zero().then_some(())?;
    let [
        DieRow {
            from: 1, to: None, ..
        },
    ] = results.as_slice()
    else {
        return None;
    };
    let row = results.pop()?;
    effects.extend(row.effects);
    Some(twin)
}

/// The wording with a six-sided die printed as such ("roll a d6" is "roll a six-sided die"), none without one.
pub fn fix23_six_sided(text: &str) -> Option<String> {
    let words = [
        ("roll a d6", "roll a six-sided die"),
        ("Roll a d6", "Roll a six-sided die"),
        ("roll two d6", "roll two six-sided dice"),
        ("Roll two d6", "Roll two six-sided dice"),
    ];
    let mut out = text.to_owned();
    for (from, to) in words {
        out = out.replace(from, to);
    }
    (out != text).then_some(out)
}

/// "If the result is 3 or less, you lose that much life.": the row of the lowest results of a roll, its amount of the
/// result said as "that much".
pub fn fix23_low_row(highest: u32, steps: &str) -> String {
    let steps = steps.replace("life equal to the result", "that much life");
    format!("if the result is {highest} or less, {steps}")
}
