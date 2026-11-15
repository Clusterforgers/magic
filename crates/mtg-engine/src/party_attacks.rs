//! A team's shared combat (CR 805.10, CR 810.7, CR 508.1): where teams share their turns (Two-Headed Giant, shared team
//! turns) each player of the active team chooses the attacks of their own creatures in turn order, and the team makes
//! one combined declaration of every choice, checked as a whole: its limits, requirements, costs and "attacks alone"
//! read the combined set (CR 805.10b).

use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::characteristics::controller_of;
use crate::combat::{AttackExtras, AttackTarget, attack_options, check_attackers};
use crate::decision::{AttackOptions, Decision, DecisionKind, PartyAttack};
use crate::error::EngineError;
use crate::events::Event;
use crate::state::GameState;

/// How often a team's choice may be refused in one combat before the engine declares the plain legal attack.
const MOST_REFUSALS: usize = 2;

/// One player's part of a team's declaration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct AttackPick {
    /// The player who chose.
    pub(crate) player: PlayerId,
    /// Their attackers and targets.
    pub(crate) attackers: Vec<(ObjectId, AttackTarget)>,
    /// The attackers they exert.
    pub(crate) exert: Vec<ObjectId>,
    /// The bands they form.
    pub(crate) bands: Vec<Vec<ObjectId>>,
}

/// Whether the players of the active team each choose their own attacks: teams that share their turns, with more than
/// one of the team still in the game. Where every player takes their own turn the active player declares alone.
pub(crate) fn splits(state: &GameState) -> bool {
    state.shares_turns() && state.active_player().is_some() && state.active_players().len() > 1
}

/// The player of the active team who chooses next, in turn order; `None` once all have chosen.
fn next_picker(state: &GameState) -> Option<PlayerId> {
    state.active_players().into_iter().find(|player| {
        !state
            .combat
            .attack_picks
            .iter()
            .any(|pick| pick.player == *player)
    })
}

/// Whether `player` is the last of the team still to choose.
fn is_last(state: &GameState, player: PlayerId) -> bool {
    state
        .active_players()
        .into_iter()
        .filter(|seat| *seat != player)
        .all(|seat| {
            state
                .combat
                .attack_picks
                .iter()
                .any(|pick| pick.player == seat)
        })
}

/// The attacks the team chose so far.
fn chosen(state: &GameState) -> Vec<(ObjectId, AttackTarget)> {
    state
        .combat
        .attack_picks
        .iter()
        .flat_map(|pick| pick.attackers.iter().copied())
        .collect()
}

/// The options of `picker`: the team's narrowed to the picker's own creatures, each limit less what the teammates
/// chose. What only the whole declaration can show (a creature that can't attack alone, joins, an attack at each
/// player) is checked once the last player has chosen.
pub(crate) fn pick_options(state: &GameState, picker: PlayerId) -> AttackOptions {
    let mut options = attack_options(state, state.turn().active());
    let before = chosen(state);
    let own = |id: &ObjectId| controller_of(state, *id) == Some(picker);
    options.attackers.retain(|option| own(&option.object));
    options.required.retain(own);
    options.cant_alone.clear();
    options.joins.clear();
    options.at_least_one = None;
    options.most = options.most.map(|most| most.saturating_sub(before.len()));
    for (target, most) in &mut options.most_at {
        let taken = before.iter().filter(|(_, at)| at == target).count();
        *most = most.saturating_sub(taken);
    }
    options.party = Some(Box::new(PartyAttack {
        declared: before
            .iter()
            .filter_map(|(object, target)| Some((controller_of(state, *object)?, *object, *target)))
            .collect(),
        refused: state.combat.attack_refusals.last().cloned(),
    }));
    options
}

/// The question of the declare-attackers step of a team's turn: the next player's choice, or `None` once all chose.
pub(crate) fn next_question(state: &GameState) -> Option<Decision> {
    let player = next_picker(state)?;
    Some(Decision {
        player,
        kind: DecisionKind::DeclareAttackers(pick_options(state, player)),
    })
}

/// A player's choice: checked against their own options, recorded, and once it is the last the team's declaration.
pub(crate) fn answer(
    state: &GameState,
    player: PlayerId,
    options: &AttackOptions,
    attackers: Vec<(ObjectId, AttackTarget)>,
    extras: AttackExtras,
) -> Result<Outcome, EngineError> {
    check_attackers(options, &attackers)?;
    crate::w11a_exert::w11a_check_exert(options, &attackers, &extras.exert)?;
    crate::keywords::banding::check_bands(state, &attackers, &extras.bands)?;
    if !is_last(state, player) {
        return Ok(Outcome::Events(vec![Event::AttackersChosen {
            player,
            attackers,
            exert: extras.exert,
            bands: extras.bands,
        }]));
    }
    let mut all = chosen(state);
    all.extend(attackers.iter().copied());
    let mut extras_all = AttackExtras::default();
    for pick in &state.combat.attack_picks {
        extras_all.exert.extend(pick.exert.iter().copied());
        extras_all.bands.extend(pick.bands.iter().cloned());
    }
    extras_all.exert.extend(extras.exert);
    extras_all.bands.extend(extras.bands);
    declare(state, all, extras_all)
}

/// The team's one declaration, made by the active player: checked whole against the team's options; when it breaks a
/// rule the team chooses again, and after [`MOST_REFUSALS`] refusals the plain legal attack is declared.
fn declare(
    state: &GameState,
    attackers: Vec<(ObjectId, AttackTarget)>,
    extras: AttackExtras,
) -> Result<Outcome, EngineError> {
    let active = state.turn().active();
    let options = attack_options(state, active);
    let checked = check_attackers(&options, &attackers)
        .map_err(EngineError::from)
        .and_then(|()| {
            crate::w11a_exert::w11a_check_exert(&options, &attackers, &extras.exert)
                .map_err(EngineError::from)
        });
    match checked {
        Ok(()) => crate::attack_tax::declare(state, active, attackers, extras),
        Err(_) if state.combat.attack_refusals.len() >= MOST_REFUSALS => {
            crate::attack_tax::declare(
                state,
                active,
                options.legal_attack(),
                AttackExtras::default(),
            )
        }
        Err(error) => Ok(Outcome::Events(vec![Event::AttackChoicesRefused {
            reason: error.to_string(),
        }])),
    }
}
