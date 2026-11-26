//! `Effect::R115RevealContest`: each player reveals the top card of their library, and the one player who revealed the
//! card with the greatest mana value wins (Psychic Battle). A tie for it, or no card revealed, leaves no winner; a player
//! with an empty library reveals nothing and is not compared (the Psychic Battle rulings).

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::Event;
use crate::library::{Revealed, top_cards};
use crate::resolve::{Cx, Next, ResolveError, Run};

/// Logs each reveal in turn order and binds the winner, if one, to the name.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R115RevealContest { who, bind } = effect else {
        return Err(wrong_effect("R115RevealContest"));
    };
    let state = cx.state();
    let mut events = Vec::new();
    let mut shown: Vec<(PlayerId, u32)> = Vec::new();
    for player in cx.scope.players(who)? {
        if !state.is_alive(player) {
            continue;
        }
        let Some(card) = top_cards(state, player, 1).into_iter().next() else {
            continue;
        };
        let Some(value) = state
            .object(card)
            .map(crate::object::GameObject::mana_value)
        else {
            continue;
        };
        events.push(Event::Revealed(Revealed {
            player,
            cards: vec![card],
        }));
        shown.push((player, value));
    }
    Ok(Run::Done {
        events,
        bound: vec![(
            bind.clone(),
            r115_winner(&shown)
                .map(Target::Player)
                .into_iter()
                .collect(),
        )],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// The one player who showed the greatest value; nobody on a tie for it or when nobody showed one.
fn r115_winner(shown: &[(PlayerId, u32)]) -> Option<PlayerId> {
    let best = shown.iter().map(|&(_, value)| value).max()?;
    match shown
        .iter()
        .filter(|&&(_, value)| value == best)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [(player, _)] => Some(*player),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the greatest value alone wins wherever it sits, a tie for the greatest leaves no winner even with a lower
    //! card shown, and nobody wins when no card is shown.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::first(&[5, 2], Some(0))]
    #[case::last(&[1, 2, 3, 7], Some(3))]
    #[case::tied_greatest(&[4, 4, 1], None)]
    #[case::tied_lower(&[4, 1, 1], Some(0))]
    #[case::zeroes(&[0, 0], None)]
    #[case::alone(&[0], Some(0))]
    #[case::none(&[], None)]
    fn r115_the_greatest_alone_wins(#[case] values: &[u32], #[case] winner: Option<u64>) {
        let shown: Vec<(PlayerId, u32)> = values
            .iter()
            .enumerate()
            .map(|(seat, value)| (PlayerId(u64::try_from(seat).unwrap_or(0)), *value))
            .collect();
        assert_eq!(r115_winner(&shown), winner.map(PlayerId));
    }
}
