//! `Effect::CreateToken`: tokens enter the battlefield under a player's control (CR 111).

use std::sync::Arc;

use mtg_cards::assemble::token_definition;
use mtg_cards::card::TokenSpec;
use mtg_cards::effect::{Effect, TokenRef};
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::caps::caps;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::combat::{AttackTarget, defenders_for};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::state::GameState;
use crate::tokens::TokenCreated;

/// What a predefined token is: the description in its file (`cards/tokens/<id>.json`).
fn predefined(id: &str) -> Result<&'static TokenSpec, ResolveError> {
    mtg_cards::builtin::library()
        .ok()
        .and_then(|library| library.token(id))
        .map(|file| &file.token)
        .ok_or_else(|| ResolveError::Invalid(format!("there is no predefined token `{id}`")))
}

/// What a player who puts tokens onto the battlefield attacking may have them attack: the other players and the
/// planeswalkers they control, while that player's own attack is under way and its attackers are declared.
fn attackable(state: &GameState, player: PlayerId) -> Vec<(AttackTarget, ChoiceOption)> {
    if state.turn().active() != player || !state.combat().attackers_declared() {
        return Vec::new();
    }
    defenders_for(state, player)
        .into_iter()
        .map(|target| {
            let option = match target {
                AttackTarget::Player(seat) => ChoiceOption::Player(seat),
                AttackTarget::Planeswalker(walker) => ChoiceOption::Label(
                    state
                        .object(walker)
                        .map_or_else(String::new, |o| o.card().name.clone()),
                ),
            };
            (target, option)
        })
        .collect()
}

/// Each player `who` names who is still in the game creates `count` tokens, which enter tapped when the effect says
/// so. A token that enters attacking (CR 508.4) was never declared as an attacker, so nothing triggers on its
/// attack; its controller chooses the player or planeswalker it attacks when there is more than one (an attack by
/// someone else's turn, or with no attackers declared yet, is no attack: the token just enters).
///
/// The characteristics and abilities come from the description, with the abilities the engine runs; they are in the
/// event, so a log folds with no card data at hand.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::CreateToken {
        who,
        token,
        count,
        tapped,
        attacking,
    } = effect
    else {
        return Err(wrong_effect("CreateToken"));
    };
    let spec = match token {
        TokenRef::Named(id) => predefined(id)?,
        TokenRef::Custom(spec) => &**spec,
        TokenRef::CopyOf { .. } => return Err(ResolveError::Unsupported("Effect.CreateToken.Copy")),
    };
    let state = cx.state();
    let number = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(0);
    let players: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    if number == 0 || players.is_empty() {
        return Ok(Run::events(Vec::new()));
    }
    // Where each player's tokens attack: chosen by their controller, once for all of them.
    let mut aims: Vec<(PlayerId, Option<AttackTarget>)> =
        players.iter().map(|player| (*player, None)).collect();
    if *attacking {
        let options: Vec<(PlayerId, Vec<(AttackTarget, ChoiceOption)>)> = players
            .iter()
            .map(|player| (*player, attackable(state, *player)))
            .filter(|(_, targets)| !targets.is_empty())
            .collect();
        let choosers: Vec<OptionChooser> = options
            .iter()
            .map(|(player, targets)| OptionChooser {
                player: *player,
                options: targets.iter().map(|(_, option)| option.clone()).collect(),
                text: "Choose a player or planeswalker for the tokens to attack.".into(),
            })
            .collect();
        match pick_options(cx, &choosers)? {
            Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
            Optioned::Done(taken) => {
                for (player, index) in taken {
                    let target = options
                        .iter()
                        .find(|(seat, _)| *seat == player)
                        .and_then(|(_, targets)| targets.get(index))
                        .map(|(target, _)| *target);
                    if let Some(aim) = aims.iter_mut().find(|(seat, _)| *seat == player) {
                        aim.1 = target;
                    }
                }
            }
        }
    }
    let definition = Arc::new(token_definition(spec, &caps()));
    let first = state.next_object().get();
    let mut made = 0;
    let mut events = Vec::with_capacity(players.len() * number);
    for (controller, aim) in aims {
        for _ in 0..number {
            events.push(Event::TokenCreated(TokenCreated {
                object: ObjectId(first + made),
                controller,
                name: definition.name.clone(),
                tapped: *tapped,
                attacking: aim,
                definition: Arc::clone(&definition),
            }));
            made += 1;
        }
    }
    Ok(Run::events(events))
}
