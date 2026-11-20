//! `Effect::R110ChooseSecretly`: the controller secretly chooses an opponent or one of some creature types, and the
//! source keeps it (CR 706); the other players see only that a choice was made until a cost reveals it.

use mtg_cards::effect::{Effect, R110Secret};
use mtg_core::card::Subtype;

use super::wrong_effect;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::events::Event;
use crate::object::Chosen;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Asks the controller (one option is taken without a question, none chooses nothing) and gives the value to the
/// source while it is still the permanent the ability came from.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R110ChooseSecretly(secret) = effect else {
        return Err(wrong_effect("R110ChooseSecretly"));
    };
    let state = cx.state();
    let controller = cx.scope.controller;
    let values: Vec<Chosen> = match secret {
        R110Secret::Opponent => state
            .turn_order_from(controller)
            .into_iter()
            .filter(|player| state.is_alive(*player) && state.is_opponent(controller, *player))
            .map(Chosen::Player)
            .collect(),
        R110Secret::CreatureType(types) => {
            types.iter().cloned().map(Chosen::CreatureType).collect()
        }
    };
    let options = values.iter().map(option).collect();
    let chooser = OptionChooser {
        player: controller,
        options,
        text: question(secret),
    };
    let taken = match pick_options(cx, &[chooser])? {
        Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        Optioned::Done(taken) => taken,
    };
    let source = cx.scope.source;
    let present = state.object(source.id).is_some_and(|object| {
        object.counter() == source.counter && object.zone() == Zone::Battlefield
    });
    let chosen = taken
        .first()
        .and_then(|(_, index)| values.get(*index))
        .filter(|_| present)
        .cloned();
    Ok(Run::events(
        chosen
            .map(|chosen| Event::R110ChoseSecretly {
                object: source.id,
                chosen,
            })
            .into_iter()
            .collect(),
    ))
}

/// The value as an option voted face down, so the answer is hidden from the other seats.
fn option(value: &Chosen) -> ChoiceOption {
    match value {
        Chosen::Player(player) => ChoiceOption::SecretPlayer(*player),
        Chosen::CreatureType(Subtype(name)) => ChoiceOption::Secret(name.clone()),
        other => ChoiceOption::Secret(format!("{other:?}")),
    }
}

/// The question in words.
fn question(secret: &R110Secret) -> String {
    match secret {
        R110Secret::Opponent => "Secretly choose an opponent.".to_owned(),
        R110Secret::CreatureType(types) => {
            let names: Vec<&str> = types.iter().map(|Subtype(name)| name.as_str()).collect();
            format!("Secretly choose {}.", names.join(", "))
        }
    }
}
