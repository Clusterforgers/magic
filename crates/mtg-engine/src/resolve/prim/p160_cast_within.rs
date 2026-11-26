//! `Effect::P160CastWithin`: a player casts spells from among a set of cards one at a time, in the order they choose,
//! while the spells cast stay within a count and a total of mana values (CR 608.2g).

use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::announce::Granted;
use crate::choose::{CardsOptions, Choice, Target};
use crate::decision::{Decision, DecisionKind};
use crate::resolve::{Cast, Cx, LET, Next, ResolveError, Run};
use crate::ways;

/// Each pass asks the player for one card among those `from` names that can be cast now and fit what is left of the
/// budget, or none; a card chosen is cast (`Run::Cast`), and once it is on the stack, or could not be cast, the effect
/// runs again (`Next::Again`) with the card tried. A pass with nothing to offer, or no card chosen, ends the effect.
/// The cards tried are bound under a name of the step, and the spells cast and their mana values, as the cards were
/// before the cast, are kept numbers of the resolution, so a game rebuilt from its log asks the same questions.
#[allow(clippy::too_many_lines)] // the budget, the offer, the cast and the pass after it
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::P160CastWithin {
        who,
        from,
        most,
        total,
        free,
    } = effect
    else {
        return Err(wrong_effect("P160CastWithin"));
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(Run::events(Vec::new()));
    };
    let names = Names::of(cx.resolving.position());
    let tried: Vec<Target> = cx
        .resolving
        .bindings()
        .get(&names.tried)
        .cloned()
        .unwrap_or_default();
    let kept = |name: &str| {
        cx.resolving
            .lets
            .iter()
            .find(|(held, _)| held == name)
            .map_or(0, |(_, value)| *value)
    };
    let (cast, spent) = (kept(&names.cast), kept(&names.spent));
    let room = match total {
        Some(amount) => Some(cx.scope.amount(amount)? - spent),
        None => None,
    };
    let more = match most {
        Some(amount) => cx.scope.amount(amount)? > cast,
        None => true,
    };
    let granted = Granted {
        free: *free,
        way: None,
        transformed: false,
        rather: None,
        spend_as: mtg_cards::modify::SpendAs::default(),
    };
    let offered: Vec<(ObjectId, i64)> = if more {
        cx.scope
            .live_objects(from)?
            .into_iter()
            .filter(|card| {
                !tried
                    .iter()
                    .any(|t| t.object().is_some_and(|o| o.id == card.id()))
            })
            .map(|card| (card.id(), i64::from(card.mana_value())))
            .filter(|(_, value)| room.is_none_or(|room| *value <= room))
            .filter(|(id, _)| ways::grantable(state, player, *id, &granted))
            .collect()
    } else {
        Vec::new()
    };
    let mut answers = cx.journal().iter();
    match (answers.next(), answers.next()) {
        (None, _) if offered.is_empty() => Ok(Run::events(Vec::new())),
        (None, _) => Ok(Run::Ask(question(cx, player, &offered, room, *free))),
        (Some((by, Choice::Cards(cards))), None) if *by == player => match cards.as_slice() {
            [] => Ok(Run::events(Vec::new())),
            [card] if offered.iter().any(|(id, _)| id == card) => Ok(Run::Cast(Cast {
                player,
                card: *card,
                free: *free,
                way: None,
                transformed: false,
                rather: None,
                spend_as: mtg_cards::modify::SpendAs::default(),
            })),
            _ => Err(unexpected("a card that is not offered")),
        },
        (Some((by, Choice::Cards(cards))), Some((again, Choice::YesNo(was_cast))))
            if *by == player && *again == player =>
        {
            let [card] = cards.as_slice() else {
                return Err(unexpected("a cast of no card"));
            };
            // The card as it was in exile: the whole card, whichever face was cast.
            let value = state
                .object(*card)
                .map_or(0, |card| i64::from(card.whole_card().mana_value()));
            let reference = state
                .object(*card)
                .map(crate::object::GameObject::reference)
                .ok_or_else(|| unexpected("a card that is gone"))?;
            let mut grown = tried.clone();
            grown.push(Target::Object(reference));
            let mut bound = vec![(names.tried.clone(), grown)];
            if *was_cast {
                bound.push((format!("{LET}{}:{}", cast + 1, names.cast), Vec::new()));
                bound.push((
                    format!("{LET}{}:{}", spent + value, names.spent),
                    Vec::new(),
                ));
            }
            Ok(Run::Done {
                events: Vec::new(),
                bound,
                snapshots: Vec::new(),
                next: Next::Again,
            })
        }
        _ => Err(unexpected("an answer that was not asked")),
    }
}

/// The names the effect keeps its passes under, per position in the resolution.
struct Names {
    /// The cards offered and chosen so far.
    tried: String,
    /// How many spells were cast.
    cast: String,
    /// The mana values of the spells cast, added up.
    spent: String,
}

impl Names {
    fn of(position: &[usize]) -> Names {
        let at: Vec<String> = position.iter().map(ToString::to_string).collect();
        let at = at.join(".");
        Names {
            tried: format!("?p160 tried {at}"),
            cast: format!("?p160 cast {at}"),
            spent: format!("?p160 spent {at}"),
        }
    }
}

/// "Choose a spell to cast without paying its mana cost (total mana value 3 or less), or none."
fn question(
    cx: &Cx<'_>,
    player: PlayerId,
    offered: &[(ObjectId, i64)],
    room: Option<i64>,
    free: bool,
) -> Decision {
    let paying = if free {
        " without paying its mana cost"
    } else {
        ""
    };
    let budget = room.map_or_else(String::new, |room| {
        format!(" (mana value {} or less)", room.max(0))
    });
    Decision {
        player,
        kind: DecisionKind::ChooseCards(CardsOptions {
            source: cx.scope.source.id,
            from: offered.iter().map(|(id, _)| *id).collect(),
            min: 0,
            max: 1,
            text: format!("Choose a spell to cast{paying}{budget}, or none"),
            can_cancel: false,
            one_per_name: false,
            total: None,
            groups: Vec::new(),
            targeted: Box::default(),
        }),
    }
}

fn unexpected(what: &str) -> ResolveError {
    ResolveError::Invalid(format!("{what} was given to a cast within a budget"))
}
