//! Casting while searching (`GameRule::R7011CastWhileSearching`, the Panglacial Wurm rulings): before a player finds
//! cards in a search of their own library, they may cast, one at a time and paying their costs, the cards of that
//! library with the rule; a card cast is on the stack and so not found, and the search goes on once they choose none.
//! Each searcher who has such a card is asked in turn; one who has none is not asked, and the search runs as it would.

use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::refs::ZoneKind;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::announce::Granted;
use crate::choose::{CardsOptions, Choice, Target};
use crate::decision::{Decision, DecisionKind};
use crate::resolve::{Cast, Cx, Next, ResolveError, Run};
use crate::state::GameState;
use crate::zones::Zone;

/// Whether the card has the rule that lets it be cast from its owner's library while they search it.
fn casts_while_searching(state: &GameState, id: ObjectId) -> bool {
    state.object(id).is_some_and(|card| {
        crate::ability_reader::printed_rules(card.card()).any(|rule| {
            matches!(
                &**rule,
                Ability::Rule {
                    rule: GameRule::R7011CastWhileSearching,
                    from: ZoneKind::Library,
                    ..
                }
            )
        })
    })
}

fn paid() -> Granted {
    Granted {
        free: false,
        way: None,
        transformed: false,
        rather: None,
        spend_as: mtg_cards::modify::SpendAs::default(),
    }
}

/// The pass of casts that comes before the search of `searchers`, each about to search their own library: a question,
/// a cast, or the next pass of the step; `None` once no searcher is left to ask, and the search runs.
pub(super) fn r7011_cast_pass(
    cx: &Cx<'_>,
    searchers: &[PlayerId],
) -> Result<Option<Run>, ResolveError> {
    let state = cx.state();
    let at: Vec<String> = cx
        .resolving
        .position()
        .iter()
        .map(ToString::to_string)
        .collect();
    let at = at.join(".");
    let (tried_name, done_name) = (format!("?r7011 tried {at}"), format!("?r7011 done {at}"));
    let bound = |name: &str| -> Vec<Target> {
        cx.resolving
            .bindings()
            .get(name)
            .cloned()
            .unwrap_or_default()
    };
    let (tried, done) = (bound(&tried_name), bound(&done_name));
    let granted = paid();
    let offer = |player: PlayerId| -> Vec<ObjectId> {
        state
            .zone(Zone::Library(player))
            .iter()
            .copied()
            .filter(|id| casts_while_searching(state, *id))
            .filter(|id| {
                !tried
                    .iter()
                    .any(|t| t.object().is_some_and(|o| o.id == *id))
            })
            .filter(|id| crate::ways::grantable(state, player, *id, &granted))
            .collect()
    };
    // A card chosen, then cast or not: the card was tried, whatever the library holds now. No search asks a yes or no,
    // so the journal is this pass's.
    if let [(by, Choice::Cards(cards)), (again, Choice::YesNo(_))] = cx.journal()
        && by == again
        && searchers.contains(by)
    {
        let [card] = cards.as_slice() else {
            return Err(unexpected("a cast of no card"));
        };
        let mut grown = tried.clone();
        grown.push(Target::Object(crate::object::ObjectRef {
            id: *card,
            counter: state.object(*card).map_or(0, |card| card.counter()),
        }));
        return Ok(Some(Run::Done {
            events: Vec::new(),
            bound: vec![(tried_name, grown)],
            snapshots: Vec::new(),
            next: Next::Again,
        }));
    }
    // With nobody left to ask, the journal is the search's own.
    let Some((player, offered)) = searchers
        .iter()
        .filter(|player| !done.contains(&Target::Player(**player)))
        .map(|player| (*player, offer(*player)))
        .find(|(_, offered)| !offered.is_empty())
    else {
        return Ok(None);
    };
    let mut answers = cx.journal().iter();
    let mut finished = done.clone();
    finished.push(Target::Player(player));
    Ok(Some(match (answers.next(), answers.next()) {
        (None, _) => Run::Ask(Decision {
            player,
            kind: DecisionKind::ChooseCards(CardsOptions {
                source: cx.scope.source.id,
                from: offered,
                min: 0,
                max: 1,
                text: "Choose a card to cast from your library while you search it, or none"
                    .to_owned(),
                can_cancel: false,
                one_per_name: false,
                total: None,
                groups: Vec::new(),
                targeted: Box::default(),
            }),
        }),
        (Some((by, Choice::Cards(cards))), None) if *by == player => match cards.as_slice() {
            [] => Run::Done {
                events: Vec::new(),
                bound: vec![(done_name, finished)],
                snapshots: Vec::new(),
                next: Next::Again,
            },
            [card] if offered.contains(card) => Run::Cast(Cast {
                player,
                card: *card,
                free: false,
                way: None,
                transformed: false,
                rather: None,
                spend_as: mtg_cards::modify::SpendAs::default(),
            }),
            _ => return Err(unexpected("a card that is not offered")),
        },
        _ => return Err(unexpected("an answer that was not asked")),
    }))
}

fn unexpected(what: &str) -> ResolveError {
    ResolveError::Invalid(format!("{what} was given to a cast while searching"))
}
