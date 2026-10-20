//! `Effect::AddMana`: mana goes into a player's pool (CR 106.4).

use mtg_cards::effect::{Effect, Produce};
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaType;

use super::wrong_effect;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// The colours "any color" offers, in the order of the five-colour wheel.
const COLORS: [Color; 5] = [
    Color::White,
    Color::Blue,
    Color::Black,
    Color::Red,
    Color::Green,
];

/// The mana of a colour.
const fn mana_of(color: Color) -> ManaType {
    match color {
        Color::White => ManaType::White,
        Color::Blue => ManaType::Blue,
        Color::Black => ManaType::Black,
        Color::Red => ManaType::Red,
        Color::Green => ManaType::Green,
    }
}

/// Each player `who` names who is still in the game adds the mana `times` times to their pool. A fixed mana is
/// added as printed. "Any color" asks the player for a colour (`ChooseOption`) each time it is added, and every mana
/// of that addition has it ("three mana of any one color"); a choice with one option is not asked. The mana is
/// credited to the object that produced it while that object is in the game.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::AddMana { who, mana, times } = effect else {
        return Err(wrong_effect("AddMana"));
    };
    let state = cx.state();
    let times = usize::try_from(cx.scope.amount(times)?.max(0)).unwrap_or(0);
    let players: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    let source = Some(cx.scope.source.id).filter(|id| state.object(*id).is_some());
    let mut events = Vec::new();
    match mana {
        Produce::Fixed(list) => {
            for player in &players {
                for _ in 0..times {
                    events.extend(list.iter().map(|&mana| Event::ManaAdded {
                        player: *player,
                        mana,
                        source,
                    }));
                }
            }
        }
        Produce::AnyColor(each) => {
            let options: Vec<ChoiceOption> = COLORS.iter().copied().map(ChoiceOption::Color).collect();
            let choosers: Vec<OptionChooser> = players
                .iter()
                .flat_map(|player| std::iter::repeat_n(*player, times))
                .map(|player| OptionChooser {
                    player,
                    options: options.clone(),
                    text: "Choose a color of mana.".into(),
                })
                .collect();
            let taken = match pick_options(cx, &choosers)? {
                Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
                Optioned::Done(taken) => taken,
            };
            for (player, index) in taken {
                let Some(color) = COLORS.get(index).copied() else {
                    return Err(ResolveError::Invalid(format!("no color is option {index}")));
                };
                events.extend(std::iter::repeat_n(
                    Event::ManaAdded {
                        player,
                        mana: mana_of(color),
                        source,
                    },
                    usize::try_from(*each).unwrap_or(0),
                ));
            }
        }
    }
    Ok(Run::events(events))
}
