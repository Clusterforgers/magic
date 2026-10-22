//! The face a spell is cast as (CR 601.2a): the half of a split card, the Adventure or the back face of a modal
//! double-faced card. The rules move the card to the stack before anything else is announced, so this comes first;
//! every later step reads the card as that face (`GameState::with_face`).
//!
//! The faces that can be cast now are tried one at a time (`faces`); with more than one the player picks with
//! `ChooseOption` over their names and costs, with one there is nothing to ask. The face is kept in `Choices.face`.

use mtg_core::ids::{ObjectId, PlayerId};

use super::{Done, Granted, Next};
use crate::choose::{ChoiceOption, Choices, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::faces;
use crate::mana::ManaPool;
use crate::priority::mana_options;
use crate::state::GameState;

/// Settles the face of the spell `card`, or returns the question that does or the end of the cast.
pub(super) fn run(
    state: &GameState,
    player: PlayerId,
    card: ObjectId,
    granted: Option<Granted>,
    choices: &mut Choices,
    done: &mut Done,
) -> Option<Next> {
    if done.face {
        return None;
    }
    let usable = if let Some(granted) = granted {
        faces::grantable_faces(state, player, card, granted)
    } else {
        let seat = state.player(player)?;
        let pool: ManaPool = seat.mana().clone();
        faces::castable_faces(
            state,
            player,
            card,
            &pool,
            &mana_options(state, player),
            seat.life(),
        )
    };
    let picked = match usable.as_slice() {
        [] => return Some(Next::Drop),
        [only] => *only,
        _ => match done.face_picked {
            Some(pick) if pick < usable.len() => usable[pick],
            _ => {
                let options = usable
                    .iter()
                    .map(|face| {
                        let (name, cost) = faces::describe(state, card, *face);
                        ChoiceOption::Label(
                            cost.map_or_else(|| name.clone(), |cost| format!("{name} {cost}")),
                        )
                    })
                    .collect();
                done.asked = Some(super::Phase::Face);
                return Some(Next::Ask(Decision {
                    player,
                    kind: DecisionKind::ChooseOption(OptionOptions {
                        source: card,
                        options,
                        text: "Choose which face to cast.".to_owned(),
                    }),
                }));
            }
        },
    };
    choices.face = picked;
    done.face = true;
    None
}
