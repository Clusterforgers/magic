//! Chance: "Flip a coin. If you win the flip, S. If you lose the flip, S." is a `FlipCoin` with a branch for each result,
//! "Roll a d20." is a `RollDice` whose table is the rows of the lines after it ("1—9 | S"), each row read as a spell and
//! attached to the roll by the compiler, and "that player flips a coin" is a flip by the player of the event.

use super::conditional::body;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::{DieRow, Effect};
use crate::refs::Ref;
use crate::value::Amount;

/// The largest number of faces a die the readers know has.
const MOST_SIDES: u32 = 1000;

/// "Flip a coin." with the branches after it: "If you win the flip, destroy all nonland permanents. If you lose the flip,
/// sacrifice ~." The win branch stands before the lose branch, each at most once; either may be left out.
pub fn flip(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let (who, they) = if r.words("flip a coin") {
        (Ref::You, false)
    } else if r.that_player && r.words("that player flips a coin") {
        (Ref::Triggering, true)
    } else {
        return None;
    };
    let (mut win, mut lose) = (Vec::new(), Vec::new());
    let mut seen = (false, false);
    while let Some((won, effects)) = r.attempt(|r| branch(r, they)) {
        let clear = if won { !seen.0 && !seen.1 } else { !seen.1 };
        clear.then_some(())?;
        if won {
            seen.0 = true;
            win = effects;
        } else {
            seen.1 = true;
            lose = effects;
        }
    }
    // A flip whose result does nothing is no effect of a card (its "wins a coin flip" triggers are other constructs).
    (!win.is_empty() || !lose.is_empty()).then_some(())?;
    Some(vec![Effect::FlipCoin { who, win, lose }])
}

/// ". If you win the flip, S" or ". If you lose the flip, S" (a player other than you: "If they lose the flip, S").
fn branch(r: &mut Reader<'_>, they: bool) -> Option<(bool, Vec<Effect>)> {
    r.punct('.').then_some(())?;
    r.need(if they { "if they" } else { "if you" })?;
    let won = r.word("win");
    if !won {
        r.need("lose")?;
    }
    r.need("the flip")?;
    r.punct(',').then_some(())?;
    let effects = body(r)?;
    (!effects.is_empty()).then_some((won, effects))
}

/// "Roll a d20.": a die of that many faces rolled by you, with a table of rows the lines after it fill in.
pub fn roll(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("roll a")?;
    let sides = r
        .take_word()?
        .strip_prefix('d')?
        .parse::<u32>()
        .ok()
        .filter(|sides| (2..=MOST_SIDES).contains(sides))?;
    Some(vec![Effect::RollDice {
        who: Ref::You,
        sides,
        plus: Amount::zero(),
        results: Vec::new(),
    }])
}

/// A row of the table of a die roll: the totals it holds and the text after the bar. "1—9 | S", "20 | S" (that total
/// only) and "15+ | S" (that total and more).
pub fn die_row(text: &str) -> Option<(u32, Option<u32>, &str)> {
    let (range, body) = text.split_once(" | ")?;
    let (from, to) = if let Some(from) = range.strip_suffix('+') {
        (from.parse().ok()?, None)
    } else if let Some((from, to)) = range.split_once('\u{2014}') {
        (from.parse().ok()?, Some(to.parse().ok()?))
    } else {
        let only = range.parse().ok()?;
        (only, Some(only))
    };
    (from >= 1 && to.is_none_or(|to| to >= from)).then_some((from, to, body))
}

/// The totals of a row as the card prints them.
pub fn range_text(from: u32, to: Option<u32>) -> String {
    match to {
        None => format!("{from}+"),
        Some(to) if to == from => from.to_string(),
        Some(to) => format!("{from}\u{2014}{to}"),
    }
}

/// The effects of an ability whose last effect may be the die roll that the rows after the ability's clause belong to.
fn effects_of(ability: &mut Ability) -> Option<&mut Vec<Effect>> {
    match ability {
        Ability::Spell { effects, .. }
        | Ability::Triggered { effects, .. }
        | Ability::Activated { effects, .. }
        | Ability::Loyalty { effects, .. } => Some(effects),
        _ => None,
    }
}

/// The table of the die roll that ends the effects, also inside the passes of a repeat.
fn tail_roll(effects: &mut [Effect]) -> Option<&mut Vec<DieRow>> {
    match effects.last_mut()? {
        Effect::RollDice { results, .. } => Some(results),
        Effect::RepeatEach { effects, .. } => tail_roll(effects),
        _ => None,
    }
}

/// Whether the ability ends in a die roll, whose table the rows after its clause fill.
pub fn ends_in_roll(ability: &Ability) -> bool {
    let mut copy = ability.clone();
    effects_of(&mut copy).is_some_and(|effects| tail_roll(effects).is_some())
}

/// The ability with a row more in the table of the die roll it ends in, when the row comes after the rows before it:
/// totals that rise, nothing after a row with no highest total.
pub fn add_row(mut ability: Ability, row: DieRow) -> Option<Ability> {
    let results = tail_roll(effects_of(&mut ability)?)?;
    let after = match results.last() {
        None => 1,
        Some(last) => last.to?.checked_add(1)?,
    };
    (row.from >= after).then_some(())?;
    results.push(row);
    Some(ability)
}

/// The rows of the table of the die roll an ability ends in.
pub fn rows(ability: &Ability) -> Vec<DieRow> {
    let mut copy = ability.clone();
    effects_of(&mut copy)
        .and_then(|effects| tail_roll(effects))
        .map(std::mem::take)
        .unwrap_or_default()
}

/// The wordings of each row of the table of the die roll an ability ends in, each with its totals before the bar.
pub fn row_texts(ability: &Ability) -> Vec<Vec<String>> {
    rows(ability)
        .into_iter()
        .map(|row| {
            let spell = Ability::Spell {
                targets: Vec::new(),
                effects: row.effects,
            };
            super::render::written_variants(&spell)
                .into_iter()
                .map(|text| format!("{} | {text}", range_text(row.from, row.to)))
                .collect()
        })
        .collect()
}
