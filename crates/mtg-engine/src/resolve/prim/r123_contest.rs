//! `Effect::R123ExileContest`: each player exiles the top card of their library and the players tied for the greatest
//! mana value repeat among themselves until one alone has it (Timesifter).

use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// The contest over the libraries as they are: nothing changes a library between passes, so each pass exiles the next
/// card from the top of each player still in it. Every card exiled is logged, and the winner, if one, is bound.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R123ExileContest { who, bind } = effect else {
        return Err(wrong_effect("R123ExileContest"));
    };
    let state = cx.state();
    let libraries: Vec<(PlayerId, Vec<(ObjectId, u32)>)> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .map(|player| {
            let cards = top_cards(state, player, usize::MAX)
                .into_iter()
                .filter_map(|id| Some((id, state.object(id)?.mana_value())))
                .collect();
            (player, cards)
        })
        .collect();
    let (exiled, winner) = r123_contest(&libraries);
    let events = exiled
        .into_iter()
        .map(|(player, object)| Event::ZoneChanged {
            object,
            from: Zone::Library(player),
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        })
        .collect();
    Ok(Run::Done {
        events,
        bound: vec![(
            bind.clone(),
            winner.map(Target::Player).into_iter().collect(),
        )],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// The cards each pass exiles, in order, with their owners, and the player who alone exiled the greatest mana value in
/// the last pass; `libraries` holds each player's cards top first with their mana values. A player with no card left
/// exiles nothing and drops out; with no one left, no one wins.
fn r123_contest(
    libraries: &[(PlayerId, Vec<(ObjectId, u32)>)],
) -> (Vec<(PlayerId, ObjectId)>, Option<PlayerId>) {
    let mut exiled = Vec::new();
    let mut players: Vec<usize> = (0..libraries.len()).collect();
    for pass in 0.. {
        let shown: Vec<(usize, u32)> = players
            .iter()
            .filter_map(|&at| {
                let (player, cards) = &libraries[at];
                let &(card, value) = cards.get(pass)?;
                exiled.push((*player, card));
                Some((at, value))
            })
            .collect();
        let Some(best) = shown.iter().map(|&(_, value)| value).max() else {
            return (exiled, None);
        };
        players = shown
            .into_iter()
            .filter(|&(_, value)| value == best)
            .map(|(at, _)| at)
            .collect();
        if let [alone] = players.as_slice() {
            return (exiled, Some(libraries[*alone].0));
        }
    }
    (exiled, None)
}

#[cfg(test)]
mod tests {
    //! Goal: the contest exiles one card from each player still in it per pass, the greatest mana value alone wins, a tie
    //! sends only the tied players on, a player out of cards drops out, and empty libraries leave no winner.
    use rstest::rstest;

    use super::*;

    /// Libraries of mana values, top first, one a `|`-separated part (`"3 9|5 0"`); card ids are the player times 100
    /// plus the depth.
    fn libraries(values: &str) -> Vec<(PlayerId, Vec<(ObjectId, u32)>)> {
        values
            .split('|')
            .enumerate()
            .map(|(seat, cards)| {
                let player = PlayerId(u64::try_from(seat).unwrap());
                let cards = cards
                    .split_whitespace()
                    .enumerate()
                    .map(|(depth, value)| {
                        let id = ObjectId(u64::try_from(seat * 100 + depth).unwrap());
                        (id, value.parse().unwrap())
                    })
                    .collect();
                (player, cards)
            })
            .collect()
    }

    #[rstest]
    #[case::greatest_alone("3 9|5 0", Some(1), 2)]
    #[case::a_tie_repeats_among_the_tied("4 1|4 6|2 9", Some(1), 5)]
    #[case::a_second_tie_repeats_again("2 3 1|2 3 7", Some(1), 6)]
    #[case::out_of_cards_drops_out("4|4 0", Some(1), 3)]
    #[case::all_empty_no_winner("|", None, 0)]
    #[case::a_lone_player_wins("0", Some(0), 1)]
    fn the_contest_finds_the_greatest_mana_value_alone(
        #[case] values: &str,
        #[case] winner: Option<u64>,
        #[case] count: usize,
    ) {
        let (exiled, found) = r123_contest(&libraries(values));
        assert_eq!(found, winner.map(PlayerId));
        assert_eq!(exiled.len(), count);
    }
}
