//! `Effect::CastFromZone`: a player may cast a card as part of a resolution (CR 608.2g), or play it (CR 305.1, 702.75a).

use mtg_cards::effect::Effect;
use mtg_cards::value::CastWay;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use super::may::performed_name;
use super::wrong_effect;
use crate::choose::{Choice, Target, YesNoOptions};
use crate::decision::{Decision, DecisionKind};
use crate::events::Event;
use crate::object::{GameObject, ObjectKind};
use crate::priority::may_play_land_while_resolving;
use crate::resolve::{Cast, Cx, Next, ResolveError, Run};
use crate::ways;
use crate::wording;

/// The player the effect names is asked whether to cast the card, unless nothing lets them: a card that is not where it
/// was named, a land (unless the effect plays it), a card with no mana cost to pay (when it is not cast free), a spell
/// with no target to announce or a cost the player cannot pay is not cast, and nobody is asked. A yes begins the
/// announcement of the spell (`Run::Cast`): its modes, targets, X, costs and payment are asked like those of any cast, with
/// no way back, and the spell goes on the stack above the resolving object. The `otherwise` effects follow when the
/// player declines or the announcement cannot go on.
///
/// An effect that plays the card (`play`) puts a land onto the battlefield as a land play, but only while the player
/// could play one: on their turn, in a main phase, with nothing on the stack but the resolving ability and a land play
/// left. Any other card is cast.
///
/// A step with k questions runs k + 1 times. The journal holds the yes or no to the first question and, after a yes, the
/// answer the announcement logs along with the events of the cast: a yes when the spell was cast and a no when it could not
/// be. Nothing else is logged for the announcement, so a game rebuilt from its log in the middle of one asks it again.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::CastFromZone {
        who,
        what,
        free,
        play,
        way,
        ..
    } = effect
    else {
        return Err(wrong_effect("CastFromZone"));
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(finished(cx, false));
    };
    let mut answers = cx.journal().iter();
    let next = answers.next();
    let after = answers.next();
    if answers.next().is_some() {
        return Err(unexpected("a third answer"));
    }
    let offered = |card: &&GameObject| {
        if *play && is_land(card) {
            card.zone() != crate::zones::Zone::Battlefield
                && may_play_land_while_resolving(state, player, cx.resolving.object())
        } else {
            ways::grantable(state, player, card.id(), *free, *way)
        }
    };
    match (next, after) {
        (None, _) => {
            let card = cx.scope.live_objects(what)?.into_iter().find(offered);
            Ok(card.map_or_else(
                || finished(cx, false),
                |card| {
                    let alternative = ways::alternative_cost(state, card.id(), *way);
                    Run::Ask(question(
                        player,
                        card.card().name.as_str(),
                        card.id(),
                        *free,
                        *play,
                        alternative.as_ref().map(|(way, cost)| (*way, cost)),
                    ))
                },
            ))
        }
        (Some((who, Choice::YesNo(false))), None) if *who == player => Ok(finished(cx, false)),
        (Some((who, Choice::YesNo(true))), None) if *who == player => {
            let card = cx
                .scope
                .live_objects(what)?
                .into_iter()
                .find(offered)
                .ok_or_else(|| unexpected("a cast of a card that is gone"))?;
            if *play && is_land(card) {
                return Ok(Run::Done {
                    events: vec![Event::LandPlayed {
                        player,
                        object: card.id(),
                        face: None,
                    }],
                    bound: vec![performed(cx, true)],
                    snapshots: Vec::new(),
                    next: Next::Sibling,
                });
            }
            Ok(Run::Cast(Cast {
                player,
                card: card.id(),
                free: *free,
                way: *way,
            }))
        }
        (Some((first, Choice::YesNo(true))), Some((second, Choice::YesNo(cast))))
            if *first == player && *second == player =>
        {
            Ok(finished(cx, *cast))
        }
        _ => Err(unexpected("an answer that was not asked")),
    }
}

/// Whether the card is a land card, which is played and not cast.
fn is_land(card: &GameObject) -> bool {
    card.kind() == ObjectKind::Card && card.base_types().contains(CardType::Land)
}

/// Whether the card was cast or played, bound under the name a following "if you do" reads (`may::performed_name`): the
/// controller when it was, nothing when it was not.
fn performed(cx: &Cx<'_>, did: bool) -> (String, Vec<Target>) {
    let who = if did {
        vec![Target::Player(cx.scope.controller)]
    } else {
        Vec::new()
    };
    (performed_name(cx.resolving.position()), who)
}

/// The step is done: on to the next effect when the spell was cast, into `otherwise` when it was not.
fn finished(cx: &Cx<'_>, cast: bool) -> Run {
    Run::Done {
        events: Vec::new(),
        bound: vec![performed(cx, cast)],
        snapshots: Vec::new(),
        next: if cast { Next::Sibling } else { Next::Enter(0) },
    }
}

fn unexpected(what: &str) -> ResolveError {
    ResolveError::Invalid(format!("{what} was given to the cast of a resolution"))
}

/// "Cast Lightning Bolt without paying its mana cost?", "Play Forest without paying its mana cost?"
fn question(
    player: PlayerId,
    name: &str,
    card: ObjectId,
    free: bool,
    play: bool,
    alternative: Option<(CastWay, &ManaCost)>,
) -> Decision {
    let text = if play {
        wording::play(name, free)
    } else {
        wording::cast(name, free, alternative)
    };
    Decision {
        player,
        kind: DecisionKind::YesNo(YesNoOptions::new(card, text)),
    }
}
