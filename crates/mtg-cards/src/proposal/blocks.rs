//! Level, Class and Station blocks: the abilities of a proposal that work under the level of a block are paired with its
//! header and lines, and each is written back without the condition the block adds.

use super::ClauseState;
use super::pairing::unmatched;
use crate::ability::Ability;
use crate::compile::{BlockPlan, keyword_grant, same_text, without_header, written_variants};
use crate::modify::Modification;
use crate::refs::Ref;
use crate::value::Amount;

/// The abilities of a proposal that stand for one block of the text.
#[derive(Debug)]
pub(super) struct Found {
    /// The abilities that stand for the block, as indexes into the face's abilities.
    pub claimed: Vec<usize>,
    /// Abilities that work under the block's level and have no line left.
    pub strays: Vec<usize>,
    /// How the block stands: read when every header, line and ability is paired and written back.
    pub state: ClauseState,
}

/// An ability of the proposal that works under the block, without the condition the block adds.
struct Owned {
    at: usize,
    bare: Ability,
    written: Vec<String>,
}

impl Owned {
    /// Whether the ability, without the level, is the line: a line of keywords is the keywords granted, any other line
    /// is a wording of the ability.
    fn says(&self, line: &str) -> bool {
        keyword_grant(line).is_some_and(|grant| grant == self.bare)
            || self.written.iter().any(|text| same_text(text, line))
    }
}

/// Pairs the free abilities with the block: the head (the level up ability, the size) by value, then the lines under
/// the header by their wording. `None` when no ability works under the block, so the clause is open.
pub(super) fn read(plan: &BlockPlan, abilities: &[Ability], free: &[usize]) -> Option<Found> {
    let mut claimed = Vec::new();
    let mut wanted: Vec<&str> = Vec::new();
    for (text, expected) in &plan.head {
        let found = free
            .iter()
            .find(|&&at| !claimed.contains(&at) && &abilities[at] == expected);
        match found {
            Some(&at) => claimed.push(at),
            None => wanted.push(text),
        }
    }
    let mut owned: Vec<Owned> = free
        .iter()
        .filter(|at| !claimed.contains(at))
        .filter_map(|&at| {
            let bare = without_header(&abilities[at], &plan.condition)?;
            let written = written_variants(&bare);
            Some(Owned { at, bare, written })
        })
        .collect();
    if claimed.is_empty() && owned.is_empty() {
        return None;
    }
    for line in &plan.lines {
        match owned.iter().position(|ability| ability.says(line)) {
            Some(hit) => claimed.push(owned.remove(hit).at),
            None => wanted.push(line),
        }
    }
    let first = claimed
        .iter()
        .chain(owned.iter().map(|o| &o.at))
        .min()
        .copied()?;
    let mut state = ClauseState::Reads { ability: first };
    let mut strays = Vec::new();
    let mut leftover = owned.into_iter();
    for text in wanted {
        let Some(ability) = leftover.next() else {
            state = worse(
                state,
                ClauseState::Differs {
                    ability: first,
                    written: format!("(nothing for the line \u{201c}{text}\u{201d})"),
                },
            );
            continue;
        };
        claimed.push(ability.at);
        let other = match size_line(&ability.bare) {
            Some(written) => ClauseState::Differs {
                ability: ability.at,
                written,
            },
            None => unmatched(&ability.bare, ability.at, text, text),
        };
        state = worse(state, other);
    }
    strays.extend(leftover.map(|ability| ability.at));
    Some(Found {
        claimed,
        strays,
        state,
    })
}

/// The line a level gives its creature when it sets the size: "2/4".
fn size_line(ability: &Ability) -> Option<String> {
    let Ability::Static {
        affected: Ref::This,
        mods,
        ..
    } = ability
    else {
        return None;
    };
    match mods.as_slice() {
        [
            Modification::SetPT {
                power: Amount::Fixed(power),
                toughness: Amount::Fixed(toughness),
            },
        ] => Some(format!("{power}/{toughness}")),
        _ => None,
    }
}

/// The state that tells more: a wording that differs before a construct without a writer before a block that reads.
fn worse(state: ClauseState, other: ClauseState) -> ClauseState {
    let rank = |state: &ClauseState| match state {
        ClauseState::Differs { .. } => 2,
        ClauseState::Unwritten { .. } => 1,
        _ => 0,
    };
    if rank(&other) > rank(&state) {
        other
    } else {
        state
    }
}
