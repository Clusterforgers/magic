//! Copies (CR 707): the copiable values of an object, the copy effect that gives them to a permanent, and the event
//! that begins it.
//!
//! A copy effect is the first layer (CR 613.1a): the permanent's card is replaced by the copiable values the effect
//! names, and every other effect (counters, auras, `becomes` effects, type changes) applies on top through the layers
//! as usual. Later copy effects replace earlier ones (CR 613.7). The values are a [`CardDefinition`], so the name,
//! mana cost, types, abilities, power, toughness and loyalty of the copy are read from it wherever the card is.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::choose::Choices;
use crate::error::EngineError;
use crate::events::Event;
use crate::fold::{invalid, reset_passes};
use crate::hash::stable_hash;
use crate::merged::Merged;
use crate::object::{GameObject, ObjectKind, ObjectRef, PermanentState};
use crate::snapshot::Snapshot;
use crate::state::{GameState, Tally};
use crate::zones::{Placement, Zone};

/// A copy effect on a permanent: the object it copies and the copiable values it gives (CR 707.2).
#[derive(Debug, Clone)]
pub struct CopyEffect {
    of: ObjectRef,
    definition: Arc<CardDefinition>,
    fingerprint: u64,
    timestamp: u64,
    /// When the effect ends: in the cleanup step ("until end of turn") or as a player's next turn begins; never for a
    /// lasting copy.
    ends: Option<C10XpCopyEnds>,
    /// The copy effect this temporary one is over, which applies again when it ends (CR 613.7).
    under: Option<Box<CopyEffect>>,
    /// What a merged permanent shows under the effect: the copiable values with the abilities of its other cards.
    merged: Option<Arc<CardDefinition>>,
}

impl CopyEffect {
    /// A copy of `of` with these copiable values, made at `timestamp`.
    pub(crate) fn new(of: ObjectRef, definition: Arc<CardDefinition>, timestamp: u64) -> Self {
        CopyEffect {
            of,
            fingerprint: stable_hash(&*definition),
            definition,
            timestamp,
            ends: None,
            under: None,
            merged: None,
        }
    }

    /// What the permanent shows: the copiable values, and for a merged permanent those with the abilities of its other
    /// cards (CR 729.2c).
    pub fn shown(&self) -> &CardDefinition {
        self.merged.as_deref().unwrap_or(&self.definition)
    }

    /// Works out what the permanent shows under this effect and the ones it is over, once `merged` is what it is made of.
    pub(crate) fn over(&mut self, merged: Option<&Merged>) {
        self.merged = merged.map(|cards| Arc::new(cards.under_copy(&self.definition)));
        if let Some(under) = self.under.as_mut() {
            under.over(merged);
        }
    }

    /// The same effect ending as `ends` says, over the copy effect `under` it (if any).
    fn temporary(mut self, ends: C10XpCopyEnds, under: Option<CopyEffect>) -> Self {
        self.ends = Some(ends);
        self.under = under.map(Box::new);
        self
    }

    /// Whether the effect ends at some point (with the turn, or as a player's next turn begins).
    pub const fn is_temporary(&self) -> bool {
        self.ends.is_some()
    }

    /// Whether this effect or a temporary one it is over ends at `ends`.
    pub fn c10_xp_ends_at(&self, ends: C10XpCopyEnds) -> bool {
        self.ends.is_some_and(|own| {
            own == ends
                || self
                    .under
                    .as_ref()
                    .is_some_and(|under| under.c10_xp_ends_at(ends))
        })
    }

    /// What the permanent is a copy of once the temporary effects that end at `ends` are gone; the others stay in their
    /// order (CR 613.7).
    fn without(mut self, ends: C10XpCopyEnds) -> Option<CopyEffect> {
        let rest = self.under.take().and_then(|under| under.without(ends));
        match self.ends {
            None => Some(self),
            Some(own) if own == ends => rest,
            Some(_) => {
                self.under = rest.map(Box::new);
                Some(self)
            }
        }
    }

    /// The object copied, as it was when the copy effect began.
    pub const fn of(&self) -> ObjectRef {
        self.of
    }

    /// The copiable values the effect gives, exceptions included (CR 707.9b).
    pub fn definition(&self) -> &CardDefinition {
        &self.definition
    }

    /// The copiable values, shared with the copy effect.
    pub(crate) fn shared(&self) -> Arc<CardDefinition> {
        Arc::clone(&self.definition)
    }

    /// The timestamp of the copy effect: a later one replaces an earlier one (CR 613.7).
    pub const fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl PartialEq for CopyEffect {
    fn eq(&self, other: &Self) -> bool {
        self.of == other.of
            && self.fingerprint == other.fingerprint
            && self.timestamp == other.timestamp
            && self.ends == other.ends
            && self.under == other.under
    }
}

impl Eq for CopyEffect {}

// The values hash once, when the effect is made.
impl Hash for CopyEffect {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.of.hash(state);
        self.fingerprint.hash(state);
        self.timestamp.hash(state);
        // Left out for a lasting effect, which hashes as it did before copies could end.
        if let Some(ends) = self.ends {
            true.hash(state);
            self.under.hash(state);
            if let C10XpCopyEnds::NextTurnOf(player) = ends {
                player.hash(state);
            }
        }
    }
}

/// When a temporary copy effect ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum C10XpCopyEnds {
    /// In the cleanup step of the turn ("until end of turn", CR 514.2).
    Cleanup,
    /// As this player's next turn begins ("until your next turn", CR 611.2b), or the next turn after the player left.
    NextTurnOf(PlayerId),
}

/// A permanent became a copy of another object (CR 707.2): its copiable values are now those of the copy effect, with a
/// timestamp of its own. It follows the move to the battlefield in the batch of a permanent that enters as a copy
/// (CR 614.1c), so the copy is what enters and what triggers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BecameCopy {
    /// The permanent that is the copy.
    pub object: ObjectId,
    /// The object it copies, as it was.
    pub of: ObjectRef,
    /// The copiable values, so a log folds to the same game with no card data at hand.
    #[cfg_attr(feature = "ts", ts(skip))]
    pub definition: Arc<CardDefinition>,
    /// The copy ends in the cleanup step of the turn, and the permanent is what it was before ("until end of turn").
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub until_end_of_turn: bool,
    /// The copy ends as this player's next turn begins, and the permanent is what it was before ("until your next turn").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub until_next_turn_of: Option<PlayerId>,
}

/// A spell was copied (CR 707.10): the copy is a spell on the stack above the original, controlled by the player who
/// copied it. It is not cast, and it ceases to exist when it leaves the stack, except onto the battlefield as a token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SpellCopy {
    /// The copy's object.
    pub object: ObjectId,
    /// The spell it copies, as it was.
    pub of: ObjectRef,
    /// Who controls the copy.
    pub controller: PlayerId,
    /// The copiable values of the spell, the face it is in use as and the exceptions applied.
    #[cfg_attr(feature = "ts", ts(skip))]
    pub definition: Arc<CardDefinition>,
    /// What the original announced (the modes, X, optional costs and targets), with the new targets the copy chose.
    #[serde(default, skip_serializing_if = "Choices::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Choices>", optional))]
    pub choices: Choices,
}

/// A card in exile was copied (CR 707.12): the copy is a token card in exile, owned by the player who made it, who may
/// cast it. It ceases to exist as soon as it is in any zone but the stack (CR 704.5e).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardCopy {
    /// The copy's object.
    pub object: ObjectId,
    /// The card it copies, as it was.
    pub of: ObjectRef,
    /// Who owns the copy.
    pub owner: PlayerId,
    /// The copiable values of the card.
    #[cfg_attr(feature = "ts", ts(skip))]
    pub definition: Arc<CardDefinition>,
}

/// An activated or triggered ability was copied (CR 707.10): the copy is an ability on the stack above the original,
/// controlled by the player who copied it. It is neither activated nor triggered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AbilityCopy {
    /// The copy's object.
    pub object: ObjectId,
    /// The ability it copies, as it was.
    pub of: ObjectRef,
    /// Who controls the copy.
    pub controller: PlayerId,
    /// What the original announced, with the new targets the copy chose.
    #[serde(default, skip_serializing_if = "Choices::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Choices>", optional))]
    pub choices: Choices,
}

/// Folds the event: the permanent takes the copy effect, which replaces any earlier one.
pub(crate) fn fold(state: &mut GameState, event: &BecameCopy) -> Result<(), EngineError> {
    let timestamp = state.next_timestamp;
    let object = state
        .objects
        .get_mut(&event.object)
        .ok_or_else(|| invalid(format!("{:?} does not exist", event.object)))?;
    if object.kind() == ObjectKind::Ability {
        return Err(invalid("an ability on the stack is not copied into"));
    }
    let permanent = object
        .permanent
        .as_mut()
        .ok_or_else(|| invalid(format!("{:?} is not a permanent", event.object)))?;
    let began = CopyEffect::new(event.of, Arc::clone(&event.definition), timestamp);
    // A lasting copy replaces the temporary ones under it too; a temporary one keeps what it is over (CR 613.7).
    let ends = match (event.until_end_of_turn, event.until_next_turn_of) {
        (_, Some(player)) => Some(C10XpCopyEnds::NextTurnOf(player)),
        (true, None) => Some(C10XpCopyEnds::Cleanup),
        (false, None) => None,
    };
    permanent.copy = Some(match ends {
        Some(ends) => began.temporary(ends, permanent.copy.take()),
        None => began,
    });
    if let Some(copy) = permanent.copy.as_mut() {
        copy.over(permanent.merged.as_ref());
    }
    state.next_timestamp += 1;
    note_definition(state, &event.definition);
    Ok(())
}

/// Folds the event: the copy is a new spell on top of the stack, above the one it copies (CR 707.10).
pub(crate) fn fold_spell(state: &mut GameState, copy: &SpellCopy) -> Result<(), EngineError> {
    let id = copy.object;
    if state.objects.contains_key(&id) || state.ceased.contains_key(&id) {
        return Err(invalid(format!("{id:?} already exists")));
    }
    if !state.is_alive(copy.controller) {
        return Err(invalid(format!(
            "{:?} is out of the game and cannot control a spell",
            copy.controller
        )));
    }
    if !on_the_stack(state, copy.of, false) {
        return Err(invalid(format!(
            "{:?} is not a spell on the stack",
            copy.of
        )));
    }
    for target in copy.choices.targeted_objects() {
        state.tally_object(target, Some(copy.controller), Tally::Targeted);
    }
    let mut spell = GameObject::new(
        id,
        copy.controller,
        Arc::clone(&copy.definition),
        Zone::Stack,
    );
    spell.kind = ObjectKind::Token;
    spell.choices.clone_from(&copy.choices);
    spell.stack_copy = Some(copy.of);
    note_definition(state, &copy.definition);
    state.objects.insert(id, spell);
    state.zones.insert(Zone::Stack, id, Placement::Top);
    state.next_object = state.next_object.max(id.get() + 1);
    reset_passes(state);
    Ok(())
}

/// Folds the event: the copy is a new token card on top of the exile zone, a copy of a card that lies there (CR 707.12).
pub(crate) fn fold_card(state: &mut GameState, copy: &CardCopy) -> Result<(), EngineError> {
    let id = copy.object;
    if state.objects.contains_key(&id) || state.ceased.contains_key(&id) {
        return Err(invalid(format!("{id:?} already exists")));
    }
    if !state.is_alive(copy.owner) {
        return Err(invalid(format!(
            "{:?} is out of the game and cannot own a copy",
            copy.owner
        )));
    }
    let copied = state.objects.get(&copy.of.id).is_some_and(|card| {
        card.counter() == copy.of.counter
            && ((card.zone() == Zone::Exile && card.kind() == ObjectKind::Card)
                || card.permanent().is_some_and(PermanentState::prepared))
    });
    if !copied {
        return Err(invalid(format!(
            "{:?} is not a card in exile or a prepared permanent",
            copy.of
        )));
    }
    let mut card = GameObject::new(id, copy.owner, Arc::clone(&copy.definition), Zone::Exile);
    card.kind = ObjectKind::Token;
    card.stack_copy = Some(copy.of);
    note_definition(state, &copy.definition);
    state.objects.insert(id, card);
    state.zones.insert(Zone::Exile, id, Placement::Top);
    state.next_object = state.next_object.max(id.get() + 1);
    Ok(())
}

/// Folds the event: a copy of a card that is anywhere but on the stack ceases to exist (CR 704.5e).
pub(crate) fn fold_card_ceased(state: &mut GameState, object: ObjectId) -> Result<(), EngineError> {
    let copy = state
        .objects
        .get(&object)
        .filter(|copy| copy.kind() == ObjectKind::Token && copy.copied_from().is_some())
        .ok_or_else(|| invalid(format!("{object:?} is not a copy of a card")))?;
    if matches!(copy.zone(), Zone::Stack | Zone::Battlefield) {
        return Err(invalid(format!(
            "{object:?} is in {:?}, where a copy of a card stays",
            copy.zone()
        )));
    }
    crate::tokens::cease(state, object);
    Ok(())
}

/// Folds the event: the copy is a new ability on top of the stack, above the one it copies (CR 707.10).
pub(crate) fn fold_ability(state: &mut GameState, copy: &AbilityCopy) -> Result<(), EngineError> {
    let id = copy.object;
    if state.objects.contains_key(&id) || state.ceased.contains_key(&id) {
        return Err(invalid(format!("{id:?} already exists")));
    }
    if !state.is_alive(copy.controller) {
        return Err(invalid(format!(
            "{:?} is out of the game and cannot control an ability",
            copy.controller
        )));
    }
    let original = state
        .objects
        .get(&copy.of.id)
        .filter(|_| on_the_stack(state, copy.of, true))
        .ok_or_else(|| invalid(format!("{:?} is not an ability on the stack", copy.of)))?;
    let ability = original
        .ability()
        .cloned()
        .ok_or_else(|| invalid(format!("{:?} is not an ability on the stack", copy.of)))?;
    let face = original.choices.face;
    let source = state.source_object(ability.source().id).ok_or_else(|| {
        invalid(format!(
            "the source {:?} of an ability is gone",
            ability.source()
        ))
    })?;
    let mut object =
        GameObject::on_stack(id, copy.controller, source, copy.choices.clone(), ability);
    object.choices.face = face;
    object.stack_copy = Some(copy.of);
    for target in copy.choices.targeted_objects() {
        state.tally_object(target, Some(copy.controller), Tally::Targeted);
    }
    state.objects.insert(id, object);
    state.zones.insert(Zone::Stack, id, Placement::Top);
    state.next_object = state.next_object.max(id.get() + 1);
    reset_passes(state);
    Ok(())
}

/// Whether `reference` names an ability (`ability`) or a spell on the stack, as it was named.
fn on_the_stack(state: &GameState, reference: ObjectRef, ability: bool) -> bool {
    state.object(reference.id).is_some_and(|object| {
        object.counter() == reference.counter
            && object.zone() == Zone::Stack
            && (object.kind() == ObjectKind::Ability) == ability
    })
}

/// Folds the event: a temporary copy effect ends, and the permanent is a copy of what it was a copy of before, if anything.
pub(crate) fn fold_ended(state: &mut GameState, ended: &Event) -> Result<(), EngineError> {
    let &Event::CopyEnded {
        object,
        next_turn_of,
    } = ended
    else {
        return Err(invalid("not the end of a copy"));
    };
    let ends = next_turn_of.map_or(C10XpCopyEnds::Cleanup, C10XpCopyEnds::NextTurnOf);
    let permanent = state
        .objects
        .get_mut(&object)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{object:?} is not a permanent")))?;
    match permanent.copy.take() {
        Some(copy) if copy.c10_xp_ends_at(ends) => {
            permanent.copy = copy.without(ends);
            Ok(())
        }
        other => {
            permanent.copy = other;
            Err(invalid(format!(
                "{object:?} is not under a copy effect that ends now"
            )))
        }
    }
}

/// Notes what a definition that enters the game in play can bring that no card of the game had: a triggered, static or
/// replacement ability, a legend, an attachment (`GameState::possible`).
pub(crate) fn note_definition(state: &mut GameState, definition: &CardDefinition) {
    if definition
        .supertypes
        .contains(&mtg_core::card::Supertype::Legendary)
    {
        state.possible.legends = true;
    }
    if crate::attach::definition_attaches(definition) {
        state.possible.attachments = true;
    }
    if crate::ability_reader::definition_triggers(definition) {
        state.triggers_possible = true;
    }
    if crate::ability_reader::definition_has_static(definition) {
        state.statics_possible = true;
    }
    if crate::ability_reader::definition_has_rule(definition) {
        state.rules_possible = true;
    }
    if crate::ability_reader::definition_has_state_trigger(definition) {
        state.possible.states = true;
    }
    if crate::ability_reader::definition_has_replacement(definition) {
        state.possible.replacements = true;
    }
    if crate::ability_reader::definition_has_cost_modifier(definition) {
        state.possible.costs = true;
    }
    if crate::ability_reader::definition_has_protection(definition) {
        state.possible.protection = true;
    }
}

/// Whether the object may be copied: an object of the game that is not an ability. A double-faced permanent is copied
/// as the face it shows (CR 707.8), so a transformed one is no exception.
pub(crate) fn may_be_copied(object: &GameObject) -> bool {
    object.kind() != ObjectKind::Ability
}

/// The copiable values of the object `reference` names (CR 707.2): what its card prints, or the values of the copy
/// effect it is under, as a card of one face. An object that has left since is read from `shot`, its last known
/// information (CR 707.2 and 608.2h); `None` when there is neither or when the object is an ability. The values of a
/// double-faced permanent are those of the face it shows, with no back face (CR 707.8, 712.8c).
pub(crate) fn copiable_values(
    state: &GameState,
    reference: ObjectRef,
    shot: Option<&Snapshot>,
) -> Option<CardDefinition> {
    if let Some(object) = state
        .object(reference.id)
        .filter(|object| object.counter() == reference.counter)
    {
        // A merged permanent has the copiable values of its topmost card alone, or of the copy effect it is under
        // (CR 729.2a).
        let printed = match (object.merged(), object.copy_effect()) {
            (_, Some(copy)) => copy.definition(),
            (Some(merged), None) => merged.top().card(),
            (None, None) => object.card(),
        };
        return may_be_copied(object).then(|| printed.copiable());
    }
    let shot = shot.filter(|shot| shot.reference() == reference)?;
    if let Some(copy) = &shot.copy {
        return Some(copy.copiable());
    }
    // A face-down permanent was the 2/2 of CR 708.2a, not its card.
    if let Some(shown) = shot
        .face_down
        .and_then(crate::object::FaceDown::characteristics)
    {
        return Some(shown.copiable());
    }
    let card = state
        .source_object(reference.id)
        .filter(|card| card.kind() != ObjectKind::Ability)?;
    let whole = card.whole_card();
    Some(whole.face(shot.face.unwrap_or(0)).copiable())
}

#[cfg(test)]
mod tests {
    //! Goal: a copy effect is the first layer: the copiable values are what an object prints, or what the copy it is under gives, and never what other effects made of it; every other effect (a pump, a set of power and toughness, a type) applies on top whatever the order of the timestamps, a later copy effect replaces an earlier one, a copy never changes the original or any other object, a copy that has left is read by last known information, and only a permanent can become a copy.
    use mtg_core::card::CardType;
    use mtg_core::ids::{EffectId, PlayerId};
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;
    use crate::characteristics::characteristics;
    use crate::effects::{ContinuousEffect, Duration, Modification, Selector};
    use crate::events::ZoneCause;
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::snapshot::snapshot;
    use crate::zones::Zone;

    const NAMES: [&str; 6] = ["Bear", "Wolf", "Ogre", "Copy A", "Copy B", "Copy C"];

    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![
                vec![
                    creature("Bear", 2, 2),
                    creature("Wolf", 4, 4),
                    creature("Ogre", 3, 5),
                    creature("Copy A", 1, 1),
                    creature("Copy B", 1, 1),
                    creature("Copy C", 1, 1),
                ],
                vec![],
            ],
        )
    }

    /// The object that starts the game as the card `name`, which may be a copy of something else later.
    fn start(name: &str) -> ObjectRef {
        let game = game();
        game.objects()
            .find(|object| object.card().name == name)
            .map(GameObject::reference)
            .unwrap()
    }

    fn become_copy(game: &mut GameState, object: &str, of: &str) {
        let of = start(of);
        let definition = copiable_values(game, of, None).unwrap();
        let event = Event::BecameCopy(BecameCopy {
            object: start(object).id,
            of,
            definition: Arc::new(definition),
            until_end_of_turn: false,
            until_next_turn_of: None,
        });
        apply_event(game, &event).unwrap();
    }

    fn effect(game: &mut GameState, object: &str, modification: Modification, timestamp: u64) {
        let id = EffectId(game.effects.len() as u64);
        game.effects.push(ContinuousEffect::new(
            id,
            Selector::Objects(vec![start(object)]),
            modification,
            Duration::UntilEndOfTurn,
            timestamp,
        ));
        game.layers.reset();
    }

    fn shown(game: &GameState, object: &str) -> (Option<i32>, Option<i32>, bool) {
        let chars = characteristics(game, start(object).id).unwrap();
        (
            chars.power,
            chars.toughness,
            chars.types.contains(CardType::Artifact),
        )
    }

    #[test]
    fn the_copiable_values_are_what_the_card_prints_and_not_what_effects_made_of_it() {
        let mut game = game();
        effect(
            &mut game,
            "Bear",
            Modification::ModifyPowerToughness {
                power: 3,
                toughness: 3,
            },
            5,
        );
        effect(
            &mut game,
            "Bear",
            Modification::AddType(CardType::Artifact),
            6,
        );
        assert_eq!(shown(&game, "Bear"), (Some(5), Some(5), true));
        let values = copiable_values(&game, start("Bear"), None).unwrap();
        assert_eq!((values.power, values.toughness), (Some(2), Some(2)));
        assert!(!values.types.contains(&CardType::Artifact));
        assert_eq!(values.faces, []);
    }

    /// What happens to Copy A, in order.
    #[derive(Clone, Copy)]
    enum Step {
        Become(&'static str),
        Pump(i32),
        Set(i32, i32),
        Artifact,
    }
    use Step::{Artifact, Become, Pump, Set};

    #[rstest]
    #[case::nothing(&[], (1, 1, false))]
    #[case::a_copy(&[Become("Wolf")], (4, 4, false))]
    #[case::the_later_copy_replaces_the_earlier(&[Become("Wolf"), Become("Ogre")], (3, 5, false))]
    #[case::a_pump_stays_on_top_of_a_later_copy(&[Pump(1), Become("Wolf")], (5, 5, false))]
    #[case::a_pump_stays_on_top_of_an_earlier_copy(&[Become("Wolf"), Pump(1)], (5, 5, false))]
    #[case::a_set_stays_on_top_of_a_later_copy(&[Set(2, 2), Become("Wolf")], (2, 2, false))]
    #[case::a_type_change_between_two_copies_stays(&[Become("Wolf"), Artifact, Become("Ogre")], (3, 5, true))]
    #[case::a_type_change_before_a_copy_stays(&[Artifact, Become("Bear")], (2, 2, true))]
    #[case::a_pump_and_a_set_after_two_copies(&[Become("Wolf"), Become("Bear"), Set(1, 1), Pump(2)], (3, 3, false))]
    fn a_copy_effect_comes_before_every_other_effect_whatever_the_timestamps(
        #[case] steps: &[Step],
        #[case] expected: (i32, i32, bool),
    ) {
        let mut game = game();
        for (at, step) in steps.iter().enumerate() {
            // The effects carry timestamps that run backwards, so a copy never wins by being later.
            let stamp = 100 - at as u64;
            match step {
                Become(of) => become_copy(&mut game, "Copy A", of),
                Pump(by) => effect(
                    &mut game,
                    "Copy A",
                    Modification::ModifyPowerToughness {
                        power: *by,
                        toughness: *by,
                    },
                    stamp,
                ),
                Set(power, toughness) => effect(
                    &mut game,
                    "Copy A",
                    Modification::SetPowerToughness {
                        power: *power,
                        toughness: *toughness,
                    },
                    stamp,
                ),
                Artifact => effect(
                    &mut game,
                    "Copy A",
                    Modification::AddType(CardType::Artifact),
                    stamp,
                ),
            }
        }
        assert_eq!(
            shown(&game, "Copy A"),
            (Some(expected.0), Some(expected.1), expected.2)
        );
    }

    #[test]
    fn a_later_copy_effect_has_a_later_timestamp_and_replaces_the_earlier() {
        let mut game = game();
        become_copy(&mut game, "Copy A", "Wolf");
        let a = start("Copy A").id;
        let stamp = |game: &GameState| game.object(a).unwrap().copy_effect().unwrap().timestamp();
        let before = stamp(&game);
        become_copy(&mut game, "Copy A", "Ogre");
        assert!(stamp(&game) > before);
        assert_eq!(game.object(a).unwrap().card().name, "Ogre");
    }

    #[test]
    fn a_copy_of_a_copy_has_the_values_the_first_copy_has() {
        let mut game = game();
        become_copy(&mut game, "Copy A", "Wolf");
        let values = copiable_values(&game, start("Copy A"), None).unwrap();
        assert_eq!((values.name.as_str(), values.power), ("Wolf", Some(4)));
        assert_eq!(values.faces, []);
    }

    #[test]
    fn only_a_permanent_becomes_a_copy_and_the_state_hash_sees_the_copy() {
        let mut game = game();
        let before = game.state_hash();
        become_copy(&mut game, "Copy A", "Wolf");
        assert_ne!(game.state_hash(), before);
        let a = start("Copy A").id;
        let left = Event::ZoneChanged {
            object: a,
            from: Zone::Battlefield,
            to: Zone::Graveyard(PlayerId(0)),
            cause: ZoneCause::Destroyed,
        };
        apply_event(&mut game, &left).unwrap();
        // The card leaves its copy effect behind (CR 400.7), and a card that is not on the battlefield becomes no copy.
        assert!(game.object(a).unwrap().copy_effect().is_none());
        assert_eq!(game.object(a).unwrap().card().name, "Copy A");
        let late = Event::BecameCopy(BecameCopy {
            object: a,
            of: start("Wolf"),
            definition: Arc::new(creature("Wolf", 4, 4).copiable()),
            until_end_of_turn: false,
            until_next_turn_of: None,
        });
        assert!(apply_event(&mut game, &late).is_err());
    }

    #[test]
    fn a_copy_that_has_left_is_read_by_last_known_information() {
        let mut game = game();
        become_copy(&mut game, "Copy A", "Wolf");
        let a = start("Copy A");
        let shot = snapshot(&game, a.id).unwrap();
        let left = Event::ZoneChanged {
            object: a.id,
            from: Zone::Battlefield,
            to: Zone::Graveyard(PlayerId(0)),
            cause: ZoneCause::Destroyed,
        };
        apply_event(&mut game, &left).unwrap();
        // The card Copy A is a new object now; the snapshot still says that what left was a Wolf.
        assert_ne!(game.object(a.id).unwrap().counter(), a.counter);
        let values = copiable_values(&game, a, Some(&shot)).unwrap();
        assert_eq!((values.name.as_str(), values.power), ("Wolf", Some(4)));
        assert!(copiable_values(&game, a, None).is_none());
    }

    fn become_copy_until_end_of_turn(game: &mut GameState, object: &str, of: &str) {
        let of = start(of);
        let definition = copiable_values(game, of, None).unwrap();
        let event = Event::BecameCopy(BecameCopy {
            object: start(object).id,
            of,
            definition: Arc::new(definition),
            until_end_of_turn: true,
            until_next_turn_of: None,
        });
        apply_event(game, &event).unwrap();
    }

    fn c10_xp_become_copy_until_next_turn(game: &mut GameState, object: &str, of: &str) {
        let of = start(of);
        let definition = copiable_values(game, of, None).unwrap();
        let event = Event::BecameCopy(BecameCopy {
            object: start(object).id,
            of,
            definition: Arc::new(definition),
            until_end_of_turn: false,
            until_next_turn_of: Some(PlayerId(0)),
        });
        apply_event(game, &event).unwrap();
    }

    #[rstest]
    #[case::the_cleanup_ends_the_copy_on_top(true, "Wolf")]
    #[case::the_next_turn_ends_the_copy_under(false, "Ogre")]
    fn c10_xp_each_end_takes_only_its_own_copies(
        #[case] cleanup_first: bool,
        #[case] after_first: &str,
    ) {
        let mut game = game();
        c10_xp_become_copy_until_next_turn(&mut game, "Copy A", "Wolf");
        become_copy_until_end_of_turn(&mut game, "Copy A", "Ogre");
        let end = |next_turn_of| Event::CopyEnded {
            object: start("Copy A").id,
            next_turn_of,
        };
        let (first, second) = if cleanup_first {
            (None, Some(PlayerId(0)))
        } else {
            (Some(PlayerId(0)), None)
        };
        apply_event(&mut game, &end(first)).unwrap();
        assert_eq!(name_of(&game, "Copy A"), after_first);
        assert!(apply_event(&mut game, &end(first)).is_err(), "ended once");
        apply_event(&mut game, &end(second)).unwrap();
        assert_eq!(name_of(&game, "Copy A"), "Copy A");
    }

    fn name_of(game: &GameState, object: &str) -> String {
        game.object(start(object).id).unwrap().card().name.clone()
    }

    #[rstest]
    #[case::over_nothing(&[], &["Wolf"], "Copy A", true)]
    #[case::over_a_lasting_copy(&["Ogre"], &["Wolf"], "Ogre", true)]
    #[case::over_two_temporary_ones(&["Ogre"], &["Wolf", "Bear"], "Ogre", true)]
    #[case::a_lasting_copy_made_meanwhile_stays(&["Ogre"], &["Wolf", "!Bear"], "Bear", false)]
    fn a_copy_until_end_of_turn_gives_way_to_what_the_permanent_was_when_it_ends(
        #[case] lasting: &[&str],
        #[case] then: &[&str],
        #[case] after: &str,
        #[case] ends: bool,
    ) {
        let mut game = game();
        for of in lasting {
            become_copy(&mut game, "Copy A", of);
        }
        let mut shown_first = None;
        for of in then {
            match of.strip_prefix('!') {
                Some(of) => become_copy(&mut game, "Copy A", of),
                None => become_copy_until_end_of_turn(&mut game, "Copy A", of),
            }
            shown_first.get_or_insert_with(|| name_of(&game, "Copy A"));
        }
        let last = then.last().unwrap().trim_start_matches('!');
        assert_eq!(
            name_of(&game, "Copy A"),
            last,
            "the latest copy effect applies"
        );
        let ended = Event::CopyEnded {
            object: start("Copy A").id,
            next_turn_of: None,
        };
        if !ends {
            // A lasting copy made over the temporary ones replaced them: there is nothing to end.
            assert!(apply_event(&mut game, &ended).is_err());
            assert_eq!(name_of(&game, "Copy A"), after);
            return;
        }
        apply_event(&mut game, &ended).unwrap();
        assert_eq!(name_of(&game, "Copy A"), after);
        assert_eq!(
            game.object(start("Copy A").id)
                .unwrap()
                .copy_effect()
                .is_some(),
            after != "Copy A"
        );
        assert!(
            apply_event(&mut game, &ended).is_err(),
            "nothing temporary is left to end"
        );
    }

    #[test]
    fn a_copy_of_a_transformed_permanent_has_the_values_of_the_face_that_is_up_and_no_back_face() {
        let front = mtg_cards::CardDefinition::new("Moth")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1);
        let back = mtg_cards::CardDefinition::new("Butterfly")
            .with_type(CardType::Creature)
            .with_power_toughness(4, 4);
        let dfc = Arc::new(
            front
                .clone()
                .with_faces(mtg_cards::definition::Layout::Transform, vec![front, back]),
        );
        let mut game = crate::fixtures::state(
            vec![vec![], vec![]],
            vec![vec![dfc, creature("Copy A", 1, 1)], vec![]],
        );
        let moth = game
            .objects()
            .find(|o| o.card().name == "Moth")
            .map(GameObject::reference)
            .unwrap();
        let front_values = copiable_values(&game, moth, None).unwrap();
        assert_eq!(front_values.name, "Moth");
        apply_event(
            &mut game,
            &Event::Transformed {
                object: moth.id,
                face: Some(1),
            },
        )
        .unwrap();
        let shot = snapshot(&game, moth.id).unwrap();
        for values in [
            copiable_values(&game, moth, None).unwrap(),
            copiable_values(&game, moth, Some(&shot)).unwrap(),
        ] {
            assert_eq!(
                (values.name.as_str(), values.power, values.faces.len()),
                ("Butterfly", Some(4), 0)
            );
        }
        // A copy of it keeps the back face's values and does not turn over with the original.
        let copy = game
            .objects()
            .find(|o| o.card().name == "Copy A")
            .map(GameObject::reference)
            .unwrap();
        let event = Event::BecameCopy(BecameCopy {
            object: copy.id,
            of: moth,
            definition: Arc::new(copiable_values(&game, moth, None).unwrap()),
            until_end_of_turn: false,
            until_next_turn_of: None,
        });
        apply_event(&mut game, &event).unwrap();
        assert_eq!(game.object(copy.id).unwrap().card().name, "Butterfly");
        assert!(!may_be_copied_wrongly(&game, copy));
    }

    #[test]
    fn a_copy_of_a_flipped_permanent_has_the_values_of_the_bottom_half_and_cannot_flip() {
        let cost = mtg_core::mana::ManaCost::from_symbols([mtg_core::mana::ManaSymbol::Generic(2)]);
        let top = mtg_cards::CardDefinition::new("Rat")
            .with_type(CardType::Creature)
            .with_mana_cost(cost.clone())
            .with_power_toughness(2, 1);
        let bottom = mtg_cards::CardDefinition::new("Nightrat")
            .with_type(CardType::Creature)
            .with_mana_cost(cost)
            .with_power_toughness(4, 2);
        let flip = Arc::new(
            top.clone()
                .with_faces(mtg_cards::definition::Layout::Flip, vec![top, bottom]),
        );
        let mut game = crate::fixtures::state(
            vec![vec![], vec![]],
            vec![vec![flip, creature("Copy A", 1, 1)], vec![]],
        );
        let rat = game
            .objects()
            .find(|o| o.card().name == "Rat")
            .map(GameObject::reference)
            .unwrap();
        apply_event(&mut game, &Event::Flipped { object: rat.id }).unwrap();
        let values = copiable_values(&game, rat, None).unwrap();
        assert_eq!(
            (values.name.as_str(), values.power, values.faces.len()),
            ("Nightrat", Some(4), 0)
        );
        assert_eq!(values.mana_value(), 2, "the bottom half keeps the cost");
        let copy = game
            .objects()
            .find(|o| o.card().name == "Copy A")
            .map(GameObject::reference)
            .unwrap();
        let event = Event::BecameCopy(BecameCopy {
            object: copy.id,
            of: rat,
            definition: Arc::new(values),
            until_end_of_turn: false,
            until_next_turn_of: None,
        });
        apply_event(&mut game, &event).unwrap();
        assert_eq!(game.object(copy.id).unwrap().card().name, "Nightrat");
        assert!(!may_be_copied_wrongly(&game, copy));
        assert!(apply_event(&mut game, &Event::Flipped { object: copy.id }).is_err());
    }

    /// Whether the copy is a transforming permanent: it never is.
    fn may_be_copied_wrongly(game: &GameState, copy: ObjectRef) -> bool {
        game.object(copy.id).unwrap().whole_card().is_multi_face()
    }

    proptest! {
        #[test]
        fn a_copy_never_changes_the_original_or_an_object_that_is_not_copied(
            moves in proptest::collection::vec((3usize..6, 0usize..3), 0..12),
        ) {
            let mut game = game();
            let before: Vec<_> = NAMES.iter().map(|name| shown(&game, name)).collect();
            let mut last = [None; 3];
            for (target, source) in &moves {
                become_copy(&mut game, NAMES[*target], NAMES[*source]);
                last[*target - 3] = Some(*source);
            }
            for (at, name) in NAMES.iter().enumerate().take(3) {
                prop_assert_eq!(shown(&game, name), before[at], "the original {} is untouched", name);
            }
            for (at, copied) in last.iter().enumerate() {
                let (power, toughness, _) = shown(&game, NAMES[3 + at]);
                // A copy is what its last source is; one that was never made a copy is still what it prints.
                let want = copied.map_or((Some(1), Some(1)), |source| (before[source].0, before[source].1));
                prop_assert_eq!((power, toughness), want);
            }
        }
    }
}
