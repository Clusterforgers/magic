//! The modes of a modal spell or ability (CR 601.2b, 700.2).
//!
//! A mode is offered only when each of its target specifications has enough legal targets; with fewer legal modes than
//! the fewest to choose, the announcement is rewound. A spell with entwine asks its optional costs first, and when the
//! entwine cost is paid every mode is chosen without a question (CR 702.42). An ability that picks its modes at random
//! chooses among the modes that could be chosen with the next substream of the game's random source, and no player is
//! asked; one that chooses a mode not yet chosen this turn, or ever, leaves out the modes its tally holds.

use super::{Ctx, Next, What, Work, optional, price};
use mtg_core::ids::PlayerId;

use crate::choose::{Modal, ModeOptions, modal_of, mode_options};
use crate::decision::{Decision, DecisionKind};
use crate::object::{GameObject, ObjectRef};
use crate::resolve::Scope;
use crate::rng::GameRng;

pub(super) fn run(ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
    if work.done.modes {
        return None;
    }
    if let Some(mut modal) = ctx.ability.and_then(modal_of) {
        if optional::entwines(ctx) {
            if let Some(ask) = optional::run(ctx, work) {
                return Some(ask);
            }
            if optional::entwined(ctx, work.choices) {
                work.choices.modes = (0..modal.modes.len()).collect();
                work.done.modes = true;
                return None;
            }
        }
        modal.used = chosen_before(ctx, modal.once == crate::choose::C11dOnce::Ever);
        let Some(mut options) = mode_options(
            ctx.state,
            ctx.source,
            ctx.player,
            &modal,
            ctx.can_cancel() && decider(ctx, &modal) == ctx.player,
            ctx.cause(),
        ) else {
            return Some(Next::Drop);
        };
        if !price::restrict(ctx, work.choices, &mut options) {
            return Some(Next::Drop);
        }
        if modal.at_random {
            return pick(ctx, work, &modal, &options);
        }
        // Nothing to choose between when no mode may be chosen.
        if options.max > 0 {
            return Some(ask(ctx, &modal, options));
        }
    }
    work.done.modes = true;
    None
}

/// The question for the modes, put to the player the modal names, and to the controller when that is no one in the game
/// (the table never waits on a seat that has left).
fn ask(ctx: &Ctx<'_>, modal: &Modal<'_>, options: ModeOptions) -> Next {
    Next::Ask(Decision {
        player: decider(ctx, modal),
        kind: DecisionKind::ChooseMode(options),
    })
}

/// Who chooses the modes: the first player `who` names, in turn order from the controller, who is still in the game.
fn decider(ctx: &Ctx<'_>, modal: &Modal<'_>) -> PlayerId {
    if modal.who.is_you() {
        return ctx.player;
    }
    let origin = ctx.state.object(ctx.source).map_or(
        ObjectRef {
            id: ctx.source,
            counter: 0,
        },
        GameObject::reference,
    );
    let named = Scope::announcing(ctx.state, ctx.player, origin)
        .players(modal.who)
        .unwrap_or_default();
    ctx.state
        .turn_order_from(ctx.player)
        .into_iter()
        .find(|seat| named.contains(seat))
        .unwrap_or(ctx.player)
}

/// The modes this ability chose earlier in the turn, or in any turn for one that chooses a mode not yet chosen; none
/// for a spell.
fn chosen_before(ctx: &Ctx<'_>, ever: bool) -> Vec<usize> {
    let key = match ctx.what {
        What::Trigger(waiting) => {
            let t = &waiting.trigger;
            Some((t.source, t.origin, t.ability))
        }
        What::Ability(address) => ctx
            .state
            .object(address.object)
            .map(|source| (source.reference(), address.source, address.ability)),
        What::Spell(_) | What::Attack(..) => None,
    };
    let Some((source, origin, ability)) = key else {
        return Vec::new();
    };
    let tally = if ever {
        &ctx.state.modes_ever
    } else {
        &ctx.state.tallies.modes
    };
    tally
        .iter()
        .filter(|(from, via, at, _)| (*from, *via, *at) == (source, origin, ability))
        .map(|(_, _, _, mode)| *mode)
        .collect()
}

/// The modes drawn at random among those offered: as many as the ability chooses, or all of them when fewer can be
/// chosen. The choices say so, and the event that puts the ability on the stack spends the substream.
fn pick(
    ctx: &Ctx<'_>,
    work: &mut Work<'_>,
    modal: &Modal<'_>,
    options: &crate::choose::ModeOptions,
) -> Option<Next> {
    let mut left: Vec<usize> = options.modes.iter().map(|mode| mode.index).collect();
    let wanted = usize::try_from(modal.choose.fixed_min().unwrap_or(1)).unwrap_or(usize::MAX);
    GameRng::substream(ctx.state.seed, ctx.state.rng_index).shuffle(&mut left);
    left.truncate(wanted);
    left.sort_unstable();
    work.choices.modes = left;
    work.choices.random_modes = true;
    work.done.modes = true;
    None
}
