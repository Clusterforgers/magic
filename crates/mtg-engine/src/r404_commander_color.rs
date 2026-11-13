//! The color a commander's owner chooses before the game begins: "If ~ is your commander, choose a color before the
//! game begins. ~ is the chosen color." (`Modification::SetCommanderColor`).
//!
//! The opening asks it of each commander whose card has such a static ability, in turn order from the first player and
//! before any hand is dealt; `CommanderColorChosen` records the color by the card's id, which stays the card's in every
//! zone, and the static ability reads it there.

use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::choose::{ChoiceOption, OptionChoice, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::state::GameState;

/// The words of the question.
const ASKED: &str = "Choose a color for your commander before the game begins.";

/// Whether the card of `id` has a static ability that makes it the color chosen for it as a commander.
fn asks_color(state: &GameState, id: ObjectId) -> bool {
    state.object(id).is_some_and(|object| {
        crate::ability_reader::definition_sets_commander_color(object.whole_card())
    })
}

/// The color chosen for the commander `id` before the game began, if one was.
pub(crate) fn chosen(state: &GameState, id: ObjectId) -> Option<Color> {
    state
        .commander_colors
        .iter()
        .find(|(card, _)| *card == id)
        .map(|(_, color)| *color)
}

/// The next color the opening asks for: a commander with the ability and no color yet, in turn order from the first
/// player, asked of its owner.
pub(crate) fn question(state: &GameState) -> Option<Decision> {
    let order = state.turn_order_from(state.first_player());
    order.into_iter().find_map(|player| {
        let id = state.commanders().find(|id| {
            state
                .object(*id)
                .is_some_and(|object| object.owner() == player)
                && asks_color(state, *id)
                && chosen(state, *id).is_none()
        })?;
        Some(Decision {
            player,
            kind: DecisionKind::ChooseOption(OptionOptions {
                source: id,
                options: Color::ALL.into_iter().map(ChoiceOption::Color).collect(),
                text: ASKED.to_owned(),
                can_cancel: false,
            }),
        })
    })
}

/// The event of the color picked for the commander the question names.
pub(crate) fn answer(
    player: PlayerId,
    options: &OptionOptions,
    choice: OptionChoice,
) -> Result<Outcome, EngineError> {
    let Some(ChoiceOption::Color(color)) = options.options.get(choice.0) else {
        return Err(IllegalAnswer::NotOffered("color").into());
    };
    Ok(Outcome::Events(vec![Event::CommanderColorChosen {
        player,
        object: options.source,
        color: *color,
    }]))
}
