//! A word chosen as the permanent enters (the Sieges): "As ~ enters, choose Khans or Dragons." is the choice
//! (`ReplaceBy::ChooseWord`) and each bullet after it ("• Khans — ...") is one ability that works while its word was
//! chosen (`Condition::EnteredAs`), so the name of a mode is kept in the rules.

use super::Own;
use super::levels::Plan;
use crate::ability::Ability;
use crate::compile::clauses;
use crate::trigger::ReplaceBy;
use crate::value::Condition;

/// The choice of the header and the line of each word, each working while its word was chosen.
pub(super) fn plan(text: &str) -> Option<Plan> {
    let (words, modes) = clauses::word_block(text)?;
    let head = text.lines().next()?;
    let words: Vec<String> = words.into_iter().map(str::to_owned).collect();
    let choice = super::statics::replacement(ReplaceBy::ChooseWord(words), None);
    let conditions: Vec<Condition> = modes
        .iter()
        .map(|(word, _)| Condition::EnteredAs((*word).to_owned()))
        .collect();
    Some(Plan {
        condition: conditions.first()?.clone(),
        head: vec![(head.to_owned(), choice)],
        lines: modes.iter().map(|(_, body)| (*body).to_owned()).collect(),
        conditions,
    })
}

/// The abilities of the block: the choice, then each line under its word; `None` when a line is not read.
pub fn block(text: &str, x_free: bool, own: &Own) -> Option<Vec<Ability>> {
    plan(text)?.read(x_free, own)
}
