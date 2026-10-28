//! Modes: "Choose one —" and its bullet lines are one `Modal` effect. Each bullet is read by the
//! effect templates and keeps the targets it asks for, so a mode's `Target(0)` is its own.

use super::tokens::Reader;
use super::{effects, statics};
use crate::effect::{Effect, Mode};
use crate::refs::Bounds;
use crate::value::{Amount, Condition};

/// The bullet that opens a mode.
pub const BULLET: char = '\u{2022}';

/// The dash that ends the choice: "Choose one —".
pub const DASH: char = '\u{2014}';

/// "Choose one —" and the bullets after it, as one effect; at least two modes. "Choose two —" asks for two of three or
/// more modes and "Choose one or both —" for one or two of a pair. "Choose one at random —" and "choose one that hasn't
/// been chosen this turn —" (or that sentence ended by a period) set a flag of the `Modal`; "Choose one. If you control
/// a commander as you cast this spell, you may choose both instead." sets the condition `all_if` of a pair.
pub fn modal(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("choose")?;
    let choose = c9_fight_choose(r)?;
    let single = choose == Bounds::default();
    let at_random = single && r.words("at random");
    let once_each_turn = single && !at_random && r.words("that hasn't been chosen this turn");
    let all_if = if !single || at_random || once_each_turn {
        None
    } else {
        r.attempt(both_instead)
    };
    // The sentence of the header may end with a period when the modes follow as bullet lines.
    (r.punct(DASH) || (once_each_turn || all_if.is_some()) && r.punct('.')).then_some(())?;
    let mut modes = Vec::new();
    while r.punct(BULLET) {
        modes.push(mode(r)?);
    }
    // "one or both" names a pair, and "two" of a pair would be both of them, which no card words so.
    let fits = match (&choose.min, &choose.max) {
        (Amount::Fixed(1), Some(Amount::Fixed(2))) => modes.len() == 2,
        (_, Some(Amount::Fixed(2))) => modes.len() >= 3,
        _ => modes.len() >= 2,
    };
    (fits && (all_if.is_none() || modes.len() == 2)).then(|| {
        vec![Effect::Modal {
            choose,
            repeat: false,
            at_random,
            once_each_turn,
            all_if,
            modes,
        }]
    })
}

/// How many modes the header asks for: "one or both" (one or two), "one" or "two".
fn c9_fight_choose(r: &mut Reader<'_>) -> Option<Bounds> {
    let (min, max) = if r.words("one or both") {
        (1, 2)
    } else if r.words("one") {
        (1, 1)
    } else if r.words("two") {
        (2, 2)
    } else {
        return None;
    };
    Some(Bounds {
        min: Amount::Fixed(min),
        max: Some(Amount::Fixed(max)),
    })
}

/// ". If you control a commander as you cast this spell, you may choose both instead" (a spell) and ". If you control a
/// commander, you may choose both instead" (a trigger), up to the period that ends the header: the condition.
fn both_instead(r: &mut Reader<'_>) -> Option<Condition> {
    r.punct('.').then_some(())?;
    r.need("if")?;
    let condition = statics::control(r)?;
    if r.words("as you cast") {
        r.source().then_some(())?;
    }
    r.punct(',').then_some(())?;
    r.need("you may choose both instead")?;
    Some(condition)
}

/// The name a card gives a mode before its text: "Thay —", "Typhoid Mary —".
fn label(r: &mut Reader<'_>) -> Option<String> {
    let mut name = r.opening_name()?;
    while let Some(next) = r.name() {
        name.push(' ');
        name.push_str(next);
    }
    r.punct(DASH).then_some(name)
}

/// The sentences of one bullet, up to the next bullet or the end, with targets of their own and a
/// fresh "it": nothing a mode names is known to the others.
fn mode(r: &mut Reader<'_>) -> Option<Mode> {
    let outer = (std::mem::take(&mut r.targets), r.it.take());
    let label = r.attempt(label);
    let mut effects = Vec::new();
    let mut read = true;
    while read && !r.end() && !r.at_punct(BULLET) {
        read = effects::sentence(r, &mut effects);
    }
    let (targets, it) = outer;
    let own = std::mem::replace(&mut r.targets, targets);
    r.it = it;
    // A mode within a mode is not a thing a card does.
    let plain = !effects
        .iter()
        .any(|effect| matches!(effect, Effect::Modal { .. }));
    (read && plain && !effects.is_empty()).then_some(Mode {
        label,
        targets: own,
        effects,
    })
}
