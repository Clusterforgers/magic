//! `Effect::Reattack`: the controller may reselect what attacking creatures attack ("you may reselect which player or
//! permanent target attacking creature is attacking").

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::combat::{AttackTarget, defenders_for};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::state::GameState;

/// What an attacker may attack now: everything its controller could attack, players only when `players` says so,
/// ignoring the requirements, restrictions and costs of attacking; with what it attacks now first, as the choice to
/// keep it (`None` once that has left).
fn choices(
    state: &GameState,
    controller: mtg_core::ids::PlayerId,
    now: Option<AttackTarget>,
    players: bool,
) -> Vec<Option<AttackTarget>> {
    let mut options = vec![now];
    options.extend(
        defenders_for(state, controller)
            .into_iter()
            .filter(|target| !players || matches!(target, AttackTarget::Player(_)))
            .filter(|target| Some(*target) != now)
            .map(Some),
    );
    options
}

/// How an option reads: the player, or the name of the permanent.
fn option(state: &GameState, target: Option<AttackTarget>) -> ChoiceOption {
    match target {
        Some(AttackTarget::Player(seat)) => ChoiceOption::Player(seat),
        Some(AttackTarget::Planeswalker(id)) => ChoiceOption::Label(
            state
                .object(id)
                .map_or_else(String::new, |object| object.card().name.clone()),
        ),
        None => ChoiceOption::Label("Keep it as it is".to_owned()),
    }
}

/// For each attacking creature `what` names, in order, the controller of the effect chooses what it attacks: what it
/// attacks now (the first option, so the "may" can be declined), or another player, planeswalker or battle its controller
/// could attack. A creature that ends up attacking something else logs `AttackRedirected`; nothing that triggers on
/// attacking sees it, and it still counts as having attacked what it was declared to attack.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Reattack { what, players } = effect else {
        return Err(wrong_effect("Reattack"));
    };
    let state = cx.state();
    let combat = state.combat();
    let mut attackers = Vec::new();
    for object in cx.scope.live_objects(what)? {
        let Some(attacker) = combat
            .attackers()
            .iter()
            .find(|a| a.object() == object.id())
        else {
            continue;
        };
        if attackers.iter().any(|(id, _)| *id == object.id()) {
            continue;
        }
        let now = (!attacker.attacks_nothing()).then_some(attacker.target);
        attackers.push((
            object.id(),
            choices(state, object.controller(), now, *players),
        ));
    }
    let choosers: Vec<OptionChooser> = attackers
        .iter()
        .map(|(id, options)| OptionChooser {
            player: cx.scope.controller,
            options: options
                .iter()
                .map(|target| option(state, *target))
                .collect(),
            text: format!(
                "Choose what {} attacks.",
                state
                    .object(*id)
                    .map_or_else(String::new, |object| object.card().name.clone())
            ),
        })
        .collect();
    let taken = match pick_options(cx, &choosers)? {
        Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        Optioned::Done(taken) => taken,
    };
    let events = attackers
        .iter()
        .zip(taken)
        .filter_map(|((object, options), (_, index))| {
            let target = (*options.get(index)?)?;
            (index > 0).then_some(Event::AttackRedirected {
                object: *object,
                target,
            })
        })
        .collect();
    Ok(Run::events(events))
}
