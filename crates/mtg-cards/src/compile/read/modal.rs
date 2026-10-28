//! Modes: "Choose one —" and its bullet lines are one `Modal` effect. Each bullet is read by the
//! effect templates and keeps the targets it asks for, so a mode's `Target(0)` is its own.

use super::amounts::number_word;
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
/// more modes, "Choose one or both —" for one or two of a pair and "Choose one or more —" for any number but none.
/// "Choose three. You may choose the same mode more than once." (or another number) lets a mode repeat. "Choose one at
/// random —", "choose one that hasn't been chosen this turn —" (or that sentence ended by a period) and "choose one that
/// hasn't been chosen —" set a flag of the `Modal`; "Choose one. If you control a commander as you cast this spell, you
/// may choose both instead." sets the condition `all_if` of a pair.
pub fn modal(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("choose")?;
    let choose = c9_fight_choose(r)?;
    let single = choose == Bounds::default();
    let at_random = single && r.words("at random");
    let once_each_turn = single && !at_random && r.words("that hasn't been chosen this turn");
    let once_ever = single && !at_random && !once_each_turn && r.words("that hasn't been chosen");
    let repeat = !single && choose.max.is_some() && r.attempt(c11d_repeat).is_some();
    let all_if = if !single || at_random || once_each_turn || once_ever {
        None
    } else {
        r.attempt(both_instead)
    };
    // The sentence of the header may end with a period when the modes follow as bullet lines.
    (repeat || r.punct(DASH) || (once_each_turn || all_if.is_some()) && r.punct('.'))
        .then_some(())?;
    let mut modes = Vec::new();
    while r.punct(BULLET) {
        modes.push(mode(r)?);
    }
    let fits = c11d_fits(&choose, repeat, modes.len());
    (fits && (all_if.is_none() || modes.len() == 2)).then(|| {
        vec![Effect::Modal {
            choose,
            repeat,
            at_random,
            once_each_turn,
            once_ever,
            all_if,
            modes,
        }]
    })
}

/// Whether the header fits the number of modes: "one or both" names a pair, and a fixed number of two or more asks for
/// fewer than there are unless a mode may repeat ("two" of a pair would be both of them, which no card words so).
fn c11d_fits(choose: &Bounds, repeat: bool, modes: usize) -> bool {
    match (&choose.min, &choose.max) {
        (Amount::Fixed(1), Some(Amount::Fixed(2))) => modes == 2,
        (Amount::Fixed(n), Some(Amount::Fixed(_))) if *n >= 2 && !repeat => {
            usize::try_from(*n).is_ok_and(|n| modes > n)
        }
        _ => modes >= 2,
    }
}

/// ". You may choose the same mode more than once." after a number of modes.
fn c11d_repeat(r: &mut Reader<'_>) -> Option<()> {
    r.punct('.').then_some(())?;
    r.need("you may choose the same mode more than once")?;
    r.punct('.').then_some(())
}

/// How many modes the header asks for: "one or both" (one or two), "one or more" (one or any more), "one", or a number
/// from two to five.
fn c9_fight_choose(r: &mut Reader<'_>) -> Option<Bounds> {
    let (min, max) = if r.words("one or both") {
        (1, Some(2))
    } else if r.words("one or more") {
        (1, None)
    } else if r.words("one") {
        (1, Some(1))
    } else {
        let word = r.peek_word()?;
        let count = number_word(word).filter(|count| (2..=5).contains(count) && word != "a")?;
        r.word(word).then_some(())?;
        let count = i32::try_from(count).ok()?;
        (count, Some(count))
    };
    Some(Bounds {
        min: Amount::Fixed(min),
        max: max.map(Amount::Fixed),
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

/// The small words a mode's name may hold between its capitalized ones: "Hire a Mercenary —".
const C11D_SMALL: [&str; 6] = ["a", "an", "the", "of", "and", "to"];

/// The name a card gives a mode before its text: "Thay —", "Typhoid Mary —", "AV Bead —", "Hire a Mercenary —".
fn label(r: &mut Reader<'_>) -> Option<String> {
    let mut name = r
        .opening_name()
        .or_else(|| r.name().map(str::to_owned))?;
    loop {
        if let Some(next) = r.name() {
            name.push(' ');
            name.push_str(next);
        } else if let Some(small) = r.attempt(c11d_small) {
            name.push(' ');
            name.push_str(small);
        } else {
            break;
        }
    }
    r.punct(DASH).then_some(name)
}

/// A small word of a mode's name, when a capitalized word follows it.
fn c11d_small<'a>(r: &mut Reader<'a>) -> Option<&'a str> {
    let word = r.peek_word().filter(|word| C11D_SMALL.contains(word))?;
    r.word(word);
    r.lookahead(|r| r.name()).map(|_| word)
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
