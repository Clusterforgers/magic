//! The reveal of the first card a player draws each turn (`GameRule::RevealFirstDraw`): after the replacements, each
//! card drawn that is its player's first this turn is revealed as it is drawn when a rule of theirs says so. An optional
//! reveal asks its player to choose the card or none, one question for each source that offers it; a `Revealed` and a `FirstDrawRevealed` naming
//! the sources follow the draw in the same batch.

use mtg_core::ids::{ObjectId, PlayerId};

use super::Processed;
use crate::choose::{CardsOptions, Choice};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::library::Revealed;
use crate::state::{GameState, Tally};

/// The events of the batch with the reveals of first draws added after their draws, or the question a reveal asks.
/// `answers` are the journal's answers the replacements left; any one no reveal asks for is refused.
pub(super) fn r4708_reveals(
    state: &GameState,
    proposed: Vec<Event>,
    events: Vec<Event>,
    answers: &[(PlayerId, Choice)],
) -> Result<Processed, EngineError> {
    let mut answers = answers.iter();
    let unasked = |answers: &mut std::slice::Iter<'_, (PlayerId, Choice)>| {
        answers.next().map_or(Ok(()), |extra| {
            Err(EngineError::InvalidEvent(format!(
                "{extra:?} answers a question that was not asked"
            )))
        })
    };
    if !state.r4708_any_first_draw_reveals() {
        unasked(&mut answers)?;
        return Ok(Processed::Events(events));
    }
    let mut drawn: Vec<(PlayerId, u32)> = Vec::new();
    let mut out = Vec::with_capacity(events.len());
    for event in events {
        let Event::CardDrawn { player, object } = event else {
            out.push(event);
            continue;
        };
        out.push(event);
        let earlier = if let Some((_, count)) = drawn.iter_mut().find(|(who, _)| *who == player) {
            *count += 1;
            *count - 1
        } else {
            drawn.push((player, 1));
            0
        };
        if state.tallied(player, Tally::Drawn) + earlier != 0 {
            continue;
        }
        let mut sources: Vec<ObjectId> = Vec::new();
        for (source, may) in state.r4708_first_draw_reveals(player) {
            if may {
                match answers.next() {
                    None => {
                        return Ok(Processed::Ask {
                            decision: Decision {
                                player,
                                kind: DecisionKind::ChooseCards(CardsOptions {
                                    one_per_name: false,
                                    source,
                                    from: vec![object],
                                    min: 0,
                                    max: 1,
                                    text: crate::wording::R4708_REVEAL_FIRST_DRAW.to_owned(),
                                    can_cancel: false,
                                    groups: Vec::new(),
                                    budget: None,
                                }),
                            },
                            proposed,
                        });
                    }
                    Some((who, Choice::Cards(cards)))
                        if *who == player && cards.iter().all(|card| *card == object) =>
                    {
                        if cards.is_empty() {
                            continue;
                        }
                    }
                    Some(other) => {
                        return Err(EngineError::InvalidEvent(format!(
                            "{other:?} is no answer to the reveal of {player:?}'s first draw"
                        )));
                    }
                }
            }
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
        if sources.is_empty() {
            continue;
        }
        out.push(Event::Revealed(Revealed {
            player,
            cards: vec![object],
        }));
        out.push(Event::FirstDrawRevealed {
            player,
            card: object,
            sources,
        });
    }
    unasked(&mut answers)?;
    Ok(Processed::Events(out))
}
