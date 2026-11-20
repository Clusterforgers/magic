//! "Each player chooses artifact, creature, or land during their untap step. That player can untap only permanents of
//! the chosen type this step." (`GameRule::R112UntapChosenType`, CR 502.3): before anything untaps, each active player
//! the rule names chooses a type (`Event::R112UntapTypeChosen`), and the untap step leaves the other permanents tapped.

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::choose::{ChoiceOption, OptionChoice, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::state::GameState;

/// The types offered, in this order.
const R112_TYPES: [CardType; 3] = [CardType::Artifact, CardType::Creature, CardType::Land];

/// The type `player` chose this turn, if any.
fn chosen(state: &GameState, player: PlayerId) -> Option<CardType> {
    state
        .r112_untap_types
        .iter()
        .find(|(who, _)| *who == player)
        .map(|(_, kind)| *kind)
}

/// The question of the first active player the rule names who has not chosen yet: a choice among artifact, creature
/// and land.
pub(crate) fn r112_question(state: &GameState, actives: &[PlayerId]) -> Option<Decision> {
    let rules = crate::rules::r112_untap_chosen(state);
    let (player, source) = actives.iter().find_map(|player| {
        rules
            .iter()
            .find(|(named, _)| named == player)
            .filter(|_| chosen(state, *player).is_none())
            .copied()
    })?;
    Some(Decision {
        player,
        kind: DecisionKind::ChooseOption(OptionOptions {
            source,
            options: R112_TYPES
                .iter()
                .map(|kind| ChoiceOption::Label(format!("{kind:?}")))
                .collect(),
            text:
                "Choose artifact, creature, or land: only permanents of that type untap this step."
                    .to_owned(),
            can_cancel: false,
        }),
    })
}

/// The answer to the question: the type chosen, logged.
pub(crate) fn r112_answer(
    state: &GameState,
    player: PlayerId,
    choice: OptionChoice,
) -> Result<Vec<Event>, EngineError> {
    let asked = r112_question(state, &state.active_players())
        .is_some_and(|question| question.player == player);
    let kind = R112_TYPES
        .get(choice.0)
        .filter(|_| asked)
        .ok_or(IllegalAnswer::NotOffered("untap type"))?;
    Ok(vec![Event::R112UntapTypeChosen {
        player,
        kind: *kind,
    }])
}

/// Whether the untap step is asking the question.
pub(crate) fn r112_asks(state: &GameState) -> bool {
    state.turn().step() == crate::turn::Step::Untap
        && r112_question(state, &state.active_players()).is_some()
}

/// Whether the permanent may untap this step: of the type its controller chose, or the type the first active player who
/// chose did when its controller did not choose; any permanent when nobody chose.
pub(crate) fn r112_allows(state: &GameState, id: ObjectId, actives: &[PlayerId]) -> bool {
    let controller = crate::characteristics::controller_of(state, id);
    let kind = controller
        .and_then(|player| chosen(state, player))
        .or_else(|| actives.iter().find_map(|player| chosen(state, *player)));
    kind.is_none_or(|kind| {
        crate::characteristics::characteristics(state, id)
            .is_some_and(|chars| chars.types.contains(kind))
    })
}
