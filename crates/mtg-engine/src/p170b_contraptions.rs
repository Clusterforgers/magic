//! Contraptions (Unstable and its release notes; the CR has no rule for them): the Contraption deck a seat brings, face
//! down in its command zone and shuffled before the game; assembling a Contraption, which puts the top card of that
//! deck onto the battlefield on one of its controller's three sprockets; the sprocket marker, which moves to the next
//! sprocket as its player's precombat main phase begins while they control a Contraption, after which they crank the
//! Contraptions they choose on that sprocket; and the scrapheap, the face-up pile in the command zone a Contraption card
//! goes to in place of a hand, a library or a graveyard.

use std::collections::{BTreeMap, BTreeSet};

use mtg_core::card::Subtype;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::characteristics::seen_as;
use crate::choose::{CardsChoice, CardsOptions, check_cards};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::fold::invalid;
use crate::object::GameObject;
use crate::state::GameState;
use crate::zones::Zone;

/// The number of sprockets each player has.
pub const SPROCKETS: u8 = 3;

/// The Contraptions of a game: each seat's Contraption deck (top card last), every Contraption card a seat brought,
/// each player's sprocket marker and the crank that asks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Contraptions {
    pub(crate) decks: BTreeMap<PlayerId, Vec<ObjectId>>,
    pub(crate) cards: BTreeSet<ObjectId>,
    pub(crate) markers: BTreeMap<PlayerId, u8>,
    /// The player who chooses the Contraptions to crank on the sprocket their marker just moved to.
    pub(crate) cranking: Option<PlayerId>,
}

impl Contraptions {
    /// Whether no seat plays with Contraptions.
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Puts the card `id` on top of `owner`'s Contraption deck (setup).
    pub(crate) fn add(&mut self, owner: PlayerId, id: ObjectId) {
        self.decks.entry(owner).or_default().push(id);
        self.cards.insert(id);
    }
}

/// The Contraption subtype.
fn contraption() -> Subtype {
    Subtype("Contraption".to_owned())
}

impl GameState {
    /// The cards of the player's Contraption deck, top card last.
    pub fn contraption_deck(&self, player: PlayerId) -> &[ObjectId] {
        self.p170b.decks.get(&player).map_or(&[], Vec::as_slice)
    }

    /// Whether the object lies in a Contraption deck.
    pub fn in_contraption_deck(&self, id: ObjectId) -> bool {
        self.p170b.decks.values().any(|deck| deck.contains(&id))
    }

    /// The sprocket the player's marker is on; `None` before it first moves.
    pub fn sprocket_marker(&self, player: PlayerId) -> Option<u8> {
        self.p170b.markers.get(&player).copied()
    }

    /// The sprocket the permanent is on: an assembled Contraption.
    pub fn sprocket(&self, id: ObjectId) -> Option<u8> {
        self.object(id)
            .and_then(GameObject::permanent)
            .and_then(crate::object::PermanentState::sprocket)
    }
}

/// Whether the permanent is a Contraption now: it prints the subtype or an effect gave it.
fn is_contraption(state: &GameState, object: &GameObject) -> bool {
    seen_as(state, object.id())
        .is_some_and(|(chars, _)| chars.has_subtype(&object.card().subtypes, &contraption()))
}

/// The Contraptions on the battlefield the player controls.
fn controlled(state: &GameState, player: PlayerId) -> impl Iterator<Item = &GameObject> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(move |object| object.controller() == player && is_contraption(state, object))
}

/// The Contraptions the player controls on `sprocket`: the ones they may crank.
fn on_sprocket(state: &GameState, player: PlayerId, sprocket: u8) -> Vec<ObjectId> {
    controlled(state, player)
        .filter(|object| state.sprocket(object.id()) == Some(sprocket))
        .map(GameObject::id)
        .collect()
}

/// The shuffle of every Contraption deck before the game begins, `offset` being the first substream of the batch left
/// to them; a seat without one shuffles nothing.
pub(crate) fn shuffles(state: &GameState, offset: u64) -> Vec<Event> {
    state
        .p170b
        .decks
        .iter()
        .filter(|(_, deck)| !deck.is_empty())
        .zip(offset..)
        .map(
            |((player, deck), offset)| Event::P170bContraptionDeckShuffled {
                player: *player,
                order: crate::mulligan::shuffled(state, offset, deck.clone()),
            },
        )
        .collect()
}

/// The top `count` cards of the player's Contraption deck, the top card first; a deck runs out quietly.
pub(crate) fn tops(state: &GameState, player: PlayerId, count: usize) -> Vec<ObjectId> {
    state
        .contraption_deck(player)
        .iter()
        .rev()
        .take(count)
        .copied()
        .collect()
}

/// The events of assembling `contraption` onto `sprocket`: its move onto the battlefield under the player's control,
/// then its assignment.
pub(crate) fn assemble_events(player: PlayerId, contraption: ObjectId, sprocket: u8) -> [Event; 2] {
    [
        Event::ZoneChanged {
            object: contraption,
            from: Zone::Command(player),
            to: Zone::Battlefield,
            cause: ZoneCause::Moved,
        },
        Event::P170bAssembled {
            player,
            contraption,
            sprocket,
        },
    ]
}

/// The sprocket a marker moves to from `at`: the first, then each in turn and back to the first.
const fn next_sprocket(at: Option<u8>) -> u8 {
    match at {
        Some(at) => at % SPROCKETS + 1,
        None => 1,
    }
}

/// The turn-based move of the active player's sprocket marker as their precombat main phase begins while they control
/// a Contraption, and whether a crank asks after it. Nothing while they control none.
pub(crate) fn precombat(state: &GameState) -> Option<(Event, bool)> {
    let active = state.active_player()?;
    controlled(state, active).next()?;
    let sprocket = next_sprocket(state.sprocket_marker(active));
    let asks = !on_sprocket(state, active, sprocket).is_empty();
    Some((
        Event::P170bSprocketMoved {
            player: active,
            sprocket,
        },
        asks,
    ))
}

/// The question of the crank while it asks: which Contraptions on the sprocket the marker is on the player cranks.
pub(crate) fn crank_question(state: &GameState) -> Option<Decision> {
    let player = state.p170b.cranking?;
    let sprocket = state.sprocket_marker(player)?;
    let from = on_sprocket(state, player, sprocket);
    let source = *from.first()?;
    Some(Decision {
        player,
        kind: DecisionKind::ChooseCards(CardsOptions {
            one_per_name: false,
            source,
            max: u32::try_from(from.len()).unwrap_or(u32::MAX),
            from,
            min: 0,
            text: format!("Choose the Contraptions on sprocket {sprocket} to crank."),
            can_cancel: false,
            total: None,
            groups: Vec::new(),
            targeted: Box::default(),
        }),
    })
}

/// Whether a choice of cards outside any resolution is the question of the crank: it asks, and over its Contraptions.
pub(crate) fn asks_crank(state: &GameState, options: &CardsOptions) -> bool {
    crank_question(state).is_some_and(|decision| {
        matches!(decision.kind, DecisionKind::ChooseCards(asked) if asked.from == options.from)
    })
}

/// The answer to the crank: the Contraptions chosen are cranked, then the active player receives priority.
pub(crate) fn crank_answer(
    state: &GameState,
    options: &CardsOptions,
    choice: &CardsChoice,
) -> Result<Outcome, EngineError> {
    check_cards(options, &choice.0)?;
    let player = state
        .p170b
        .cranking
        .ok_or_else(|| invalid("a crank answered while none asks"))?;
    let sprocket = state.sprocket_marker(player).unwrap_or(1);
    let contraptions = options
        .from
        .iter()
        .copied()
        .filter(|id| choice.0.contains(id))
        .collect();
    Ok(Outcome::Events(vec![
        Event::P170bCranked {
            player,
            sprocket,
            contraptions,
        },
        Event::PriorityGranted {
            player: state.active_or_next(),
        },
    ]))
}

/// Folds a shuffle of a Contraption deck: the same cards in a new order.
pub(crate) fn fold_shuffled(
    state: &mut GameState,
    player: PlayerId,
    order: &[ObjectId],
) -> Result<(), EngineError> {
    let deck = state.p170b.decks.entry(player).or_default();
    let mut same = order.to_vec();
    same.sort_unstable();
    let mut held = deck.clone();
    held.sort_unstable();
    if same != held {
        return Err(invalid(format!(
            "{player:?} shuffles other cards into their Contraption deck"
        )));
    }
    deck.clone_from(&order.to_vec());
    Ok(())
}

/// Folds an assembly: the Contraption goes on its sprocket. A card a replacement kept off the battlefield is on none.
pub(crate) fn fold_assembled(
    state: &mut GameState,
    contraption: ObjectId,
    sprocket: u8,
) -> Result<(), EngineError> {
    if !(1..=SPROCKETS).contains(&sprocket) {
        return Err(invalid(format!("sprocket {sprocket} does not exist")));
    }
    if let Some(perm) = state
        .objects
        .get_mut(&contraption)
        .and_then(|object| object.permanent.as_mut())
    {
        perm.designations.sprocket = Some(sprocket);
    }
    Ok(())
}

/// Folds a move of the sprocket marker: the crank asks while a Contraption of the player is on the new sprocket.
pub(crate) fn fold_moved(state: &mut GameState, player: PlayerId, sprocket: u8) {
    state.p170b.markers.insert(player, sprocket);
    if !on_sprocket(state, player, sprocket).is_empty() {
        state.p170b.cranking = Some(player);
    }
}

/// Folds a crank: it no longer asks.
pub(crate) fn fold_cranked(state: &mut GameState) {
    state.p170b.cranking = None;
}

/// A card that leaves the command zone leaves its owner's Contraption deck.
pub(crate) fn fold_left(state: &mut GameState, object: ObjectId, from: Zone) {
    if let Zone::Command(owner) = from
        && let Some(deck) = state.p170b.decks.get_mut(&owner)
    {
        deck.retain(|id| *id != object);
    }
}

/// Whether the object is a Contraption card a seat brought, which goes to the scrapheap.
pub(crate) fn is_contraption_card(state: &GameState, object: ObjectId) -> bool {
    state.p170b.cards.contains(&object)
}

#[cfg(test)]
mod tests {
    //! Goal: the sprocket marker moves to 1 first, then 1, 2, 3 and back to 1; each seat with a Contraption deck shuffles
    //! it once before the game, its cards face down in its command zone, and a seat without one shuffles nothing.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use crate::events::Event;
    use crate::fixtures::{creature, setup};
    use crate::state::GameState;

    #[rstest]
    #[case::one_deck(vec![3, 0])]
    #[case::both(vec![2, 1])]
    #[case::none(vec![0, 0])]
    fn p170b_each_deck_is_shuffled_once(#[case] sizes: Vec<usize>) {
        let mut table = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
        for (seat, size) in sizes.iter().enumerate() {
            table.players[seat].contraptions = (0..*size)
                .map(|n| creature(&format!("Gizmo {seat} {n}"), 0, 0))
                .collect();
        }
        let game = GameState::from_setup(&table).unwrap();
        let events = super::shuffles(&game, 0);
        assert_eq!(events.len(), sizes.iter().filter(|size| **size > 0).count());
        for event in events {
            let Event::P170bContraptionDeckShuffled { player, mut order } = event else {
                panic!("{event:?}");
            };
            let mut deck = game.contraption_deck(player).to_vec();
            deck.sort_unstable();
            order.sort_unstable();
            assert_eq!(order, deck);
        }
        for seat in 0..2 {
            let player = PlayerId::from_index(seat);
            for id in game.contraption_deck(player) {
                let card = game.object(*id).unwrap();
                assert_eq!(card.zone(), crate::zones::Zone::Command(player));
                assert_eq!(card.face_down, Some(crate::object::FaceDown::Contraption));
            }
        }
    }

    #[rstest]
    #[case(None, 1)]
    #[case(Some(1), 2)]
    #[case(Some(2), 3)]
    #[case(Some(3), 1)]
    fn p170b_the_marker_moves_round(#[case] at: Option<u8>, #[case] next: u8) {
        assert_eq!(super::next_sprocket(at), next);
    }
}
