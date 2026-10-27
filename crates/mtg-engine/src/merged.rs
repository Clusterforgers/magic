//! Mutate and merged permanents (CR 702.140, 729): a creature spell cast for its mutate cost targets a non-Human creature with
//! the same owner and merges with it as it resolves, and the result is one object made of several cards.
//!
//! The merged permanent is the object it was before: it keeps its id, counters, attachments and status. The spell's card
//! leaves the game's objects and lives in `PermanentState::merged` until the permanent leaves the battlefield, where each
//! card becomes an object of its own again in the new zone (CR 729.3, 400.7). The permanent has the characteristics of the
//! topmost card and the abilities of every card, in the order they merged, so an ability keeps its place when another
//! card merges.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable};
use mtg_cards::value::CastWay;
use mtg_core::card::{CardType, Subtype};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::choose::{Target, YesNoChoice, YesNoOptions, legal_targets};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::fold::invalid;
use crate::hash::stable_hash;
use crate::object::{GameObject, ObjectKind, PermanentState};
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// One card (or token) of a merged permanent.
#[derive(Debug, Clone)]
pub struct Component {
    id: ObjectId,
    definition: Arc<CardDefinition>,
    face: Option<u8>,
    kind: ObjectKind,
    counter: u32,
    over: bool,
}

impl Component {
    /// The card as the object it was before it merged: its id, and the card with the face it was in use as.
    fn of(object: &GameObject, over: bool) -> Self {
        Component {
            id: object.id,
            definition: Arc::clone(&object.def),
            face: object.face(),
            kind: object.kind,
            counter: object.counter,
            over,
        }
    }

    /// The object id the card has, and gets again when it is an object of its own.
    pub const fn id(&self) -> ObjectId {
        self.id
    }

    /// The card with the face it is in use as.
    pub fn card(&self) -> &CardDefinition {
        match self.face {
            Some(face) => self.definition.face(face),
            None => &self.definition,
        }
    }

    /// Whether the merged permanent is a token of this card (a token component ceases to exist when it leaves).
    pub fn is_token(&self) -> bool {
        self.kind == ObjectKind::Token
    }
}

/// The cards a merged permanent is made of, in the order they merged: the card the permanent was first.
#[derive(Debug, Clone)]
pub struct Merged {
    components: Vec<Component>,
    definition: Arc<CardDefinition>,
    fingerprint: u64,
    /// The cards that go to another zone than the permanent when it leaves, with that zone (`Event::CardParted`).
    parted: Vec<(ObjectId, Zone)>,
}

impl Merged {
    /// A permanent of one card, which another is about to merge with.
    fn start(object: &GameObject) -> Self {
        Merged::of(vec![Component::of(object, false)])
    }

    fn of(components: Vec<Component>) -> Self {
        let definition = Arc::new(derive(&components));
        Merged {
            fingerprint: stable_hash(&*definition),
            components,
            definition,
            parted: Vec::new(),
        }
    }

    /// The permanent with `card` merged in, on top of the others or under them.
    fn with(&self, card: Component) -> Self {
        let mut components = self.components.clone();
        components.push(card);
        Merged::of(components)
    }

    /// The characteristics of the topmost card with the abilities of every card.
    pub fn definition(&self) -> &CardDefinition {
        &self.definition
    }

    /// The same, shared.
    pub(crate) fn shared(&self) -> Arc<CardDefinition> {
        Arc::clone(&self.definition)
    }

    /// The topmost card: the last that went over the others, else the card the permanent was.
    pub fn top(&self) -> &Component {
        top_of(&self.components)
    }

    /// The cards from the top down: those put over the others, the latest first, then the card the permanent was and
    /// those put under it.
    pub fn from_the_top(&self) -> Vec<&Component> {
        let over = self.components.iter().rev().filter(|c| c.over);
        let under = self.components.iter().filter(|c| !c.over);
        over.chain(under).collect()
    }

    /// How many times the permanent has mutated: the cards merged into it, less the one it was (CR 702.140).
    pub fn mutations(&self) -> u32 {
        u32::try_from(self.others().len()).unwrap_or(u32::MAX)
    }

    /// What the permanent is under a copy effect that gives the copiable values `copy`: they stand in the place of the
    /// topmost card's, and the abilities of the other cards stay (CR 729.2c).
    pub(crate) fn under_copy(&self, copy: &CardDefinition) -> CardDefinition {
        let top = self
            .components
            .iter()
            .rposition(|card| card.over)
            .unwrap_or(0);
        let cards = self.components.iter().enumerate();
        copy.clone()
            .with_abilities_of(cards.map(|(at, card)| if at == top { copy } else { card.card() }))
    }

    /// The ids of the cards other than the one the permanent was that become objects of their own when it leaves the
    /// battlefield; a token among them ceases to exist (CR 111.7, 729.3).
    fn apart(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.others()
            .iter()
            .filter(|card| !card.is_token())
            .map(Component::id)
    }

    /// The cards other than the one the permanent was, in the order they merged.
    fn others(&self) -> &[Component] {
        self.components.get(1..).unwrap_or_default()
    }

    /// The zone `card` goes to in place of the permanent's when the permanent leaves, if one was set.
    fn parted(&self, card: ObjectId) -> Option<Zone> {
        self.parted
            .iter()
            .find(|(id, _)| *id == card)
            .map(|(_, zone)| *zone)
    }
}

impl PartialEq for Merged {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
            && self.components.len() == other.components.len()
            && self.components.iter().zip(&other.components).all(|(a, b)| {
                (a.id, a.counter, a.over, a.kind) == (b.id, b.counter, b.over, b.kind)
            })
            && self.parted == other.parted
    }
}

impl Eq for Merged {}

impl Hash for Merged {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.fingerprint.hash(state);
        for card in &self.components {
            (card.id, card.counter, card.over).hash(state);
        }
        // Hashed only when set, so a game without it hashes as before.
        if !self.parted.is_empty() {
            self.parted.hash(state);
        }
    }
}

fn top_of(components: &[Component]) -> &Component {
    components
        .iter()
        .rev()
        .find(|card| card.over)
        .unwrap_or(&components[0])
}

/// What the permanent is: the topmost card's characteristics and every card's abilities (CR 729.2a, 702.140d).
fn derive(components: &[Component]) -> CardDefinition {
    top_of(components)
        .card()
        .copiable()
        .with_abilities_of(components.iter().map(Component::card))
}

/// The target of a creature spell cast for its mutate cost: a non-Human creature with the same owner (CR 702.140a).
pub(crate) fn target_spec() -> TargetSpec {
    TargetSpec {
        what: Targetable::Object(Selector(vec![
            Pred::Type(CardType::Creature),
            Pred::Not(Box::new(Pred::Subtype(Subtype("Human".to_owned())))),
            Pred::OwnedBy(Ref::OwnerOf(Box::new(Ref::This))),
        ])),
        bounds: Bounds::default(),
        divide: None,
    }
}

/// Whether `player` can announce a creature for the card `id` cast for its mutate cost.
pub(crate) fn announceable(state: &GameState, player: PlayerId, id: ObjectId) -> bool {
    !legal_targets(state, &target_spec(), id, player).is_empty()
}

/// The creature the mutating creature spell on the stack merges with, when it is still a legal target as the spell
/// resolves; otherwise the spell resolves as a creature spell (CR 702.140b).
pub(crate) fn merge_target(state: &GameState, spell: ObjectId) -> Option<ObjectId> {
    let spell = state.object(spell)?;
    if spell.zone() != Zone::Stack
        || spell.kind() == ObjectKind::Ability
        || spell.choices().way != Some(CastWay::Mutate)
    {
        return None;
    }
    let Target::Object(target) = *spell.choices().targets.first()?.first()? else {
        return None;
    };
    legal_targets(state, &target_spec(), spell.id(), spell.controller())
        .contains(&Target::Object(target))
        .then_some(target.id)
}

/// The question of the player whose mutating creature spell is about to resolve: on top of the creature or under it
/// (CR 702.140b); asked when every player has passed in turn and the target is still legal.
pub(crate) fn question(state: &GameState) -> Option<Decision> {
    let spell = state.object(*state.stack().last()?)?;
    let onto = state.object(merge_target(state, spell.id())?)?;
    Some(Decision {
        player: spell.controller(),
        kind: DecisionKind::YesNo(YesNoOptions::new(
            spell.id(),
            format!(
                "Put {} on top of {}? No puts it under.",
                spell.card().name,
                onto.card().name
            ),
        )),
    })
}

/// Whether a yes-or-no question outside any resolution is the one about a mutating creature spell.
pub(crate) fn asks(state: &GameState, options: &YesNoOptions) -> bool {
    question(state).is_some_and(|decision| {
        matches!(&decision.kind, DecisionKind::YesNo(asked) if asked.source == options.source)
    })
}

/// The answer: yes merges the spell over the creature, no under it.
pub(crate) fn answer(state: &GameState, options: &YesNoOptions, choice: YesNoChoice) -> Outcome {
    let (Some(spell), Some(onto)) = (
        state.object(options.source),
        merge_target(state, options.source),
    ) else {
        return Outcome::Events(Vec::new());
    };
    Outcome::Events(vec![Event::Mutated {
        object: onto,
        spell: spell.id(),
        over: choice.0,
        player: spell.controller(),
    }])
}

/// Folds `Mutated`: the spell leaves the stack and its card joins the permanent, which lies face up from now on
/// (CR 729.2).
pub(crate) fn fold(
    state: &mut GameState,
    permanent: ObjectId,
    spell: ObjectId,
    over: bool,
) -> Result<(), EngineError> {
    if !state.zones.get(Zone::Stack).contains(&spell) {
        return Err(invalid(format!("{spell:?} is not on the stack")));
    }
    if !state.zones.get(Zone::Battlefield).contains(&permanent) {
        return Err(invalid(format!("{permanent:?} is not on the battlefield")));
    }
    let card = state
        .objects
        .remove(&spell)
        .ok_or_else(|| invalid(format!("unknown object {spell:?}")))?;
    state.zones.remove(Zone::Stack, spell);
    let host = state
        .objects
        .get_mut(&permanent)
        .ok_or_else(|| invalid(format!("unknown object {permanent:?}")))?;
    let added = Component::of(&card, over);
    let before = host.permanent.as_ref().and_then(|p| p.merged.clone());
    let start = before.unwrap_or_else(|| Merged::start(host));
    let Some(held) = host.permanent.as_mut() else {
        return Err(invalid(format!("{permanent:?} is no permanent")));
    };
    let merged = start.with(added);
    if let Some(copy) = held.copy.as_mut() {
        copy.over(Some(&merged));
    }
    held.merged = Some(merged);
    host.face_down = None;
    crate::fold::priority_to_active(state);
    Ok(())
}

/// The ids of the cards that leave with `object` as objects of their own when it is a merged permanent, else none.
pub(crate) fn cards_apart(object: &GameObject) -> Vec<ObjectId> {
    object
        .permanent()
        .and_then(PermanentState::merged)
        .map_or_else(Vec::new, |merged| merged.apart().collect())
}

/// The commander cards of `object` with their zone-change counters when it is a merged permanent (the card it was among
/// them), else none.
pub(crate) fn commander_cards(state: &GameState, object: &GameObject) -> Vec<(ObjectId, u32)> {
    let Some(merged) = object.permanent().and_then(PermanentState::merged) else {
        return Vec::new();
    };
    let others = merged
        .others()
        .iter()
        .filter(|card| !card.is_token())
        .map(|card| (card.id, card.counter));
    std::iter::once((object.id(), object.counter()))
        .chain(others)
        .filter(|(id, _)| state.is_commander(*id))
        .collect()
}

/// The name of the card `card` of the merged permanent `object`.
pub(crate) fn card_name(object: &GameObject, card: ObjectId) -> Option<&str> {
    object
        .permanent()
        .and_then(PermanentState::merged)?
        .components
        .iter()
        .find(|part| part.id == card)
        .map(|part| part.card().name.as_str())
}

/// Folds `Event::CardParted`: the card `card` of the merged permanent `permanent` goes to `to` when the permanent leaves.
pub(crate) fn part(
    state: &mut GameState,
    permanent: ObjectId,
    card: ObjectId,
    to: Zone,
) -> Result<(), EngineError> {
    if !state.zones.exists(to) || matches!(to, Zone::Battlefield | Zone::Stack) {
        return Err(invalid(format!("{card:?} cannot part to {to:?}")));
    }
    let merged = state
        .objects
        .get_mut(&permanent)
        .and_then(|object| object.permanent.as_mut())
        .and_then(|held| held.merged.as_mut())
        .ok_or_else(|| invalid(format!("{permanent:?} is no merged permanent")))?;
    if !merged.others().iter().any(|part| part.id == card) {
        return Err(invalid(format!("{card:?} is no card of {permanent:?}")));
    }
    merged.parted.retain(|(id, _)| *id != card);
    merged.parted.push((card, to));
    Ok(())
}

/// What the permanent that left the battlefield was made of, besides its own card: the cards that become objects of their
/// own, each with the zone set for it in place of the permanent's, if any.
pub(crate) fn taken(permanent: &mut Option<PermanentState>) -> Vec<(Component, Option<Zone>)> {
    permanent
        .as_mut()
        .and_then(|p| p.merged.take())
        .map_or_else(Vec::new, |merged| {
            merged
                .others()
                .iter()
                .map(|card| (card.clone(), merged.parted(card.id)))
                .collect()
        })
}

/// Puts the cards of a merged permanent that left into `to`, or into the zone set for a card, as objects of their own
/// owned by `owner` (CR 729.3).
pub(crate) fn scatter(
    state: &mut GameState,
    cards: Vec<(Component, Option<Zone>)>,
    owner: PlayerId,
    to: Zone,
    placement: Placement,
) {
    for (card, parted) in cards {
        // A token of the merged permanent ceases to exist (CR 111.7).
        if card.is_token() {
            continue;
        }
        let to = parted.unwrap_or(to);
        let mut object = GameObject::new(card.id, owner, Arc::clone(&card.definition), to);
        object.counter = card.counter + 1;
        state.zones.insert(to, card.id, placement);
        state.commanders.moved(card.id);
        if to == Zone::Battlefield {
            object.permanent = Some(PermanentState::entering(
                state.next_timestamp,
                Vec::new(),
                None,
                None,
                None,
                Vec::new(),
            ));
            state.next_timestamp += 1;
        }
        state.objects.insert(card.id, object);
    }
}
