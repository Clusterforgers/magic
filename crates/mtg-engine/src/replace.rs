//! The replacement step: a stop in front of the fold for the events that "would" happen (CR 614 to 616).
//!
//! Every batch of events the engine is about to apply goes through [`process`]. The replacement effects (the
//! `Ability::Replacement` abilities that `ability_reader` lists for the permanents in play, and the ones an entering
//! object prints) are tried against each event in turn:
//!
//! - an effect applies to an event at most once, to the events that replace it included (CR 614.5);
//! - a self-replacement effect (one about its own object) applies before the others (CR 614.15, 616.1a);
//! - when two or more effects apply, the affected player, or the controller of the affected object, picks the one
//!   that applies first with a `ChooseReplacement` decision (CR 616.1); the rest still apply, one after another, to
//!   what the first one left (CR 616.2);
//! - an event that is replaced entirely does not happen, so it is not in the result;
//! - a shield (CR 615) is a replacement effect an effect made for a while: a prevention shield takes up to a given
//!   amount of the damage of the events it covers, and when the events of one affected player share what it has left
//!   that player picks the one it takes next (`ChooseOption`); a regeneration shield replaces the next destruction of its
//!   permanent before anything else applies to it. Damage that can't be prevented (`GameRule::DamageCantBePrevented`) is
//!   passed by every prevention effect and shield.
//!
//! The step is a pure function of the state, the proposed events and the answers given so far. A question the step
//! needs answered (the order, a choice made as a permanent enters) comes back as a decision; the caller keeps the
//! proposed events, adds the answer to the journal and runs the step again, which asks the same questions in the same
//! order. A game with no replacement effect pays one flag.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::definition::Layout;
use mtg_cards::modify::Modification;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::{EntersChoice, ReplaceBy, ReplaceWhen};
use mtg_cards::value::{CastWay, CounterKind};
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::abilities::AbilityRef;
use crate::ability_reader::{abilities_of, provided_here, working_here};
use crate::announce::payments;
use crate::characteristics::controller_of_object;
use crate::choose::{
    CardsOptions, Choice, ChoiceOption, OptionOptions, ReplacementCandidate, ReplacementOptions,
    Target, YesNoOptions,
};
use crate::combat::DamageTarget;
use crate::copies::{BecameCopy, copiable_values, may_be_copied};
use crate::counters::CountersChanged;
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::{DeathCause, Event, ResolutionOutcome, ZoneCause};
use crate::keywords::{self, Holder, OptionalKind};
use crate::object::{Chosen, GameObject, ObjectKind, ObjectRef};
use crate::resolve::{Scope, zone_kind};
use crate::rules::{DamageShield, Rule, RuleEffect, damage_cant_be_prevented};
use crate::state::GameState;
use crate::tokens::TokenCreated;
use crate::turn::Step;
use crate::zones::Zone;

/// What the replacement step made of the proposed events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Processed {
    /// The events that happen.
    Events(Vec<Event>),
    /// A player has to answer first; the proposed events come back so that the caller can keep them for the rerun.
    Ask {
        /// The question.
        decision: Decision,
        /// The events the step was given.
        proposed: Vec<Event>,
    },
}

/// Passes the proposed events through the replacement effects that apply to them in `state`. `journal` holds the answers
/// given so far to the questions of this very batch, in the order they were asked.
pub fn process(
    state: &GameState,
    proposed: Vec<Event>,
    journal: &[(PlayerId, Choice)],
) -> Result<Processed, EngineError> {
    if !(state.possible.replacements || state.possible.protection) || proposed.is_empty() {
        return Ok(Processed::Events(proposed));
    }
    // A card that enters as one of its faces (a land played as one, a card put onto the battlefield transformed)
    // enters as that face, so the effects that apply to its entering are the face's (CR 614.12, 712.14a): the
    // replacement step reads the card as it will be, not as it lies in the hand.
    let view = crate::faces::entering_as(state, &proposed);
    let state = &*view;
    let sources = gather(state);
    let shields = shields_of(state);
    if sources.is_empty()
        && shields.is_empty()
        && !state.possible.protection
        && !proposed
            .iter()
            .any(|event| entering(state, event).is_some())
        && !proposed.iter().any(|event| {
            let item = Item::One(event.clone());
            leaves_stack_exiled(state, &item)
                || returns_by_buyback(state, &item)
                || exiles_by_adventure(state, &item)
                || exiles_by_rebound(state, &item)
                || commander_leaves(state, event)
        })
    {
        return Ok(Processed::Events(proposed));
    }
    let original = proposed.clone();
    let left = shields
        .iter()
        .filter_map(|effect| match &effect.rule {
            Rule::PreventDamage(DamageShield {
                left: Some(left), ..
            }) => Some((effect.id, *left)),
            _ => None,
        })
        .collect();
    let mut step = Pass {
        state,
        sources,
        shields,
        left,
        touched: Vec::new(),
        spent: BTreeSet::new(),
        answers: journal.iter(),
        next_id: state.next_object().get(),
        slots: slots_of(proposed),
        draws_changed: false,
        pulled: Vec::new(),
        activated: Vec::new(),
    };
    step.activated = step
        .slots
        .iter()
        .filter_map(|slot| match &slot.item {
            Item::One(Event::AbilityActivated { source, .. }) => Some(source.id),
            _ => None,
        })
        .collect();
    step.next_id = step
        .slots
        .iter()
        .filter_map(|slot| match &slot.item {
            Item::Tokens(run) => run.iter().map(|token| token.object.get()).max(),
            Item::One(_) => None,
        })
        .fold(step.next_id, |all, id| all.max(id + 1));
    match step.run()? {
        Run::Ask(decision) => Ok(Processed::Ask {
            decision,
            proposed: original,
        }),
        Run::Done => Ok(Processed::Events(step.finish())),
    }
}

/// The source `dealer` and the player who controls what `target` is, when protection of the target from the dealer
/// prevents the damage (CR 702.16e).
fn protection_of(
    state: &GameState,
    dealer: ObjectId,
    target: DamageTarget,
) -> Option<(ObjectRef, PlayerId)> {
    if !state.possible.protection {
        return None;
    }
    let source = keywords::Source::live(state, dealer)?;
    let (holder, controller) = match target {
        DamageTarget::Player(player) => (Holder::Player(player), player),
        DamageTarget::Object(object) => (
            Holder::Object(object),
            controller_of_object(state, state.object(object)?),
        ),
    };
    keywords::protected_from(state, holder, source).then_some((source.object, controller))
}

/// Whether combat damage is being dealt: the damage steps with nothing resolving.
pub(crate) fn in_combat_damage(state: &GameState) -> bool {
    state.resolving().is_none()
        && matches!(
            state.turn().step(),
            Step::FirstStrikeDamage | Step::CombatDamage
        )
}

/// One replaceable thing in a batch: an event, or all the tokens one effect makes for a player (they are one event).
#[derive(Debug, Clone)]
enum Item {
    One(Event),
    Tokens(Vec<TokenCreated>),
}

/// An item with the effects already applied to it and to the events it came from (CR 614.5).
#[derive(Debug, Clone)]
struct Slot {
    item: Item,
    applied: Vec<Origin>,
    /// Nothing applies to it any more: it carries out an event that an earlier effect already redirected.
    frozen: bool,
}

impl Slot {
    fn new(item: Item) -> Slot {
        Slot {
            item,
            applied: Vec::new(),
            frozen: false,
        }
    }
}

/// The batch as slots: a run of `TokenCreated` for one controller is a single slot.
fn slots_of(events: Vec<Event>) -> Vec<Slot> {
    let mut slots: Vec<Slot> = Vec::with_capacity(events.len());
    for event in events {
        match event {
            Event::TokenCreated(token) => match slots.last_mut() {
                Some(Slot {
                    item: Item::Tokens(run),
                    ..
                }) if run[0].controller == token.controller => run.push(token),
                _ => slots.push(Slot::new(Item::Tokens(vec![token]))),
            },
            other => slots.push(Slot::new(Item::One(other))),
        }
    }
    slots
}

/// Where a replacement effect comes from: an ability of an object, or a shield that an effect made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Origin {
    Ability(AbilityRef),
    /// The dredge of a card in a graveyard, which its owner may decline for one draw and be asked about again for another.
    Dredge(AbilityRef),
    /// The rule at this index among the copiable values of a card that enters as a copy.
    Copied(usize),
    Shield(EffectId),
    /// The protection of the permanent or player the damage is dealt to (CR 702.16e).
    Protection,
    /// The spell cast for its flashback cost, or the half with aftermath cast from a graveyard, which is exiled instead of
    /// going anywhere else from the stack (CR 702.34a, 702.127a).
    Flashback,
    /// The spell that resolved with its buyback cost paid, which returns to its owner's hand instead of going to the
    /// graveyard (CR 702.27a).
    Buyback,
    /// The spell with rebound that resolved after it was cast from a hand, which is exiled instead of going to its
    /// owner's graveyard and cast from exile at the next upkeep (CR 702.88a).
    Rebound,
    /// The spell that resolved as an Adventure, which is exiled instead of going to its owner's graveyard (CR 715.4).
    Adventure,
    /// A commander that would be put into a hand or a library, which its owner may put into the command zone instead
    /// (CR 903.9b).
    Commander,
}

impl Origin {
    /// The index of the ability among its object's rules that the question of the order shows; 0 for what is no ability.
    const fn ability_index(self) -> usize {
        match self {
            Origin::Ability(address) | Origin::Dredge(address) => address.ability,
            Origin::Copied(index) => index,
            _ => 0,
        }
    }

    /// The shield this is, if it is one.
    const fn shield(self) -> Option<EffectId> {
        match self {
            Origin::Shield(id) => Some(id),
            _ => None,
        }
    }
}

/// Whether the event puts a commander into a hand or a library (a draw included), where its owner may send it to the
/// command zone.
fn commander_leaves(state: &GameState, event: &Event) -> bool {
    state.commanders.any()
        && match event {
            Event::CardDrawn { object, .. } => state.is_commander(*object),
            _ => moving(state, event).is_some_and(|(object, _, to)| {
                matches!(to, Zone::Hand(_) | Zone::Library(_)) && state.is_commander(object)
            }),
        }
}

/// Whether the item is the resolution of an instant or sorcery whose buyback cost was paid: a spell that was countered
/// or did not resolve goes to the graveyard.
fn returns_by_buyback(state: &GameState, item: &Item) -> bool {
    let Item::One(event @ Event::SpellResolved { object }) = item else {
        return false;
    };
    entering(state, event).is_none()
        && state.object(*object).is_some_and(|spell| {
            // A copy of a spell is no card to return, and ceases to exist as it resolves.
            spell.kind() == ObjectKind::Card
                && payments(spell.card(), spell.paid_costs(), OptionalKind::Buyback) > 0
        })
}

/// Whether the item is the resolution of an Adventure, which is exiled instead of going to the graveyard (CR 715.4).
fn exiles_by_adventure(state: &GameState, item: &Item) -> bool {
    let Item::One(event @ Event::SpellResolved { object }) = item else {
        return false;
    };
    entering(state, event).is_none()
        && state.object(*object).is_some_and(|spell| {
            spell.kind() == ObjectKind::Card
                && spell.whole_card().layout == Layout::Adventure
                && spell.face() == Some(1)
        })
}

/// Whether the item is the resolution of a spell with rebound that was cast from its owner's hand, which is exiled in
/// place of going to the graveyard (CR 702.88a).
fn exiles_by_rebound(state: &GameState, item: &Item) -> bool {
    let Item::One(event @ Event::SpellResolved { object }) = item else {
        return false;
    };
    entering(state, event).is_none()
        && state.object(*object).is_some_and(|spell| {
            // A copy of a spell is no card, and was not cast from a hand.
            spell.kind() == ObjectKind::Card
                && spell.cast_from_hand()
                && keywords::keywords_of(state, spell.id())
                    .iter()
                    .any(|keyword| matches!(keyword, mtg_cards::keyword::Keyword::Rebound))
        })
}

/// Whether the item moves a spell cast for its flashback cost or with jump-start, or the half of a split card with
/// aftermath that was cast from a graveyard, from the stack to any zone but exile.
fn leaves_stack_exiled(state: &GameState, item: &Item) -> bool {
    let Item::One(event) = item else {
        return false;
    };
    moving(state, event).is_some_and(|(object, from, to)| {
        from == Zone::Stack
            && to != Zone::Exile
            && state.object(object).is_some_and(|spell| {
                spell.kind() == ObjectKind::Card
                    && (spell.choices().way.is_some_and(CastWay::exiles_the_spell)
                        || (spell.cast_from_graveyard() && keywords::aftermath::has(state, object)))
            })
    })
}

/// The shields in play: prevention and regeneration shields that still last, in the order they began.
fn shields_of(state: &GameState) -> Vec<RuleEffect> {
    state
        .rule_effects()
        .iter()
        .filter(|effect| {
            matches!(effect.rule, Rule::PreventDamage(_) | Rule::Regenerate(_))
                && effect.source.is_some()
                && effect.is_active(state)
        })
        .cloned()
        .collect()
}

/// The permanent a destruction destroys whatever may be done about it: also one that can't be regenerated, which umbra
/// armor replaces (CR 702.89a).
fn destruction(item: &Item) -> Option<ObjectId> {
    match item {
        Item::One(Event::ZoneChanged {
            object,
            from: Zone::Battlefield,
            cause: ZoneCause::Buried,
            ..
        }) => Some(*object),
        _ => destroyed(item),
    }
}

/// The permanent a destruction destroys: a destroy effect (not one that can't be regenerated) or lethal damage and
/// deathtouch, but not zero toughness (CR 704.5f) or a sacrifice.
fn destroyed(item: &Item) -> Option<ObjectId> {
    match item {
        Item::One(
            Event::ZoneChanged {
                object,
                from: Zone::Battlefield,
                cause: ZoneCause::Destroyed,
                ..
            }
            | Event::CreatureDied {
                object,
                cause: DeathCause::LethalDamage | DeathCause::Deathtouch,
            },
        ) => Some(*object),
        _ => None,
    }
}

/// A replacement effect of an object in play, with whose it is.
#[derive(Debug, Clone)]
struct Source {
    address: AbilityRef,
    ability: Arc<Ability>,
    controller: PlayerId,
    object: ObjectRef,
    /// Read from the copiable values of a card that enters as a copy, whose address is an index into those values.
    copied: bool,
}

/// The replacement effects that work now: those of the permanents (printed, granted, added by a keyword) and of
/// objects in the zone an ability says it works from.
fn gather(state: &GameState) -> Vec<Source> {
    let mut found = Vec::new();
    for object in state.objects() {
        if object.kind() == ObjectKind::Ability {
            continue;
        }
        let zone = object.zone();
        let listed: Vec<(AbilityRef, Arc<Ability>)> = if zone == Zone::Battlefield {
            abilities_of(state, object.id())
        } else if state.possible.elsewhere {
            working_here(object)
                .map(|(address, rules)| (address, Arc::clone(rules)))
                .chain(provided_here(state, object))
                .collect()
        } else {
            continue;
        };
        for (address, rules) in listed {
            if matches!(&*rules, Ability::Replacement { from, .. } if *from == zone_kind(zone)) {
                found.push(Source {
                    address,
                    ability: Arc::clone(&rules),
                    controller: controller_of_object(state, object),
                    object: object.reference(),
                    copied: false,
                });
            }
            if zone == Zone::Battlefield {
                found.extend(riot::given_to_entering(
                    address,
                    &rules,
                    controller_of_object(state, object),
                    object.reference(),
                ));
            }
        }
    }
    found
}

/// What an event is, as the replacement effects see it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Unit {
    /// An effect would create tokens under a player's control.
    Tokens { controller: PlayerId },
    /// A source would deal damage.
    Damage {
        source: ObjectId,
        target: DamageTarget,
    },
    /// Counters would be put on a permanent or a player.
    Counters { on: Target, counter: CounterKind },
    /// A player would gain life.
    Life { player: PlayerId },
    /// A player would draw a card.
    Draw { player: PlayerId },
    /// An object would move to another zone than the battlefield.
    Move {
        object: ObjectId,
        from: Zone,
        to: Zone,
        /// The move is a discard.
        discard: bool,
        /// The move is a destruction (CR 701.8): lethal damage or deathtouch, or an effect that destroys.
        destroyed: bool,
    },
    /// A card would enter the battlefield.
    Enter {
        object: ObjectId,
        controller: PlayerId,
    },
    /// Nothing that is replaced.
    Other,
}

/// The card that enters the battlefield with this event, and under whose control.
fn entering(state: &GameState, event: &Event) -> Option<(ObjectId, PlayerId)> {
    match event {
        Event::LandPlayed { player, object, .. } => Some((*object, *player)),
        Event::SpellResolved { object } => {
            let spell = state.object(*object)?;
            let types = spell.base_types();
            (!types.contains(CardType::Instant) && !types.contains(CardType::Sorcery))
                .then(|| (*object, spell.controller()))
        }
        Event::ZoneChanged {
            object,
            to: Zone::Battlefield,
            ..
        } => Some((*object, state.object(*object)?.owner())),
        _ => None,
    }
}

/// Whether the event is a discard: a card put into a graveyard from a hand, or one a replacement effect sent elsewhere.
fn discards(event: &Event) -> bool {
    matches!(
        event,
        Event::CardDiscarded { .. }
            | Event::ZoneChanged {
                cause: ZoneCause::Discarded,
                ..
            }
    )
}

/// The move to a zone other than the battlefield that this event is, with where the object is and where it goes.
fn moving(state: &GameState, event: &Event) -> Option<(ObjectId, Zone, Zone)> {
    let owner = |object: &ObjectId| state.object(*object).map(GameObject::owner);
    match event {
        Event::ZoneChanged {
            object, from, to, ..
        } if *to != Zone::Battlefield => Some((*object, *from, *to)),
        Event::CreatureDied { object, .. } => {
            Some((*object, Zone::Battlefield, Zone::Graveyard(owner(object)?)))
        }
        Event::CardDiscarded { player, object } => Some((
            *object,
            Zone::Hand(*player),
            Zone::Graveyard(owner(object)?),
        )),
        Event::SpellResolved { object } if entering(state, event).is_none() => {
            Some((*object, Zone::Stack, Zone::Graveyard(owner(object)?)))
        }
        Event::ResolutionEnded { object, outcome } if *outcome != ResolutionOutcome::Resolved => {
            let spell = state
                .object(*object)
                .filter(|o| o.kind() != ObjectKind::Ability)?;
            Some((*object, Zone::Stack, Zone::Graveyard(spell.owner())))
        }
        _ => None,
    }
}

/// Whether the effect is about its own object alone: a self-replacement effect applies first (CR 614.15).
fn is_self(when: &ReplaceWhen) -> bool {
    use mtg_cards::refs::Pred;
    let only_this = |selector: &mtg_cards::refs::Selector| selector.0.contains(&Pred::This);
    match when {
        ReplaceWhen::Enters { what }
        | ReplaceWhen::ZoneChange { what, .. }
        | ReplaceWhen::Destroyed { what }
        | ReplaceWhen::Discarded { what } => only_this(what),
        ReplaceWhen::Damage { to, .. } | ReplaceWhen::CountersPut { on: to, .. } => {
            matches!(to, Some(Ref::This))
        }
        ReplaceWhen::TokenCreation { .. }
        | ReplaceWhen::LifeGain { .. }
        | ReplaceWhen::Draw { .. } => false,
    }
}

/// Whether the engine can apply `by` to this kind of event.
fn applies_to(by: &ReplaceBy, unit: &Unit) -> bool {
    match by {
        ReplaceBy::PreventUpTo(_) | ReplaceBy::PreventRemovingCounter(_) => {
            matches!(unit, Unit::Damage { .. })
        }
        ReplaceBy::Dredge(_) => matches!(unit, Unit::Draw { .. }),
        ReplaceBy::UmbraArmor => matches!(
            unit,
            Unit::Move {
                destroyed: true,
                ..
            }
        ),
        ReplaceBy::Multiply(_) | ReplaceBy::Add(_) | ReplaceBy::Prevent => matches!(
            unit,
            Unit::Tokens { .. }
                | Unit::Damage { .. }
                | Unit::Counters { .. }
                | Unit::Life { .. }
                | Unit::Draw { .. }
        ),
        ReplaceBy::MoveTo(kind) => {
            matches!(unit, Unit::Move { .. })
                && matches!(kind, ZoneKind::Exile | ZoneKind::Graveyard | ZoneKind::Hand)
        }
        ReplaceBy::Tapped
        | ReplaceBy::WithCounters { .. }
        | ReplaceBy::Choose(_)
        | ReplaceBy::PayLifeOrTapped(_)
        | ReplaceBy::Devour { .. }
        | ReplaceBy::Riot
        | ReplaceBy::AsCopy { .. } => matches!(unit, Unit::Enter { .. }),
        // Not run yet: a card that uses it is not Verified.
        ReplaceBy::Instead(_) => false,
    }
}

/// What an effect does to the event.
#[derive(Debug, Clone)]
enum Action {
    Replace(ReplaceBy),
    /// The permanent is tapped, loses its damage and leaves combat instead of being destroyed (CR 701.19).
    Regenerate,
}

impl Action {
    /// Whether this prevents damage, which a rule can forbid.
    fn prevents(&self) -> bool {
        matches!(
            self,
            Action::Replace(
                ReplaceBy::Prevent
                    | ReplaceBy::PreventUpTo(_)
                    | ReplaceBy::PreventRemovingCounter(_)
            )
        )
    }
}

/// An effect that applies to an event, with who owns it.
#[derive(Debug, Clone)]
struct Candidate {
    origin: Origin,
    /// The object with the ability, or that made the shield.
    object: ObjectRef,
    controller: PlayerId,
    action: Action,
    is_self: bool,
}

/// How far the step has come.
enum Run {
    Ask(Decision),
    Done,
}

/// A question that is asked, or the answer to it.
enum Asked<T> {
    Ask(Decision),
    Got(T),
}

/// What applying one effect needs: a question first, or it is applied.
enum Applied {
    Ask(Decision),
    Done,
}

/// The step's working state.
struct Pass<'a> {
    state: &'a GameState,
    sources: Vec<Source>,
    shields: Vec<RuleEffect>,
    /// What the prevention shields that prevent a given amount still prevent, as the batch has used them so far.
    left: BTreeMap<EffectId, u32>,
    /// The shields the batch used, in the order it used them.
    touched: Vec<EffectId>,
    /// The regeneration shields the batch used.
    spent: BTreeSet<EffectId>,
    answers: std::slice::Iter<'a, (PlayerId, Choice)>,
    next_id: u64,
    slots: Vec<Slot>,
    draws_changed: bool,
    /// The commanders put into the command zone in place of being drawn: the draws after them take the cards below.
    pulled: Vec<ObjectId>,
    /// The objects that activated an ability in this batch: what they pay is a cost, not an effect.
    activated: Vec<ObjectId>,
}

impl Pass<'_> {
    fn unit(&self, item: &Item) -> Unit {
        let state = self.state;
        let event = match item {
            Item::Tokens(run) => {
                return Unit::Tokens {
                    controller: run[0].controller,
                };
            }
            Item::One(event) => event,
        };
        if let Some((object, controller)) = entering(state, event) {
            return Unit::Enter {
                object,
                controller: self.entering_under(object, controller),
            };
        }
        if let Some((object, from, to)) = moving(state, event) {
            return Unit::Move {
                object,
                from,
                to,
                discard: discards(event),
                destroyed: destruction(item).is_some(),
            };
        }
        match event {
            Event::DamageDealt { source, target, .. } => Unit::Damage {
                source: *source,
                target: *target,
            },
            Event::CountersChanged(CountersChanged { on, counter, delta }) if *delta > 0 => {
                // What a planeswalker pays to activate a loyalty ability is a cost, not an effect.
                let cost =
                    matches!(on, Target::Object(named) if self.activated.contains(&named.id));
                if cost {
                    Unit::Other
                } else {
                    Unit::Counters {
                        on: *on,
                        counter: counter.clone(),
                    }
                }
            }
            Event::LifeGained { player, .. } => Unit::Life { player: *player },
            Event::CardDrawn { player, .. } | Event::DrewFromEmptyLibrary { player } => {
                Unit::Draw { player: *player }
            }
            _ => Unit::Other,
        }
    }

    /// Who an object that enters comes in under: the player an `Entered` event of this batch names, else `default`.
    fn entering_under(&self, object: ObjectId, default: PlayerId) -> PlayerId {
        self.slots
            .iter()
            .find_map(|slot| match &slot.item {
                Item::One(Event::Entered {
                    object: entered,
                    controller,
                    ..
                }) if *entered == object => Some(*controller),
                _ => None,
            })
            .unwrap_or(default)
    }

    fn reference(&self, id: ObjectId) -> Option<ObjectRef> {
        self.state.object(id).map(GameObject::reference)
    }

    /// Whether combat damage is being dealt: the damage steps with nothing resolving.
    fn combat_damage(&self) -> bool {
        in_combat_damage(self.state)
    }

    /// Whether the effect names the target `target`.
    fn includes(
        &self,
        scope: &Scope<'_>,
        named: &Ref,
        target: Target,
    ) -> Result<bool, EngineError> {
        Ok(match (named, target) {
            (Ref::Each(selector), Target::Object(object)) => match self.reference(object.id) {
                Some(reference) => scope.selects(selector, reference)?,
                None => false,
            },
            (Ref::Each(_), Target::Player(_)) => false,
            (named, Target::Player(player)) => scope.refs(named)?.contains(&Target::Player(player)),
            (named, Target::Object(object)) => scope
                .refs(named)?
                .iter()
                .any(|found| matches!(found, Target::Object(other) if other.id == object.id)),
        })
    }

    /// Whether the effect replaces this event.
    fn matches(
        &self,
        source: &Source,
        when: &ReplaceWhen,
        unit: &Unit,
    ) -> Result<bool, EngineError> {
        let scope = Scope::announcing(self.state, source.controller, source.object);
        Ok(match (when, unit) {
            (ReplaceWhen::TokenCreation { controller }, Unit::Tokens { controller: made }) => {
                scope.players(controller)?.contains(made)
            }
            (
                ReplaceWhen::Damage {
                    to,
                    source: from,
                    combat,
                },
                Unit::Damage {
                    source: dealer,
                    target,
                },
            ) => {
                if combat.is_some_and(|wanted| wanted != self.combat_damage()) {
                    return Ok(false);
                }
                if !from.is_empty() {
                    let Some(reference) = self.reference(*dealer) else {
                        return Ok(false);
                    };
                    if !scope.selects(from, reference)? {
                        return Ok(false);
                    }
                }
                match to {
                    None => true,
                    Some(to) => {
                        let hit = match target {
                            DamageTarget::Player(player) => Target::Player(*player),
                            DamageTarget::Object(object) => match self.reference(*object) {
                                Some(reference) => Target::Object(reference),
                                None => return Ok(false),
                            },
                        };
                        self.includes(&scope, to, hit)?
                    }
                }
            }
            (
                ReplaceWhen::ZoneChange { what, from, to },
                Unit::Move {
                    object,
                    from: left,
                    to: entered,
                    ..
                },
            ) => {
                let Some(reference) = self.reference(*object) else {
                    return Ok(false);
                };
                from.is_none_or(|kind| zone_kind(*left) == kind)
                    && to.is_none_or(|kind| zone_kind(*entered) == kind)
                    && scope.selects(what, reference)?
            }
            (
                ReplaceWhen::CountersPut { on, counter },
                Unit::Counters {
                    on: put,
                    counter: kind,
                },
            ) => {
                counter.as_ref().is_none_or(|wanted| wanted == kind)
                    && match on {
                        None => true,
                        Some(on) => self.includes(&scope, on, *put)?,
                    }
            }
            (ReplaceWhen::LifeGain { who }, Unit::Life { player })
            | (ReplaceWhen::Draw { who }, Unit::Draw { player }) => {
                scope.players(who)?.contains(player)
            }
            (
                ReplaceWhen::Discarded { what },
                Unit::Move {
                    object,
                    to: Zone::Graveyard(_),
                    discard: true,
                    ..
                },
            )
            | (
                ReplaceWhen::Destroyed { what },
                Unit::Move {
                    object,
                    destroyed: true,
                    ..
                },
            )
            | (ReplaceWhen::Enters { what }, Unit::Enter { object, .. }) => {
                let Some(reference) = self.reference(*object) else {
                    return Ok(false);
                };
                scope.selects(what, reference)?
            }
            _ => false,
        })
    }

    /// The effects that apply to the item and have not been applied to it, in the order of their addresses.
    fn candidates(&self, at: usize) -> Result<Vec<Candidate>, EngineError> {
        let slot = &self.slots[at];
        let unit = self.unit(&slot.item);
        if unit == Unit::Other || slot.frozen {
            return Ok(Vec::new());
        }
        // A destruction is replaced by a regeneration shield before anything else applies to the permanent's move, but
        // for umbra armor, which is the other way to save it (CR 616.1).
        let regeneration = self.regeneration_candidate(&slot.item);
        let unpreventable = matches!(
            unit,
            Unit::Damage { source, .. } if damage_cant_be_prevented(self.state, source, self.combat_damage())
        );
        if !unpreventable
            && let Unit::Damage { source, target } = &unit
            && let Some(protection) = self.protection_candidate(*source, *target)
        {
            return Ok(vec![protection]);
        }
        let mut found: Vec<Candidate> = Vec::new();
        let mut consider = |step: &Pass<'_>, source: &Source| -> Result<(), EngineError> {
            let Ability::Replacement {
                when,
                by,
                condition,
                ..
            } = &*source.ability
            else {
                return Ok(());
            };
            let origin = if source.copied {
                Origin::Copied(source.address.ability)
            } else if matches!(by, ReplaceBy::Dredge(_)) {
                Origin::Dredge(source.address)
            } else {
                Origin::Ability(source.address)
            };
            if slot.applied.contains(&origin)
                || found.iter().any(|c| c.origin == origin)
                || !applies_to(by, &unit)
                || (regeneration.is_some() && !matches!(by, ReplaceBy::UmbraArmor))
                || (unpreventable && Action::Replace(by.clone()).prevents())
                || matches!(by, ReplaceBy::Dredge(cards) if !step.can_dredge(at, source.object.id, *cards))
                || !step.matches(source, when, &unit)?
            {
                return Ok(());
            }
            if let Some(condition) = condition {
                let scope = Scope::announcing(step.state, source.controller, source.object);
                if !scope.holds(condition)? {
                    return Ok(());
                }
            }
            found.push(Candidate {
                origin,
                object: source.object,
                controller: source.controller,
                action: Action::Replace(by.clone()),
                is_self: is_self(when),
            });
            Ok(())
        };
        for source in &self.sources {
            consider(self, source)?;
        }
        // The card that enters is not in play yet: the effects it prints about its own entering work from wherever it is.
        if let Unit::Enter { object, .. } = &unit
            && let Some(card) = self.state.object(*object)
            && card.zone() != Zone::Battlefield
        {
            let controller = controller_of_object(self.state, card);
            for (address, rules, copied) in self.own_abilities(*object) {
                let own = Source {
                    address,
                    ability: rules,
                    controller,
                    object: card.reference(),
                    copied,
                };
                if matches!(
                    &*own.ability,
                    Ability::Replacement {
                        when: ReplaceWhen::Enters { .. },
                        ..
                    }
                ) {
                    consider(self, &own)?;
                }
            }
        }
        if let Some(regeneration) = regeneration {
            found.push(regeneration);
            found.sort_by_key(|candidate| candidate.origin);
            return Ok(found);
        }
        if !unpreventable {
            found.extend(self.shield_candidates(slot, &unit)?);
        }
        if matches!(&unit, Unit::Move { .. }) {
            found.extend(self.move_rule_candidates(slot));
        }
        found.extend(self.commander_candidate(at, &unit));
        found.sort_by_key(|candidate| candidate.origin);
        Ok(found)
    }

    /// The rules about a move that are no abilities: the exile of a spell that leaves the stack, buyback, an Adventure
    /// and rebound.
    fn move_rule_candidates(&self, slot: &Slot) -> Vec<Candidate> {
        [
            self.exile_from_stack_candidate(slot),
            self.buyback_candidate(slot),
            self.adventure_candidate(slot),
            self.rebound_candidate(slot),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// The card the draw at `at` takes: the one the event names, or once a draw of the batch was changed, the top card
    /// that the draws before it and the commanders pulled into the command zone leave.
    fn drawn_at(&self, at: usize) -> Option<ObjectId> {
        let Item::One(Event::CardDrawn { player, object }) = &self.slots[at].item else {
            return None;
        };
        if !self.draws_changed {
            return Some(*object);
        }
        self.library_at(at, *player).first().copied()
    }

    /// The exile of a spell cast for its flashback cost or by aftermath that leaves the stack, when it has not been applied.
    fn exile_from_stack_candidate(&self, slot: &Slot) -> Option<Candidate> {
        if slot.applied.contains(&Origin::Flashback) || !leaves_stack_exiled(self.state, &slot.item)
        {
            return None;
        }
        let Unit::Move { object, .. } = self.unit(&slot.item) else {
            return None;
        };
        let spell = self.state.object(object)?;
        Some(Candidate {
            origin: Origin::Flashback,
            object: spell.reference(),
            controller: spell.controller(),
            action: Action::Replace(ReplaceBy::MoveTo(ZoneKind::Exile)),
            is_self: true,
        })
    }

    /// The exile of a spell with rebound that resolved after it was cast from a hand, when it has not been applied.
    fn rebound_candidate(&self, slot: &Slot) -> Option<Candidate> {
        if slot.applied.contains(&Origin::Rebound) || !exiles_by_rebound(self.state, &slot.item) {
            return None;
        }
        let Item::One(Event::SpellResolved { object }) = &slot.item else {
            return None;
        };
        let spell = self.state.object(*object)?;
        Some(Candidate {
            origin: Origin::Rebound,
            object: spell.reference(),
            controller: spell.controller(),
            action: Action::Replace(ReplaceBy::MoveTo(ZoneKind::Exile)),
            // The rule is no effect of the spell, so a replacement of the graveyard move (Rest in Peace) is ordered with it (CR 616.1).
            is_self: false,
        })
    }

    /// The exile of a spell that resolved as an Adventure, when it has not been applied.
    fn adventure_candidate(&self, slot: &Slot) -> Option<Candidate> {
        let Item::One(event @ Event::SpellResolved { object }) = &slot.item else {
            return None;
        };
        let spell = self.state.object(*object)?;
        let adventure = spell.kind() == ObjectKind::Card
            && spell.whole_card().layout == Layout::Adventure
            && spell.face() == Some(1);
        if slot.applied.contains(&Origin::Adventure)
            || !adventure
            || entering(self.state, event).is_some()
        {
            return None;
        }
        Some(Candidate {
            origin: Origin::Adventure,
            object: spell.reference(),
            controller: spell.controller(),
            action: Action::Replace(ReplaceBy::MoveTo(ZoneKind::Exile)),
            // The rule is no effect of the spell, so a replacement of the graveyard move (Rest in Peace) is ordered with it (CR 616.1).
            is_self: false,
        })
    }

    /// The command zone in place of a hand or a library for a commander, when its owner has not been asked yet
    /// (CR 903.9b). A draw puts the card into a hand, so a commander that would be drawn is offered it too.
    fn commander_candidate(&self, at: usize, unit: &Unit) -> Option<Candidate> {
        let (object, to) = match unit {
            Unit::Move { object, to, .. } => (*object, *to),
            Unit::Draw { player } => (self.drawn_at(at)?, Zone::Hand(*player)),
            _ => return None,
        };
        if self.slots[at].applied.contains(&Origin::Commander)
            || !matches!(to, Zone::Hand(_) | Zone::Library(_))
            || !self.state.is_commander(object)
        {
            return None;
        }
        let card = self.state.object(object)?;
        self.state.is_alive(card.owner()).then(|| Candidate {
            origin: Origin::Commander,
            object: card.reference(),
            controller: card.owner(),
            action: Action::Replace(ReplaceBy::MoveTo(ZoneKind::Command)),
            // The rule is no effect of the commander, so it is ordered with the others (CR 616.1).
            is_self: false,
        })
    }

    /// The return of a spell that resolved with its buyback cost paid to its owner's hand, when it has not been applied.
    fn buyback_candidate(&self, slot: &Slot) -> Option<Candidate> {
        if slot.applied.contains(&Origin::Buyback) || !returns_by_buyback(self.state, &slot.item) {
            return None;
        }
        let Item::One(Event::SpellResolved { object }) = &slot.item else {
            return None;
        };
        let spell = self.state.object(*object)?;
        Some(Candidate {
            origin: Origin::Buyback,
            object: spell.reference(),
            controller: spell.controller(),
            action: Action::Replace(ReplaceBy::MoveTo(ZoneKind::Hand)),
            is_self: true,
        })
    }

    /// The protection that prevents all the damage `dealer` would deal to `target`, as the only effect that applies to it:
    /// preventing all of it first leaves every shield whole, which is never worse for the player (CR 616.1).
    fn protection_candidate(&self, dealer: ObjectId, target: DamageTarget) -> Option<Candidate> {
        let (object, controller) = protection_of(self.state, dealer, target)?;
        Some(Candidate {
            origin: Origin::Protection,
            object,
            controller,
            action: Action::Replace(ReplaceBy::Prevent),
            is_self: false,
        })
    }

    /// The regeneration shield that replaces the destruction the item is, as the only effect that applies to it.
    fn regeneration_candidate(&self, item: &Item) -> Option<Candidate> {
        let shield = self.regeneration_of(destroyed(item)?)?;
        Some(Candidate {
            origin: Origin::Shield(shield.id),
            object: shield.source?,
            controller: shield.controller,
            action: Action::Regenerate,
            is_self: false,
        })
    }

    /// The prevention shields that cover the damage the slot holds and have not been applied to it.
    fn shield_candidates(&self, slot: &Slot, unit: &Unit) -> Result<Vec<Candidate>, EngineError> {
        let Unit::Damage { source, target } = unit else {
            return Ok(Vec::new());
        };
        let mut found = Vec::new();
        for effect in &self.shields {
            let origin = Origin::Shield(effect.id);
            let (Rule::PreventDamage(shield), Some(maker)) = (&effect.rule, effect.source) else {
                continue;
            };
            if slot.applied.contains(&origin)
                || !self.shield_covers(effect, shield, *source, *target)?
            {
                continue;
            }
            let by = match self.left.get(&effect.id) {
                Some(0) => continue,
                Some(left) => ReplaceBy::PreventUpTo(*left),
                None => ReplaceBy::Prevent,
            };
            found.push(Candidate {
                origin,
                object: maker,
                controller: effect.controller,
                action: Action::Replace(by),
                is_self: false,
            });
        }
        Ok(found)
    }

    /// The regeneration shield that replaces the destruction of `object`, if one is still unused.
    fn regeneration_of(&self, object: ObjectId) -> Option<&RuleEffect> {
        let current = self.state.object(object)?.counter();
        self.shields.iter().find(|effect| {
            matches!(&effect.rule, Rule::Regenerate(named) if named.id == object && named.counter == current)
                && !self.spent.contains(&effect.id)
        })
    }

    /// Whether the shield covers the damage `dealer` would deal to `target`.
    fn shield_covers(
        &self,
        effect: &RuleEffect,
        shield: &DamageShield,
        dealer: ObjectId,
        target: DamageTarget,
    ) -> Result<bool, EngineError> {
        if shield
            .combat
            .is_some_and(|wanted| wanted != self.combat_damage())
        {
            return Ok(false);
        }
        if let Some(protected) = &shield.to {
            let hit = protected.iter().any(|named| match (named, target) {
                (Target::Player(player), DamageTarget::Player(hit)) => *player == hit,
                (Target::Object(named), DamageTarget::Object(hit)) => {
                    named.id == hit
                        && self
                            .state
                            .object(hit)
                            .is_some_and(|o| o.counter() == named.counter)
                }
                _ => false,
            });
            if !hit {
                return Ok(false);
            }
        }
        if shield.source.is_empty() {
            return Ok(true);
        }
        let (Some(maker), Some(reference)) = (effect.source, self.reference(dealer)) else {
            return Ok(false);
        };
        let scope = Scope::announcing(self.state, effect.controller, maker);
        Ok(scope.selects(&shield.source, reference)?)
    }

    /// The player who orders the effects that apply to the item (CR 616.1): the one it happens to, or the controller of
    /// the object it happens to.
    fn affected(&self, unit: &Unit) -> PlayerId {
        let controller_of = |object: ObjectId| {
            self.state
                .object(object)
                .map_or(self.state.active_or_next(), |o| {
                    controller_of_object(self.state, o)
                })
        };
        match unit {
            Unit::Tokens { controller } | Unit::Enter { controller, .. } => *controller,
            Unit::Life { player } | Unit::Draw { player } => *player,
            Unit::Damage { target, .. } => match target {
                DamageTarget::Player(player) => *player,
                DamageTarget::Object(object) => controller_of(*object),
            },
            Unit::Counters { on, .. } => match on {
                Target::Player(player) => *player,
                Target::Object(object) => controller_of(object.id),
            },
            Unit::Move { object, .. } => controller_of(*object),
            Unit::Other => self.state.active_or_next(),
        }
    }

    /// The question for the item in words.
    fn describe(&self, item: &Item) -> String {
        let name = |object: ObjectId| {
            self.state
                .object(object)
                .map_or_else(|| "A card".to_owned(), |o| o.card().name.clone())
        };
        match (item, self.unit(item)) {
            (Item::Tokens(run), _) => format!("Create {} token(s).", run.len()),
            (Item::One(Event::DamageDealt { amount, .. }), _) => format!("Deal {amount} damage."),
            (Item::One(Event::CountersChanged(changed)), _) => {
                format!(
                    "Put {} {} counter(s).",
                    changed.delta,
                    changed.counter.name()
                )
            }
            (Item::One(Event::LifeGained { amount, .. }), _) => format!("Gain {amount} life."),
            (_, Unit::Draw { .. }) => "Draw a card.".to_owned(),
            (_, Unit::Move { object, to, .. }) => {
                format!("{} would be put into {}.", name(object), zone_word(to))
            }
            (_, Unit::Enter { object, .. }) => {
                format!("{} would enter the battlefield.", name(object))
            }
            _ => String::new(),
        }
    }

    /// The next answer of the journal, if there is one.
    fn answer(&mut self) -> Option<&(PlayerId, Choice)> {
        self.answers.next()
    }

    fn invalid(what: String) -> EngineError {
        EngineError::Resolution(what)
    }

    /// Applies the effects to the items in order until none applies.
    fn run(&mut self) -> Result<Run, EngineError> {
        let mut at = 0;
        // Each effect applies to an event once, so the work is bounded by the effects times the events.
        let mut budget = 10_000;
        while at < self.slots.len() {
            budget -= 1;
            if budget == 0 {
                return Err(Self::invalid(
                    "replacement effects went on without end".into(),
                ));
            }
            let candidates = self.candidates(at)?;
            let chosen = if candidates.is_empty() {
                at += 1;
                continue;
            } else {
                let selfs: Vec<&Candidate> = candidates.iter().filter(|c| c.is_self).collect();
                let pool: Vec<&Candidate> = if selfs.is_empty() {
                    candidates.iter().collect()
                } else {
                    selfs
                };
                // Preventing all of it twice is the same whichever comes first.
                let alike = pool
                    .iter()
                    .all(|c| matches!(c.action, Action::Replace(ReplaceBy::Prevent)));
                // Entering tapped, a choice made as it enters and the counters it enters with each change a part of the
                // entering of their own, so the order is not a choice (a Thriving land is tapped and asks for a color).
                let apart = pool.iter().all(|c| {
                    matches!(
                        c.action,
                        Action::Replace(
                            ReplaceBy::Tapped
                                | ReplaceBy::Choose(_)
                                | ReplaceBy::WithCounters { .. }
                                | ReplaceBy::Devour { .. }
                                | ReplaceBy::Riot
                        )
                    )
                });
                if let [only, ..] = pool.as_slice()
                    && (pool.len() == 1 || alike || apart)
                {
                    (*only).clone()
                } else {
                    let unit = self.unit(&self.slots[at].item);
                    let player = self.affected(&unit);
                    match self.answer() {
                        None => {
                            return Ok(Run::Ask(Decision {
                                player,
                                kind: DecisionKind::ChooseReplacement(ReplacementOptions {
                                    candidates: pool
                                        .iter()
                                        .map(|c| ReplacementCandidate {
                                            source: c.object.id,
                                            ability: c.origin.ability_index(),
                                            shield: c.origin.shield(),
                                        })
                                        .collect(),
                                    text: self.describe(&self.slots[at].item),
                                }),
                            }));
                        }
                        Some((who, Choice::Replacement(index)))
                            if *who == player && *index < pool.len() =>
                        {
                            pool[*index].clone()
                        }
                        Some(other) => {
                            return Err(Self::invalid(format!(
                                "{other:?} is no answer to the order of replacement effects of {player:?}"
                            )));
                        }
                    }
                }
            };
            let (slot, chosen) = match self.shield_takes(at, chosen)? {
                Asked::Ask(decision) => return Ok(Run::Ask(decision)),
                Asked::Got(pick) => pick,
            };
            match self.confirmed(slot, &chosen)? {
                Asked::Ask(decision) => return Ok(Run::Ask(decision)),
                Asked::Got(false) => continue,
                Asked::Got(true) => {}
            }
            match self.apply(slot, &chosen)? {
                Applied::Ask(decision) => return Ok(Run::Ask(decision)),
                Applied::Done => {}
            }
        }
        if self.answers.next().is_some() {
            return Err(Self::invalid(
                "an answer was given to a question that was not asked".into(),
            ));
        }
        Ok(Run::Done)
    }

    /// The slot a prevention shield that stops a given amount is applied to when it is chosen for the slot at `at`: the
    /// damage events of one affected player that it covers share what it has left, so that player picks which event it
    /// takes next. Nothing is asked when it has enough for all of them or the events are the same event.
    fn shield_takes(
        &mut self,
        at: usize,
        chosen: Candidate,
    ) -> Result<Asked<(usize, Candidate)>, EngineError> {
        let (Origin::Shield(_), Action::Replace(ReplaceBy::PreventUpTo(left))) =
            (chosen.origin, &chosen.action)
        else {
            return Ok(Asked::Got((at, chosen)));
        };
        let left = u64::from(*left);
        let unit = self.unit(&self.slots[at].item);
        if !matches!(unit, Unit::Damage { .. }) {
            return Ok(Asked::Got((at, chosen)));
        }
        let player = self.affected(&unit);
        let mut group: Vec<(usize, Event, Candidate)> = Vec::new();
        for slot in at..self.slots.len() {
            let Item::One(event @ Event::DamageDealt { .. }) = &self.slots[slot].item else {
                continue;
            };
            if self.affected(&self.unit(&self.slots[slot].item)) != player {
                continue;
            }
            let found = self.candidates(slot)?;
            if found.iter().any(|candidate| candidate.is_self) {
                continue;
            }
            if let Some(same) = found
                .into_iter()
                .find(|candidate| candidate.origin == chosen.origin)
            {
                group.push((slot, event.clone(), same));
            }
        }
        let total: u64 = group
            .iter()
            .map(|(_, event, _)| match event {
                Event::DamageDealt { amount, .. } => u64::from(*amount),
                _ => 0,
            })
            .sum();
        let mut distinct: Vec<(usize, Event, Candidate)> = Vec::new();
        for entry in group {
            if !distinct.iter().any(|(_, seen, _)| *seen == entry.1) {
                distinct.push(entry);
            }
        }
        if total <= left || distinct.len() < 2 {
            return Ok(Asked::Got((at, chosen)));
        }
        let options: Vec<ChoiceOption> = distinct
            .iter()
            .map(|(_, event, _)| ChoiceOption::Label(self.describe_damage(event)))
            .collect();
        let maker = self.state.object(chosen.object.id).map_or_else(
            || "The shield".to_owned(),
            |object| object.card().name.clone(),
        );
        match self.answer() {
            None => Ok(Asked::Ask(Decision {
                player,
                kind: DecisionKind::ChooseOption(OptionOptions {
                    source: chosen.object.id,
                    options,
                    text: format!("Choose the damage {maker} prevents first."),
                }),
            })),
            Some((who, Choice::Option(index))) if *who == player && *index < distinct.len() => {
                let (slot, _, candidate) = distinct.swap_remove(*index);
                Ok(Asked::Got((slot, candidate)))
            }
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to the order of the damage a shield prevents of {player:?}"
            ))),
        }
    }

    /// A damage event in words ("2 damage from Imp to Bear0").
    fn describe_damage(&self, event: &Event) -> String {
        let name = |object: ObjectId| {
            self.state
                .object(object)
                .map_or_else(|| "a card".to_owned(), |o| o.card().name.clone())
        };
        let Event::DamageDealt {
            source,
            target,
            amount,
            ..
        } = event
        else {
            return String::new();
        };
        let to = match target {
            DamageTarget::Player(player) => format!("player {}", player.get()),
            DamageTarget::Object(object) => name(*object),
        };
        format!("{amount} damage from {} to {to}", name(*source))
    }

    /// Puts the items back as events; cards drawn by an event that was changed are the top cards in order.
    fn finish(self) -> Vec<Event> {
        let used = self.shield_events();
        let mut events: Vec<Event> = self
            .slots
            .into_iter()
            .flat_map(|slot| match slot.item {
                Item::One(event) => vec![event],
                Item::Tokens(run) => run.into_iter().map(Event::TokenCreated).collect(),
            })
            .collect();
        if self.draws_changed {
            let mut taken: BTreeMap<PlayerId, usize> = BTreeMap::new();
            for event in &mut events {
                let drawn = match event {
                    Event::CardDrawn { player, .. } | Event::DrewFromEmptyLibrary { player } => {
                        *player
                    }
                    _ => continue,
                };
                let nth = taken.entry(drawn).or_default();
                let top = self
                    .state
                    .zone(Zone::Library(drawn))
                    .iter()
                    .rev()
                    .filter(|card| !self.pulled.contains(card))
                    .nth(*nth)
                    .copied();
                *nth += 1;
                *event = match top {
                    Some(object) => Event::CardDrawn {
                        player: drawn,
                        object,
                    },
                    None => Event::DrewFromEmptyLibrary { player: drawn },
                };
            }
        }
        events.extend(used);
        events
    }

    /// Applies one effect to the item at `at`.
    fn apply(&mut self, at: usize, candidate: &Candidate) -> Result<Applied, EngineError> {
        let mut applied = self.slots[at].applied.clone();
        applied.push(candidate.origin);
        let unit = self.unit(&self.slots[at].item);
        let by = match &candidate.action {
            Action::Regenerate => {
                self.regenerate(at, candidate);
                return Ok(Applied::Done);
            }
            Action::Replace(by) => by,
        };
        match (by, &unit) {
            (ReplaceBy::Prevent | ReplaceBy::PreventUpTo(_), Unit::Damage { .. }) => {
                self.prevent(at, candidate, by, applied);
            }
            (ReplaceBy::PreventRemovingCounter(kind), Unit::Damage { .. }) => {
                self.prevent(at, candidate, &ReplaceBy::Prevent, applied.clone());
                self.lose_counter(at, candidate, kind, applied);
            }
            (ReplaceBy::Prevent | ReplaceBy::PreventUpTo(_), _) => {
                if matches!(
                    &self.slots[at].item,
                    Item::One(Event::CardDrawn { .. } | Event::DrewFromEmptyLibrary { .. })
                ) {
                    self.draws_changed = true;
                }
                self.slots.remove(at);
            }
            (ReplaceBy::Multiply(_) | ReplaceBy::Add(_), _) => {
                self.scale(at, by, &applied);
            }
            (
                ReplaceBy::MoveTo(kind),
                Unit::Move {
                    object, from, to, ..
                },
            ) => {
                let Some(owner) = self.state.object(*object).map(GameObject::owner) else {
                    self.slots[at].applied = applied;
                    return Ok(Applied::Done);
                };
                let destination = match kind {
                    ZoneKind::Exile => Zone::Exile,
                    ZoneKind::Hand => Zone::Hand(owner),
                    ZoneKind::Command => Zone::Command(owner),
                    _ => Zone::Graveyard(owner),
                };
                self.slots[at].applied.clone_from(&applied);
                if destination != *to {
                    self.redirect(at, *object, *from, destination, &applied);
                }
            }
            (ReplaceBy::Dredge(cards), Unit::Draw { .. }) => self.dredge(at, candidate, *cards),
            (ReplaceBy::UmbraArmor, Unit::Move { object, .. }) => {
                self.umbra(at, candidate, *object);
            }
            (ReplaceBy::MoveTo(ZoneKind::Command), Unit::Draw { player }) => {
                self.slots[at].applied.clone_from(&applied);
                self.pull_drawn_commander(at, *player, &applied);
            }
            (ReplaceBy::Devour { .. }, Unit::Enter { object, controller }) => {
                return self.devour(at, candidate, by, *object, *controller, applied);
            }
            (ReplaceBy::Riot, Unit::Enter { object, controller }) => {
                return self.riot(at, *object, *controller, applied);
            }
            (_, Unit::Enter { object, controller }) => {
                return self.enter(at, candidate, by, *object, *controller, applied);
            }
            _ => self.slots[at].applied = applied,
        }
        Ok(Applied::Done)
    }

    /// The Aura's umbra armor saves the creature: its damage is removed in place of its destruction, and the Aura is
    /// destroyed (CR 702.89a). That destruction is an event of its own that other effects replace, unless the batch
    /// already moves the Aura or it is indestructible.
    fn umbra(&mut self, at: usize, candidate: &Candidate, creature: ObjectId) {
        let aura = candidate.object;
        let mut slots = vec![Slot {
            item: Item::One(Event::DamageRemoved { object: creature }),
            applied: Vec::new(),
            frozen: true,
        }];
        let state = self.state;
        let destroyable = state.object(aura.id).filter(|object| {
            object.zone() == Zone::Battlefield
                && object.counter() == aura.counter
                && !keywords::is_indestructible(state, aura.id)
        });
        let leaving = self.slots.iter().any(|slot| {
            matches!(&slot.item, Item::One(event)
                if moving(state, event).is_some_and(|(object, _, _)| object == aura.id))
        });
        if let (Some(object), false) = (destroyable, leaving) {
            slots.push(Slot::new(Item::One(Event::ZoneChanged {
                object: aura.id,
                from: Zone::Battlefield,
                to: Zone::Graveyard(object.owner()),
                cause: ZoneCause::Destroyed,
            })));
        }
        self.slots.splice(at..=at, slots);
    }

    /// Puts the commander the draw at `at` would take into its owner's command zone instead: no card is drawn.
    fn pull_drawn_commander(&mut self, at: usize, player: PlayerId, applied: &[Origin]) {
        let Some(object) = self.drawn_at(at) else {
            return;
        };
        let Some(owner) = self.state.object(object).map(GameObject::owner) else {
            return;
        };
        self.pulled.push(object);
        self.redirect(
            at,
            object,
            Zone::Library(player),
            Zone::Command(owner),
            applied,
        );
    }

    /// Takes from the damage of the event what the effect prevents: all of it, or as much as the effect has left (the
    /// shield remembers what it used). Lifelink gains what the damage that is left is.
    /// The permanent with the replacement loses one counter of the kind, if it holds one: the removal takes the place of
    /// the damage event the prevention took out at `at`.
    fn lose_counter(
        &mut self,
        at: usize,
        candidate: &Candidate,
        kind: &CounterKind,
        applied: Vec<Origin>,
    ) {
        let holds = self.state.object(candidate.object.id).is_some_and(|card| {
            card.counter() == candidate.object.counter
                && card
                    .permanent()
                    .is_some_and(|held| held.counters_of(kind) > 0)
        });
        if !holds {
            return;
        }
        self.slots.insert(
            at.min(self.slots.len()),
            Slot {
                item: Item::One(Event::CountersChanged(CountersChanged {
                    on: Target::Object(candidate.object),
                    counter: kind.clone(),
                    delta: -1,
                })),
                applied,
                frozen: false,
            },
        );
    }

    fn prevent(&mut self, at: usize, candidate: &Candidate, by: &ReplaceBy, applied: Vec<Origin>) {
        let Item::One(Event::DamageDealt {
            source,
            target,
            amount,
            deathtouch,
            form,
            toxic,
        }) = self.slots[at].item.clone()
        else {
            self.slots[at].applied = applied;
            return;
        };
        let most = match by {
            ReplaceBy::PreventUpTo(most) => *most,
            _ => u32::MAX,
        };
        let taken = most.min(amount);
        if let Origin::Shield(id) = candidate.origin
            && let Some(left) = self.left.get_mut(&id)
        {
            *left = left.saturating_sub(taken);
            if !self.touched.contains(&id) {
                self.touched.push(id);
            }
        }
        let now = amount - taken;
        self.follow_lifelink(at, source, amount, now);
        if now == 0 {
            self.slots.remove(at);
        } else {
            self.slots[at] = Slot {
                item: Item::One(Event::DamageDealt {
                    source,
                    target,
                    amount: now,
                    deathtouch,
                    form,
                    toxic,
                }),
                applied,
                frozen: false,
            };
        }
    }

    /// The permanent regenerates instead of being destroyed; the shield is used up.
    fn regenerate(&mut self, at: usize, candidate: &Candidate) {
        let (Origin::Shield(id), Some(object)) =
            (candidate.origin, destroyed(&self.slots[at].item))
        else {
            return;
        };
        self.spent.insert(id);
        self.touched.push(id);
        self.slots[at] = Slot {
            item: Item::One(Event::Regenerated { object }),
            applied: vec![candidate.origin],
            frozen: true,
        };
    }

    /// What the shields the batch used did to them: a shield with nothing left ends, another remembers what is left.
    fn shield_events(&self) -> Vec<Event> {
        self.touched
            .iter()
            .map(|id| match self.left.get(id) {
                Some(left) if *left > 0 && !self.spent.contains(id) => Event::ShieldUsed {
                    id: *id,
                    left: *left,
                },
                _ => Event::RuleExpired { id: *id },
            })
            .collect()
    }

    /// Multiplies or increases the number in the event: the tokens, the damage, the counters, the life, the cards drawn.
    fn scale(&mut self, at: usize, by: &ReplaceBy, applied: &[Origin]) {
        let more = |amount: u32| match by {
            ReplaceBy::Multiply(times) => amount.saturating_mul(*times),
            ReplaceBy::Add(extra) => amount.saturating_add(*extra),
            _ => amount,
        };
        let item = self.slots[at].item.clone();
        let rebuilt: Vec<Item> = match item {
            Item::Tokens(run) => {
                let mut made: Vec<TokenCreated> = Vec::new();
                let copy = |step: &mut Pass<'_>, token: &TokenCreated| {
                    let fresh = TokenCreated {
                        object: ObjectId(step.next_id),
                        ..token.clone()
                    };
                    step.next_id += 1;
                    fresh
                };
                match by {
                    ReplaceBy::Multiply(times) => {
                        for token in &run {
                            made.push(token.clone());
                            for _ in 1..*times {
                                made.push(copy(self, token));
                            }
                        }
                    }
                    ReplaceBy::Add(extra) => {
                        made.extend(run.iter().cloned());
                        if let Some(last) = run.last() {
                            for _ in 0..*extra {
                                made.push(copy(self, last));
                            }
                        }
                    }
                    _ => made = run,
                }
                vec![Item::Tokens(made)]
            }
            Item::One(Event::DamageDealt {
                source,
                target,
                amount,
                deathtouch,
                form,
                toxic,
            }) => {
                let now = more(amount);
                self.follow_lifelink(at, source, amount, now);
                vec![Item::One(Event::DamageDealt {
                    source,
                    target,
                    amount: now,
                    deathtouch,
                    form,
                    toxic,
                })]
            }
            Item::One(Event::CountersChanged(CountersChanged { on, counter, delta })) => {
                let now = i32::try_from(more(delta.unsigned_abs())).unwrap_or(i32::MAX);
                vec![Item::One(Event::CountersChanged(CountersChanged {
                    on,
                    counter,
                    delta: now,
                }))]
            }
            Item::One(Event::LifeGained { player, amount }) => vec![Item::One(Event::LifeGained {
                player,
                amount: more(amount),
            })],
            Item::One(event @ (Event::CardDrawn { .. } | Event::DrewFromEmptyLibrary { .. })) => {
                self.draws_changed = true;
                let count = more(1);
                (0..count).map(|_| Item::One(event.clone())).collect()
            }
            other @ Item::One(_) => vec![other],
        };
        let slots: Vec<Slot> = rebuilt
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let mut applied = applied.to_vec();
                // The questions of the commander and of a dredge were about the first card drawn: another draw is
                // another event.
                if index > 0 && matches!(item, Item::One(Event::CardDrawn { .. })) {
                    applied
                        .retain(|origin| !matches!(origin, Origin::Commander | Origin::Dredge(_)));
                }
                Slot {
                    item,
                    applied,
                    frozen: false,
                }
            })
            .collect();
        self.slots.splice(at..=at, slots);
    }

    /// The life a source with lifelink gains with the damage it deals is gained with the damage it deals now.
    fn follow_lifelink(&mut self, at: usize, source: ObjectId, before: u32, now: u32) {
        let Some(Slot {
            item: Item::One(Event::LifeGained { player, amount }),
            ..
        }) = self.slots.get_mut(at + 1)
        else {
            return;
        };
        let dealer = crate::damage::Dealer::live(self.state, source);
        if *amount != before || !dealer.lifelink_for(*player) {
            return;
        }
        *amount = now;
        if now == 0 {
            self.slots.remove(at + 1);
        }
    }

    /// The delayed trigger that lets the controller of a rebound spell cast the card from exile at the beginning of their
    /// next upkeep, without paying its mana cost (CR 702.88a). It names the card as the object it becomes in exile.
    fn rebound_cast(&self, spell: ObjectId) -> Option<Event> {
        let object = self.state.object(spell)?;
        let mut snapshot = crate::snapshot::snapshot(self.state, spell)?;
        snapshot.counter += 1;
        snapshot.zone = Zone::Exile;
        Some(Event::DelayedTriggerCreated(Box::new(
            crate::triggers::DelayedCreated {
                id: crate::triggers::DelayedId(self.state.next_delayed),
                source: ObjectRef {
                    id: spell,
                    counter: object.counter() + 1,
                },
                snapshot,
                controller: object.controller(),
                ability: 0,
                when: mtg_cards::trigger::TriggerPattern::StepBegins {
                    step: mtg_cards::trigger::Step::Upkeep,
                    whose: Ref::You,
                },
                once: true,
                effects: vec![mtg_cards::effect::Effect::CastFromZone {
                    who: Ref::You,
                    what: Ref::This,
                    free: true,
                    play: false,
                    way: None,
                    otherwise: Vec::new(),
                }],
                bindings: Vec::new(),
            },
        )))
    }

    /// Sends the object that would move to `destination` instead.
    fn redirect(
        &mut self,
        at: usize,
        object: ObjectId,
        from: Zone,
        destination: Zone,
        applied: &[Origin],
    ) {
        let item = self.slots[at].item.clone();
        let Item::One(event) = item else {
            return;
        };
        let moved = |cause: ZoneCause| Event::ZoneChanged {
            object,
            from,
            to: destination,
            cause,
        };
        let events: Vec<Event> = match event {
            Event::ZoneChanged { cause, .. } => vec![moved(cause)],
            Event::CreatureDied { .. } => vec![moved(ZoneCause::StateBased)],
            Event::CardDiscarded { .. } => vec![moved(ZoneCause::Discarded)],
            Event::CardDrawn { .. } => vec![moved(ZoneCause::Moved)],
            Event::SpellResolved { .. } => {
                // An Adventure that the Adventure rule exiles is on an adventure; one that another effect exiled is
                // not (CR 715.4).
                let cause = if applied.contains(&Origin::Adventure) {
                    ZoneCause::Adventure
                } else {
                    ZoneCause::Moved
                };
                // The spell would have gone to its owner's graveyard and then given priority to the active player.
                let holder = self
                    .state
                    .turn_order_from(self.state.turn().active())
                    .first()
                    .copied();
                std::iter::once(moved(cause))
                    .chain(
                        applied
                            .contains(&Origin::Rebound)
                            .then(|| self.rebound_cast(object))
                            .flatten(),
                    )
                    .chain(holder.map(|player| Event::PriorityGranted { player }))
                    .collect()
            }
            ended @ Event::ResolutionEnded { .. } => vec![moved(ZoneCause::Countered), ended],
            other => vec![other],
        };
        // What carries the move out (the end of the resolution, the priority after it) is not replaced a second time.
        let slots: Vec<Slot> = events
            .into_iter()
            .map(|event| Slot {
                frozen: matches!(
                    event,
                    Event::ResolutionEnded { .. } | Event::PriorityGranted { .. }
                ),
                item: Item::One(event),
                applied: applied.to_vec(),
            })
            .collect();
        self.slots.splice(at..=at, slots);
        // An object sent elsewhere is not among the cards an effect then bottoms or shuffles into the library.
        for slot in &mut self.slots {
            match &mut slot.item {
                Item::One(Event::LibraryBottomed { cards, .. }) => cards.retain(|id| *id != object),
                Item::One(Event::LibraryShuffled { order, .. }) => order.retain(|id| *id != object),
                _ => {}
            }
        }
    }

    /// The `Entered` event that follows the enter event of `object`, made when the batch has none yet.
    fn entered_at(&mut self, at: usize, object: ObjectId, controller: PlayerId) -> usize {
        let existing = self.slots.iter().position(|slot| {
            matches!(&slot.item, Item::One(Event::Entered { object: o, .. }) if *o == object)
        });
        existing.unwrap_or_else(|| {
            let applied = self.slots[at].applied.clone();
            self.slots.insert(
                at + 1,
                Slot {
                    item: Item::One(Event::Entered {
                        object,
                        tapped: false,
                        controller,
                        chosen: None,
                    }),
                    applied,
                    frozen: false,
                },
            );
            at + 1
        })
    }

    /// Changes how the card enters: tapped, with counters, with a choice made, or with life paid.
    fn enter(
        &mut self,
        at: usize,
        candidate: &Candidate,
        by: &ReplaceBy,
        object: ObjectId,
        controller: PlayerId,
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let source = candidate.object.id;
        let tapped = |step: &mut Pass<'_>, at: usize| {
            let entered = step.entered_at(at, object, controller);
            if let Item::One(Event::Entered { tapped, .. }) = &mut step.slots[entered].item {
                *tapped = true;
            }
        };
        match by {
            ReplaceBy::Tapped => tapped(self, at),
            ReplaceBy::WithCounters {
                counter,
                count,
                tapped: and_tapped,
            } => {
                let scope = Scope::announcing(self.state, candidate.controller, candidate.object);
                let number = scope.amount(count)?.max(0);
                if let (Some(card), Ok(delta)) = (self.state.object(object), i32::try_from(number))
                    && delta > 0
                {
                    // The counters go on the permanent the card becomes (CR 400.7).
                    let on = Target::Object(ObjectRef {
                        id: object,
                        counter: card.counter() + 1,
                    });
                    let mut inherited = applied.clone();
                    inherited.dedup();
                    let follow = Slot {
                        item: Item::One(Event::CountersChanged(CountersChanged {
                            on,
                            counter: counter.clone(),
                            delta,
                        })),
                        applied: inherited,
                        frozen: false,
                    };
                    self.slots.insert(at + 1, follow);
                }
                if *and_tapped {
                    tapped(self, at);
                }
            }
            ReplaceBy::PayLifeOrTapped(life) => {
                match self.pays_life(source, controller, *life)? {
                    Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
                    Asked::Got(true) => {
                        self.slots.insert(
                            at,
                            Slot {
                                item: Item::One(Event::LifePaid {
                                    player: controller,
                                    amount: *life,
                                }),
                                applied: Vec::new(),
                                frozen: true,
                            },
                        );
                        // The enter event moved one place on.
                        self.slots[at + 1].applied = applied;
                        return Ok(Applied::Done);
                    }
                    Asked::Got(false) => tapped(self, at),
                }
            }
            ReplaceBy::Choose(kind) => {
                let chosen = match self.chooses(source, controller, *kind)? {
                    Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
                    Asked::Got(chosen) => chosen,
                };
                let entered = self.entered_at(at, object, controller);
                if let Item::One(Event::Entered { chosen: slot, .. }) =
                    &mut self.slots[entered].item
                {
                    *slot = chosen;
                }
            }
            ReplaceBy::AsCopy { of, except } => {
                let copy = match self.copies(candidate.object, controller, object, of, except)? {
                    Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
                    Asked::Got(copy) => copy,
                };
                if let Some(copy) = copy {
                    let mut inherited = applied.clone();
                    inherited.dedup();
                    // The copy begins as the permanent enters, so the effects of the copied values apply to its entering.
                    self.slots.insert(
                        at + 1,
                        Slot {
                            item: Item::One(Event::BecameCopy(copy)),
                            applied: inherited,
                            frozen: false,
                        },
                    );
                }
            }
            _ => {}
        }
        // The enter event itself stays, with the effect marked as applied to it.
        if let Some(slot) = self.slots.get_mut(at)
            && matches!(&slot.item, Item::One(event) if entering(self.state, event).is_some())
        {
            slot.applied = applied;
        }
        Ok(Applied::Done)
    }

    /// Devours as the card enters (CR 702.82): `controller` picks any number of the permanents they control that the
    /// keyword names, each is sacrificed before the card enters, and it enters with the counters for each. Nobody is
    /// asked when there is nothing to sacrifice.
    fn devour(
        &mut self,
        at: usize,
        candidate: &Candidate,
        by: &ReplaceBy,
        object: ObjectId,
        controller: PlayerId,
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let ReplaceBy::Devour { counters, of } = by else {
            return Ok(Applied::Done);
        };
        let scope = Scope::announcing(self.state, candidate.controller, candidate.object);
        let options: Vec<ObjectId> = scope
            .matching(of)?
            .into_iter()
            .map(|named| named.id)
            .filter(|id| {
                *id != object
                    && self
                        .state
                        .object(*id)
                        .is_some_and(|permanent| permanent.zone() == Zone::Battlefield)
                    && crate::characteristics::controller_of(self.state, *id) == Some(controller)
            })
            .collect();
        let chosen: Vec<ObjectId> = if options.is_empty() {
            Vec::new()
        } else {
            match self.answer() {
                None => {
                    return Ok(Applied::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseCards(CardsOptions {
                            source: candidate.object.id,
                            max: u32::try_from(options.len()).unwrap_or(u32::MAX),
                            from: options,
                            min: 0,
                            text: "Choose permanents to sacrifice.".into(),
                            can_cancel: false,
                        }),
                    }));
                }
                Some((who, Choice::Cards(cards)))
                    if *who == controller
                        && cards.iter().all(|card| options.contains(card))
                        && cards.iter().collect::<BTreeSet<_>>().len() == cards.len() =>
                {
                    cards.clone()
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the choice of what {controller:?} devours"
                    )));
                }
            }
        };
        let sacrifices: Vec<Slot> = chosen
            .iter()
            .filter_map(|id| {
                let owner = self.state.object(*id)?.owner();
                Some(Slot::new(Item::One(Event::ZoneChanged {
                    object: *id,
                    from: Zone::Battlefield,
                    to: Zone::Graveyard(owner),
                    cause: ZoneCause::Sacrificed,
                })))
            })
            .collect();
        let made = sacrifices.len();
        self.slots.splice(at..at, sacrifices);
        // The enter event moved to make room for the sacrifices.
        let entering = at + made;
        if let Some(slot) = self.slots.get_mut(entering) {
            slot.applied.clone_from(&applied);
        }
        let gained = u32::try_from(made)
            .ok()
            .and_then(|made| made.checked_mul(*counters))
            .and_then(|total| i32::try_from(total).ok())
            .filter(|total| *total > 0);
        if let (Some(card), Some(delta)) = (self.state.object(object), gained) {
            let mut inherited = applied;
            inherited.dedup();
            // The counters go on the permanent the card becomes (CR 400.7).
            let on = Target::Object(ObjectRef {
                id: object,
                counter: card.counter() + 1,
            });
            self.slots.insert(
                entering + 1,
                Slot {
                    item: Item::One(Event::CountersChanged(CountersChanged {
                        on,
                        counter: CounterKind::PlusOnePlusOne,
                        delta,
                    })),
                    applied: inherited,
                    frozen: false,
                },
            );
        }
        Ok(Applied::Done)
    }

    /// The abilities of a card that enters, each with whether it is read from copied values. A card that enters as a copy
    /// has the copied values from the start (CR 614.12a), so the effects they print about entering apply in place of
    /// its own.
    fn own_abilities(&self, object: ObjectId) -> Vec<(AbilityRef, Arc<Ability>, bool)> {
        match self.entering_copy(object) {
            Some(copy) => crate::ability_reader::printed_rules(&copy.definition)
                .enumerate()
                .map(|(index, rules)| (AbilityRef::printed(object, index), Arc::clone(rules), true))
                .collect(),
            None => abilities_of(self.state, object)
                .into_iter()
                .map(|(address, rules)| (address, rules, false))
                .collect(),
        }
    }

    /// The copy the batch makes `object` as it enters, once the replacement that chose it has applied.
    fn entering_copy(&self, object: ObjectId) -> Option<&BecameCopy> {
        self.slots.iter().find_map(|slot| match &slot.item {
            Item::One(Event::BecameCopy(copy)) if copy.object == object => Some(copy),
            _ => None,
        })
    }

    /// What `controller` has the card `entering` copy as it enters (CR 614.1c, 707.9): any permanent `of` selects, none
    /// or one by their choice, with the exceptions in the copiable values. Nobody is asked when nothing can be copied,
    /// since the battlefield is public.
    fn copies(
        &mut self,
        source: ObjectRef,
        controller: PlayerId,
        entering: ObjectId,
        of: &Selector,
        except: &[Modification],
    ) -> Result<Asked<Option<BecameCopy>>, EngineError> {
        let scope = Scope::announcing(self.state, controller, source);
        let options: Vec<ObjectId> = scope
            .matching(of)?
            .into_iter()
            .filter(|named| {
                named.id != entering && self.state.object(named.id).is_some_and(may_be_copied)
            })
            .map(|named| named.id)
            .collect();
        if options.is_empty() {
            return Ok(Asked::Got(None));
        }
        let chosen = match self.answer() {
            None => {
                return Ok(Asked::Ask(Decision {
                    player: controller,
                    kind: DecisionKind::ChooseCards(CardsOptions {
                        source: source.id,
                        from: options,
                        min: 0,
                        max: 1,
                        text: "Choose a permanent to copy.".into(),
                        can_cancel: false,
                    }),
                }));
            }
            Some((who, Choice::Cards(cards)))
                if *who == controller
                    && cards.len() <= 1
                    && cards.iter().all(|card| options.contains(card)) =>
            {
                cards.first().copied()
            }
            Some(other) => {
                return Err(Self::invalid(format!(
                    "{other:?} is no answer to the choice of what {controller:?} copies"
                )));
            }
        };
        let Some(original) = chosen.and_then(|id| self.state.object(id)) else {
            return Ok(Asked::Got(None));
        };
        let of = original.reference();
        let mut values = copiable_values(self.state, of, None)
            .ok_or_else(|| Self::invalid(format!("{of:?} cannot be copied")))?;
        for modification in except {
            if !values.except(modification) {
                return Err(Self::invalid(format!(
                    "{modification:?} is no exception of a copy effect"
                )));
            }
        }
        Ok(Asked::Got(Some(BecameCopy {
            object: entering,
            of,
            definition: Arc::new(values),
            until_end_of_turn: false,
        })))
    }

    /// Whether the effect that was picked applies: the optional ones ask their player first (the commander rule, dredge).
    fn confirmed(&mut self, at: usize, candidate: &Candidate) -> Result<Asked<bool>, EngineError> {
        match &candidate.action {
            Action::Replace(ReplaceBy::Dredge(cards)) => self.dredge_wanted(at, candidate, *cards),
            _ if candidate.origin == Origin::Commander => {
                self.may_go_to_command_zone(at, candidate)
            }
            _ => Ok(Asked::Got(true)),
        }
    }

    /// Whether the owner of a commander puts it into the command zone in place of the hand or library it would go to
    /// (CR 903.9b): asked yes or no. A no is remembered for the item, which then goes where it was going.
    fn may_go_to_command_zone(
        &mut self,
        at: usize,
        candidate: &Candidate,
    ) -> Result<Asked<bool>, EngineError> {
        let owner = candidate.controller;
        match self.answer() {
            None => {
                let to = match self.unit(&self.slots[at].item) {
                    Unit::Move { to, .. } => zone_word(to),
                    Unit::Draw { .. } => "drawing it",
                    _ => "another zone",
                };
                let name = self
                    .state
                    .object(candidate.object.id)
                    .map_or("the commander", |card| card.card().name.as_str());
                Ok(Asked::Ask(Decision {
                    player: owner,
                    kind: DecisionKind::YesNo(YesNoOptions::new(
                        candidate.object.id,
                        format!("Put {name} into the command zone instead of {to}?"),
                    )),
                }))
            }
            Some((who, Choice::YesNo(yes))) if *who == owner => {
                let yes = *yes;
                if !yes {
                    self.slots[at].applied.push(Origin::Commander);
                }
                Ok(Asked::Got(yes))
            }
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to the commander question of {owner:?}"
            ))),
        }
    }

    /// Whether `controller` pays `life` as the card enters: asked yes or no, with No the only answer when they cannot
    /// pay, so the pace does not show whether they could.
    fn pays_life(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        life: u32,
    ) -> Result<Asked<bool>, EngineError> {
        let can_pay = self
            .state
            .player(controller)
            .is_some_and(|p| u32::try_from(p.life()).is_ok_and(|have| have >= life));
        match self.answer() {
            None => Ok(Asked::Ask(Decision {
                player: controller,
                kind: DecisionKind::YesNo(YesNoOptions {
                    only_no: !can_pay,
                    ..YesNoOptions::new(source, format!("Pay {life} life?"))
                }),
            })),
            Some((who, Choice::YesNo(yes))) if *who == controller => Ok(Asked::Got(*yes)),
            Some(other) => Err(Self::invalid(format!(
                "{other:?} is no answer to the payment of life of {controller:?}"
            ))),
        }
    }

    /// What `controller` chooses as the card enters: asked unless there is one thing to choose or none.
    fn chooses(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        kind: EntersChoice,
    ) -> Result<Asked<Option<Chosen>>, EngineError> {
        let options = self.options_of(kind);
        let picked = match options.as_slice() {
            [] => None,
            [only] => Some(only.clone()),
            _ => match self.answer() {
                None => {
                    let text = match kind {
                        EntersChoice::Color => "Choose a color.".to_owned(),
                        EntersChoice::ColorOtherThan(other) => {
                            format!(
                                "Choose a color other than {}.",
                                format!("{other:?}").to_lowercase()
                            )
                        }
                        EntersChoice::CreatureType => "Choose a creature type.".to_owned(),
                        EntersChoice::Player => "Choose a player.".to_owned(),
                        EntersChoice::Opponent => "Choose an opponent.".to_owned(),
                    };
                    return Ok(Asked::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseOption(OptionOptions {
                            source,
                            options,
                            text,
                        }),
                    }));
                }
                Some((who, Choice::Option(index)))
                    if *who == controller && *index < options.len() =>
                {
                    Some(options[*index].clone())
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the choice of {controller:?}"
                    )));
                }
            },
        };
        Ok(Asked::Got(picked.and_then(|option| match option {
            ChoiceOption::Color(color) => Some(Chosen::Color(color)),
            ChoiceOption::Label(name) => Some(Chosen::CreatureType(Subtype(name))),
            ChoiceOption::Player(_) => None,
        })))
    }

    /// What a player may choose as a permanent enters: the five colours (less the one a choice excludes), or the creature types of the cards in the game.
    fn options_of(&self, kind: EntersChoice) -> Vec<ChoiceOption> {
        match kind {
            EntersChoice::Color => Color::ALL
                .iter()
                .copied()
                .map(ChoiceOption::Color)
                .collect(),
            EntersChoice::ColorOtherThan(other) => Color::ALL
                .iter()
                .copied()
                .filter(|color| *color != other)
                .map(ChoiceOption::Color)
                .collect(),
            EntersChoice::CreatureType => {
                let mut types: Vec<String> = self
                    .state
                    .objects()
                    .filter(|o| o.card().is_type(CardType::Creature))
                    .flat_map(|o| o.card().subtypes.iter().map(|s| s.0.clone()))
                    .collect();
                types.sort();
                types.dedup();
                types.into_iter().map(ChoiceOption::Label).collect()
            }
            // The engine does not offer a player yet: no option, so nothing is chosen (`ReplaceBy.Choose.Player` and
            // `.Opponent` keep every card that uses them out of the game).
            EntersChoice::Player | EntersChoice::Opponent => Vec::new(),
        }
    }
}

/// A zone as a phrase.
fn zone_word(zone: Zone) -> &'static str {
    match zone {
        Zone::Library(_) => "a library",
        Zone::Hand(_) => "a hand",
        Zone::Battlefield | Zone::PhasedOut => "the battlefield",
        Zone::Graveyard(_) => "a graveyard",
        Zone::Stack => "the stack",
        Zone::Exile => "exile",
        Zone::Command(_) => "the command zone",
    }
}

mod dredge;

mod riot;

#[cfg(test)]
mod shields;

#[cfg(test)]
mod umbra;

#[cfg(test)]
mod tests {
    //! Goal: the replacement step hands a batch back unchanged when no replacement effect is in the game, applies each effect once to each event in the order the answers give, applies an effect about its own object before the others, removes an event that is replaced entirely, asks only when two or more effects apply and hands the batch back with the question, and refuses an answer that does not fit the question that was asked.
    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};

    const ME: PlayerId = PlayerId(0);

    fn replacement(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    /// A permanent with these replacement effects.
    fn with(name: &str, effects: &[&str]) -> std::sync::Arc<CardDefinition> {
        let mut card = CardDefinition::new(name).with_type(CardType::Enchantment);
        for effect in effects {
            card = card.with_rules(replacement(effect));
        }
        std::sync::Arc::new(card)
    }

    const DOUBLE_LIFE: &str = r#"{"Replacement":{"when":{"LifeGain":{}},"by":{"Multiply":2}}}"#;
    const PLUS_ONE_LIFE: &str = r#"{"Replacement":{"when":{"LifeGain":{}},"by":{"Add":1}}}"#;
    const NO_LIFE: &str = r#"{"Replacement":{"when":{"LifeGain":{}},"by":"Prevent"}}"#;

    fn gain(amount: u32) -> Event {
        Event::LifeGained { player: ME, amount }
    }

    fn events(result: Result<Processed, EngineError>) -> Vec<Event> {
        match result.unwrap() {
            Processed::Events(events) => events,
            other @ Processed::Ask { .. } => panic!("events were expected, got {other:?}"),
        }
    }

    fn table_of_two_effects() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![
                vec![
                    with("Doubler", &[DOUBLE_LIFE]),
                    with("Plus", &[PLUS_ONE_LIFE]),
                ],
                vec![],
            ],
        )
    }

    #[test]
    fn a_game_without_replacement_effects_pays_for_nothing_and_changes_nothing() {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![]],
        );
        assert!(!game.possible.replacements);
        let batch = vec![gain(2), Event::PriorityPassed { player: ME }];
        assert_eq!(events(process(&game, batch.clone(), &[])), batch);
        assert_eq!(events(process(&game, Vec::new(), &[])), Vec::new());
    }

    #[test]
    fn the_flag_follows_a_replacement_effect_of_any_card_in_the_game() {
        let game = state(
            vec![vec![with("Hand", &[DOUBLE_LIFE])], vec![]],
            vec![vec![], vec![]],
        );
        assert!(game.possible.replacements);
    }

    #[rstest]
    #[case::double(&[DOUBLE_LIFE], 3, vec![6])]
    #[case::plus_one(&[PLUS_ONE_LIFE], 3, vec![4])]
    #[case::prevented(&[NO_LIFE], 3, vec![])]
    fn an_effect_changes_the_event_it_replaces_once(
        #[case] effects: &[&str],
        #[case] amount: u32,
        #[case] gained: Vec<u32>,
    ) {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![with("Doubler", effects)], vec![]],
        );
        let result = events(process(&game, vec![gain(amount)], &[]));
        assert_eq!(result, gained.into_iter().map(gain).collect::<Vec<_>>());
    }

    #[test]
    fn two_effects_ask_and_the_answer_decides_what_the_second_one_sees() {
        let game = table_of_two_effects();
        let batch = vec![gain(3)];
        let Processed::Ask { decision, proposed } = process(&game, batch.clone(), &[]).unwrap()
        else {
            panic!("two effects apply, so the player is asked");
        };
        assert_eq!(proposed, batch);
        assert_eq!(decision.player, ME);
        let DecisionKind::ChooseReplacement(options) = decision.kind else {
            panic!("a replacement order was expected");
        };
        assert_eq!(options.candidates.len(), 2);
        // The candidates are in the order of their addresses: the doubler first.
        for (choice, total) in [(0, 7), (1, 8)] {
            let answers = [(ME, Choice::Replacement(choice))];
            assert_eq!(
                events(process(&game, batch.clone(), &answers)),
                vec![gain(total)]
            );
        }
    }

    #[rstest]
    #[case::wrong_player(vec![(PlayerId(1), Choice::Replacement(0))])]
    #[case::wrong_kind(vec![(ME, Choice::YesNo(true))])]
    #[case::out_of_range(vec![(ME, Choice::Replacement(2))])]
    #[case::one_too_many(vec![(ME, Choice::Replacement(0)), (ME, Choice::Replacement(0))])]
    fn an_answer_that_does_not_fit_the_question_is_refused(
        #[case] answers: Vec<(PlayerId, Choice)>,
    ) {
        let game = table_of_two_effects();
        assert!(process(&game, vec![gain(3)], &answers).is_err());
    }

    #[test]
    fn an_effect_applies_to_each_event_of_a_batch_and_to_none_of_another_players_life() {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![with("Doubler", &[DOUBLE_LIFE])], vec![]],
        );
        let foe = Event::LifeGained {
            player: PlayerId(1),
            amount: 5,
        };
        let batch = vec![gain(1), foe.clone(), gain(2)];
        assert_eq!(
            events(process(&game, batch, &[])),
            vec![gain(2), foe, gain(4)]
        );
    }

    #[test]
    fn a_self_replacement_effect_applies_before_the_others_and_is_not_asked() {
        // The permanent enters with two counters (its own effect) and another doubles counters put on permanents.
        let enters = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":2}}}}"#;
        let doubles = r#"{"Replacement":{"when":{"CountersPut":{}},"by":{"Multiply":2}}}"#;
        let hydra = std::sync::Arc::new(
            CardDefinition::new("Hydra")
                .with_type(CardType::Land)
                .with_rules(replacement(enters)),
        );
        let game = state(
            vec![vec![hydra], vec![]],
            vec![vec![with("Doubler", &[doubles])], vec![]],
        );
        let card = game.zone(Zone::Hand(ME))[0];
        let batch = vec![Event::LandPlayed {
            player: ME,
            object: card,
            face: None,
        }];
        let result = events(process(&game, batch, &[]));
        // The move, then the counters the effect added and the doubler doubled: four, with nobody asked.
        let counters: Vec<i32> = result
            .iter()
            .filter_map(|event| match event {
                Event::CountersChanged(changed) => Some(changed.delta),
                _ => None,
            })
            .collect();
        assert_eq!(counters, vec![4]);
        assert!(matches!(result[0], Event::LandPlayed { .. }));
    }
}
