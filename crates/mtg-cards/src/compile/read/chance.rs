//! Chance: "Flip a coin. If you win the flip, S. If you lose the flip, S." is a `FlipCoin` with a branch for each result,
//! "Roll a d20." is a `RollDice` whose table is the rows of the lines after it ("1—9 | S"), each row read as a spell and
//! attached to the roll by the compiler, and "that player flips a coin" is a flip by the player of the event.

use super::conditional::body;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::{DieRow, Effect};
use crate::refs::Ref;
use crate::value::{Amount, DieResult};

/// The largest number of faces a die the readers know has.
const MOST_SIDES: u32 = 1000;

/// "Flip a coin." with the branches after it: "If you win the flip, destroy all nonland permanents. If you lose the flip,
/// sacrifice ~." Each branch stands at most once and either may be left out; the lose branch may come first.
pub fn flip(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let (who, they) = if r.words("flip a coin") {
        (Ref::You, false)
    } else if r.that_player && r.words("that player flips a coin") {
        (Ref::Triggering, true)
    } else {
        return None;
    };
    let until = !they && r.words("until you lose a flip");
    let (mut win, mut lose) = (Vec::new(), Vec::new());
    let mut seen = (false, false);
    // "If you lose the flip, S. If you win the flip, S." (Krark, the Thumbless): the same flip, written back that way.
    let mut lose_first = false;
    while let Some((won, effects)) = r.attempt(|r| branch(r, they)) {
        let clear = if won { !seen.0 } else { !seen.1 };
        clear.then_some(())?;
        if won {
            lose_first = seen.1;
            seen.0 = true;
            win = effects;
        } else {
            seen.1 = true;
            lose = effects;
        }
    }
    // A flip with no branch still counts for "whenever you win a coin flip"; a flip until a loss takes no branch.
    (!until || (win.is_empty() && lose.is_empty())).then_some(())?;
    Some(vec![Effect::FlipCoin {
        who,
        win,
        lose_first,
        lose,
        until,
        won_as: None,
        heads: false,
    }])
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

/// "Roll a d20.": a die of that many faces rolled by you, with a table of rows the lines after it fill in. "Roll two d4
/// and choose one result.": two dice, one result chosen, which the effects after it read. "Roll a d20 for each player
/// being attacked and ignore all but the highest roll.": one die per attacked player, the highest counting.
pub fn roll(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("roll")?;
    if let Some(several) = r.attempt(r4703_several_dice) {
        return Some(several);
    }
    let dice = if r.word("a") {
        1
    } else {
        r.word("two").then_some(2)?
    };
    let sides = r
        .take_word()?
        .strip_prefix('d')?
        .parse::<u32>()
        .ok()
        .filter(|sides| (2..=MOST_SIDES).contains(sides))?;
    let choose = dice > 1;
    if choose {
        r.need("and choose one result")?;
    }
    let highest = (dice == 1
        && r.words("for each player being attacked and ignore all but the highest roll"))
    .then(|| Box::new(Amount::Players(Ref::Attacked)));
    Some(vec![Effect::RollDice {
        who: Ref::You,
        sides,
        dice,
        choose,
        plus: Amount::zero(),
        highest,
        results: Vec::new(),
        count: None,
    }])
}

/// The words of the faces of a die rolled with others: "six-sided".
pub(super) const R4703_SIDES: [(&str, u32); 4] = [
    ("four-sided", 4),
    ("six-sided", 6),
    ("eight-sided", 8),
    ("twenty-sided", 20),
];

/// "Roll three six-sided dice": several dice whose results are read together, none chosen.
fn r4703_several_dice(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    let dice = super::amounts::number_word(r.take_word()?).filter(|dice| *dice > 1)?;
    let sides = R4703_SIDES
        .iter()
        .find(|(words, _)| r.word(words))
        .map(|(_, sides)| *sides)?;
    r.word("dice").then_some(())?;
    Some(vec![Effect::RollDice {
        who: Ref::You,
        sides,
        dice,
        choose: false,
        plus: Amount::zero(),
        highest: None,
        results: Vec::new(),
        count: None,
    }])
}

/// "For each different result, S.": the steps once for each different number the dice rolled; one token creation, made
/// that many times.
pub fn r4703_each_result(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("for each different result")?;
    r.punct(',').then_some(())?;
    let effects = body(r)?;
    let [Effect::CreateToken { count, .. }] = effects.as_slice() else {
        return None;
    };
    count.is_one().then_some(())?;
    let mut effects = effects;
    if let Some(Effect::CreateToken { count, .. }) = effects.first_mut() {
        *count = Amount::DieResult(DieResult::Distinct);
    }
    Some(effects)
}

/// "the result" of a die, "that result" chosen among two and "the other result": a result of the latest roll.
pub(super) fn c11e_result(r: &mut Reader<'_>) -> Option<Amount> {
    let which = if r.words("the result") {
        DieResult::Only
    } else if r.words("that result") {
        DieResult::Chosen
    } else {
        r.need("the other result")?;
        DieResult::Other
    };
    Some(Amount::DieResult(which))
}

/// The words of a result of the latest roll.
pub(super) fn c11e_result_text(which: DieResult) -> String {
    match which {
        DieResult::Only => "the result".to_owned(),
        DieResult::Chosen => "that result".to_owned(),
        DieResult::Other => "the other result".to_owned(),
        DieResult::Odd => "the number of odd results".to_owned(),
        DieResult::Even => "the number of even results".to_owned(),
        DieResult::Total => "the total of those results".to_owned(),
        DieResult::Times(result) => format!("the number of times you rolled {result}"),
        DieResult::Distinct => "the number of different results".to_owned(),
    }
}

/// A row of the table of a die roll: the totals it holds and the text after the bar. "1—9 | S" (or "1-9 | S"), "20 | S"
/// (that total only), "15+ | S" (that total and more) and "9 or less | S" (from 1 to that total).
pub fn die_row(text: &str) -> Option<(u32, Option<u32>, &str)> {
    let (range, body) = text.split_once(" | ")?;
    let (from, to) = if let Some(from) = range.strip_suffix('+') {
        (from.parse().ok()?, None)
    } else if let Some(to) = range.strip_suffix(" or less") {
        (1, Some(to.parse().ok()?))
    } else if let Some((from, to)) = range.split_once(['\u{2014}', '-']) {
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

/// A row's wording, and the same words with "You" before an imperative verb ("You draw a card."): the roller is the
/// controller, who a row's plain verb names.
fn r5518_with_subject(text: String) -> Vec<String> {
    const VERBS: [&str; 10] = [
        "Draw",
        "Create",
        "Gain",
        "Lose",
        "Scry",
        "Surveil",
        "Mill",
        "Discard",
        "Sacrifice",
        "Return",
    ];
    let subject = text
        .split_once(' ')
        .filter(|(verb, _)| VERBS.contains(verb))
        .map(|(verb, rest)| format!("You {} {rest}", verb.to_lowercase()));
    std::iter::once(text).chain(subject).collect()
}

/// Every way a card prints the totals of a row: the dash may be a hyphen, and a range from 1 may read "N or less".
pub fn r5518_range_texts(from: u32, to: Option<u32>) -> Vec<String> {
    let mut texts = vec![range_text(from, to)];
    if let Some(to) = to.filter(|&to| to > from) {
        texts.push(format!("{from}-{to}"));
        if from == 1 {
            texts.push(format!("{to} or less"));
        }
    }
    texts
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
            let ranges = r5518_range_texts(row.from, row.to);
            super::render::written_variants(&spell)
                .into_iter()
                .flat_map(r5518_with_subject)
                .flat_map(|text| {
                    ranges
                        .iter()
                        .map(move |range| format!("{range} | {text}"))
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .collect()
}
