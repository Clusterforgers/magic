//! Manual actions: what the holder of priority does by hand for a card the engine does not run, as the same events the
//! effects produce.
//!
//! `PriorityAction::Manual` carries a [`ManualAction`]. The seat that has priority may take one at any time; it is
//! checked here against the state, answered with the `Event::ManualAction` that names the seat and the action and the
//! events of the change (a `ZoneChanged`, a `CountersChanged`, a `TokenCreated`, a `LifeLost`), and committed like the
//! events of any effect, so replacement effects, triggers and state-based actions follow it as they follow a spell.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::assemble::token_definition;
use mtg_cards::card::TokenSpec;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::attach;
use crate::caps::caps;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::{Event, ZoneCause};
use crate::format::ExtraZone;
use crate::loyalty;
use crate::object::{GameObject, ObjectKind, PermanentState};
use crate::rules::{cant_draw, cant_gain_life};
use crate::state::GameState;
use crate::steps::draw;
use crate::tokens::TokenCreated;
use crate::zones::Zone;

/// Most cards one action moves or draws.
pub const MAX_CARDS: u32 = 10;
/// Most tokens one action creates.
pub const MAX_TOKENS: u32 = 10;
/// Most counters one action puts on or takes off.
pub const MAX_COUNTERS: i32 = 99;
/// Most life one action gains or loses.
pub const MAX_LIFE: i32 = 1000;
/// Most characters of a custom token's name.
pub const MAX_NAME_CHARS: usize = 40;
/// Most characters of a subtype or of the name of a kind of counter.
pub const MAX_WORD_CHARS: usize = 24;
/// Most subtypes of a custom token.
pub const MAX_SUBTYPES: usize = 6;
/// Largest power or toughness of a custom token.
pub const MAX_STAT: i32 = 99;

/// Where a card goes: a zone of its owner, or the shared battlefield or exile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ManualZone {
    /// The battlefield, under its owner's control.
    Battlefield,
    /// Its owner's hand.
    Hand,
    /// Its owner's graveyard.
    Graveyard,
    /// Exile.
    Exile,
    /// The top of its owner's library.
    LibraryTop,
    /// The bottom of its owner's library.
    LibraryBottom,
    /// Its owner's command zone, in a game that has one.
    Command,
}

impl ManualZone {
    /// The zone this names for a card of `owner`.
    pub const fn zone(self, owner: PlayerId) -> Zone {
        match self {
            ManualZone::Battlefield => Zone::Battlefield,
            ManualZone::Hand => Zone::Hand(owner),
            ManualZone::Graveyard => Zone::Graveyard(owner),
            ManualZone::Exile => Zone::Exile,
            ManualZone::LibraryTop | ManualZone::LibraryBottom => Zone::Library(owner),
            ManualZone::Command => Zone::Command(owner),
        }
    }
}

/// A token from the list of the common ones; the first three are the predefined tokens of `cards/tokens`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum CommonToken {
    /// A Treasure artifact.
    Treasure,
    /// A Clue artifact.
    Clue,
    /// A Food artifact.
    Food,
    /// A white 1/1 Soldier creature.
    Soldier,
    /// A black 2/2 Zombie creature.
    Zombie,
    /// A red 1/1 Goblin creature.
    Goblin,
    /// A white 1/1 Spirit creature with flying.
    Spirit,
    /// A green 1/1 Saproling creature.
    Saproling,
    /// A green 3/3 Beast creature.
    Beast,
    /// A colorless 1/1 Thopter artifact creature with flying.
    Thopter,
    /// A colorless 1/1 Servo artifact creature.
    Servo,
    /// A white 1/1 Human creature.
    Human,
    /// A green 1/1 Insect creature.
    Insect,
    /// A green 1/1 Squirrel creature.
    Squirrel,
    /// A green 2/2 Wolf creature.
    Wolf,
    /// A white 1/1 Bird creature with flying.
    Bird,
    /// A red 4/4 Dragon creature with flying.
    Dragon,
    /// A white 4/4 Angel creature with flying.
    Angel,
}

impl CommonToken {
    /// Every token of the list, in the order a menu shows them.
    pub const ALL: [CommonToken; 18] = [
        CommonToken::Treasure,
        CommonToken::Clue,
        CommonToken::Food,
        CommonToken::Soldier,
        CommonToken::Zombie,
        CommonToken::Goblin,
        CommonToken::Spirit,
        CommonToken::Saproling,
        CommonToken::Beast,
        CommonToken::Thopter,
        CommonToken::Servo,
        CommonToken::Human,
        CommonToken::Insect,
        CommonToken::Squirrel,
        CommonToken::Wolf,
        CommonToken::Bird,
        CommonToken::Dragon,
        CommonToken::Angel,
    ];

    /// What the token is, or `None` when a predefined token's file is missing.
    fn spec(self) -> Option<TokenSpec> {
        let predefined = |id: &str| {
            mtg_cards::builtin::library()
                .ok()
                .and_then(|library| library.token(id))
                .map(|file| file.token.clone())
        };
        let creature = |name: &str, colors: &[Color], stats: (i32, i32), flying: bool| TokenSpec {
            name: name.to_owned(),
            colors: colors.to_vec(),
            supertypes: Vec::new(),
            types: vec![CardType::Creature],
            subtypes: vec![Subtype(name.to_owned())],
            power: Some(stats.0),
            toughness: Some(stats.1),
            abilities: if flying {
                vec![Ability::Keyword(Keyword::Flying)]
            } else {
                Vec::new()
            },
        };
        let artifact_creature = |name: &str, flying: bool| TokenSpec {
            types: vec![CardType::Artifact, CardType::Creature],
            ..creature(name, &[], (1, 1), flying)
        };
        Some(match self {
            CommonToken::Treasure => predefined("treasure")?,
            CommonToken::Clue => predefined("clue")?,
            CommonToken::Food => predefined("food")?,
            CommonToken::Soldier => creature("Soldier", &[Color::White], (1, 1), false),
            CommonToken::Zombie => creature("Zombie", &[Color::Black], (2, 2), false),
            CommonToken::Goblin => creature("Goblin", &[Color::Red], (1, 1), false),
            CommonToken::Spirit => creature("Spirit", &[Color::White], (1, 1), true),
            CommonToken::Saproling => creature("Saproling", &[Color::Green], (1, 1), false),
            CommonToken::Beast => creature("Beast", &[Color::Green], (3, 3), false),
            CommonToken::Thopter => artifact_creature("Thopter", true),
            CommonToken::Servo => artifact_creature("Servo", false),
            CommonToken::Human => creature("Human", &[Color::White], (1, 1), false),
            CommonToken::Insect => creature("Insect", &[Color::Green], (1, 1), false),
            CommonToken::Squirrel => creature("Squirrel", &[Color::Green], (1, 1), false),
            CommonToken::Wolf => creature("Wolf", &[Color::Green], (2, 2), false),
            CommonToken::Bird => creature("Bird", &[Color::White], (1, 1), true),
            CommonToken::Dragon => creature("Dragon", &[Color::Red], (4, 4), true),
            CommonToken::Angel => creature("Angel", &[Color::White], (4, 4), true),
        })
    }
}

/// A token the acting seat describes: a name, colours, types, subtypes and, for a creature, power and toughness.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CustomToken {
    /// What it is called.
    pub name: String,
    /// Its colours.
    pub colors: Vec<Color>,
    /// Its card types: creature, artifact, enchantment or land.
    pub types: Vec<CardType>,
    /// Its subtypes.
    pub subtypes: Vec<String>,
    /// Its power, which a creature has and nothing else may.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub power: Option<i32>,
    /// Its toughness, which a creature has and nothing else may.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub toughness: Option<i32>,
}

impl CustomToken {
    /// What the token is, or why the description cannot be a token.
    fn spec(&self) -> Result<TokenSpec, ManualRefusal> {
        let bad = ManualRefusal::BadToken;
        let name = self.name.trim();
        if name.is_empty()
            || name.chars().count() > MAX_NAME_CHARS
            || name.chars().any(char::is_control)
        {
            return Err(bad("a name of 1 to 40 characters"));
        }
        let allowed = |ty: &CardType| {
            matches!(
                ty,
                CardType::Creature | CardType::Artifact | CardType::Enchantment | CardType::Land
            )
        };
        if self.types.is_empty() || !self.types.iter().all(allowed) {
            return Err(bad("creature, artifact, enchantment or land as its types"));
        }
        if self.subtypes.len() > MAX_SUBTYPES
            || self.subtypes.iter().any(|word| {
                let word = word.trim();
                word.is_empty()
                    || word.chars().count() > MAX_WORD_CHARS
                    || !word
                        .chars()
                        .all(|c| c.is_alphabetic() || matches!(c, ' ' | '-' | '\''))
            })
        {
            return Err(bad("up to 6 subtypes of letters"));
        }
        let creature = self.types.contains(&CardType::Creature);
        let stats = match (creature, self.power.zip(self.toughness)) {
            (true, Some((power, toughness)))
                if (0..=MAX_STAT).contains(&power) && (0..=MAX_STAT).contains(&toughness) =>
            {
                Some((power, toughness))
            }
            (false, None) if self.power.is_none() && self.toughness.is_none() => None,
            _ => return Err(bad("power and toughness from 0 to 99 on a creature only")),
        };
        let mut colors = self.colors.clone();
        colors.sort_unstable();
        colors.dedup();
        let mut types = self.types.clone();
        types.sort_unstable();
        types.dedup();
        Ok(TokenSpec {
            name: name.to_owned(),
            colors,
            supertypes: Vec::new(),
            types,
            subtypes: self
                .subtypes
                .iter()
                .map(|word| Subtype(word.trim().to_owned()))
                .collect(),
            power: stats.map(|(power, _)| power),
            toughness: stats.map(|(_, toughness)| toughness),
            abilities: Vec::new(),
        })
    }
}

/// The token a manual action creates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ManualToken {
    /// One of the common tokens.
    Common(CommonToken),
    /// A token the seat describes.
    Custom(Box<CustomToken>),
}

/// What the holder of priority does by hand. Zone moves, discards and attachments work on what the seat owns or
/// controls; counters, tapping and untapping on any permanent; life and a player's counters on the seat's own.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ManualAction {
    /// Move a card or token of the seat from the battlefield, its hand, graveyard, exile or command zone to a zone of its
    /// owner. A token that leaves the battlefield ceases to exist.
    Move {
        /// The object.
        object: ObjectId,
        /// Where it goes.
        to: ManualZone,
    },
    /// Move the top cards of the seat's library, the top card first, to its hand, graveyard, exile or the bottom of the
    /// library.
    MoveTop {
        /// How many cards, 1 to 10.
        count: u32,
        /// Where they go.
        to: ManualZone,
    },
    /// Put counters on a permanent (a positive `delta`) or take them off (a negative one).
    Counters {
        /// The permanent.
        object: ObjectId,
        /// The kind of counter.
        counter: CounterKind,
        /// How many, from -99 to 99 and not 0.
        delta: i32,
    },
    /// Put counters on the seat (poison, energy, experience) or take them off.
    PlayerCounters {
        /// The seat itself.
        player: PlayerId,
        /// The kind of counter.
        counter: CounterKind,
        /// How many, from -99 to 99 and not 0.
        delta: i32,
    },
    /// The seat gains (a positive `delta`) or loses life.
    Life {
        /// The seat itself.
        player: PlayerId,
        /// How much, from -1000 to 1000 and not 0.
        delta: i32,
    },
    /// Create tokens under the seat's control.
    CreateToken {
        /// What they are.
        token: ManualToken,
        /// How many, 1 to 10.
        count: u32,
    },
    /// Tap an untapped permanent.
    Tap {
        /// The permanent.
        object: ObjectId,
    },
    /// Untap a tapped permanent.
    Untap {
        /// The permanent.
        object: ObjectId,
    },
    /// The seat draws cards.
    Draw {
        /// How many, 1 to 10.
        count: u32,
    },
    /// The seat discards a card from its hand.
    Discard {
        /// The card.
        object: ObjectId,
    },
    /// Attach an Aura, Equipment or Fortification of the seat to a permanent.
    Attach {
        /// The attachment.
        object: ObjectId,
        /// The permanent it is attached to.
        to: ObjectId,
    },
    /// Take an Aura, Equipment or Fortification of the seat off what it is attached to.
    Unattach {
        /// The attachment.
        object: ObjectId,
    },
}

/// Why a manual action is refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ManualRefusal {
    /// The object is not in the game, or is an ability.
    #[error("{0:?} is not a card or permanent in the game")]
    Unknown(ObjectId),
    /// The seat neither owns nor controls the object, or names another seat's life or counters.
    #[error("a seat changes only what it owns or controls and its own life and counters")]
    NotYours,
    /// The object is in a zone this action does not work on.
    #[error("{object:?} is in {zone:?}, where this action does not apply")]
    WrongZone {
        /// The object.
        object: ObjectId,
        /// Where it is.
        zone: Zone,
    },
    /// The action would change nothing.
    #[error("nothing would change")]
    NoChange,
    /// A number is out of its range.
    #[error("{0} is out of range")]
    OutOfRange(&'static str),
    /// There are fewer counters or cards than the action takes.
    #[error("there are not enough {0}")]
    NotEnough(&'static str),
    /// The destination does not exist in this game, or the card cannot go there.
    #[error("the card cannot go to {0:?}")]
    NoDestination(ManualZone),
    /// A rule in force keeps the seat from doing it.
    #[error("a rule keeps the seat from {0}")]
    Forbidden(&'static str),
    /// The object is not an Aura, Equipment or Fortification, or the host is not a permanent it can be attached to.
    #[error("{0:?} cannot be attached this way")]
    NotAttachable(ObjectId),
    /// A custom token or a kind of counter is not acceptable.
    #[error("a token or counter needs {0}")]
    BadToken(&'static str),
}

/// Whether `player` may take `action` in `state`, or why not; the check `PriorityAction::Manual` answers to.
pub fn check(
    state: &GameState,
    player: PlayerId,
    action: &ManualAction,
) -> Result<(), ManualRefusal> {
    events(state, player, action).map(drop)
}

/// The events of `action` taken by `player`, who holds priority: the `Event::ManualAction` that names it and the events of
/// the change. Nothing is changed when the action is refused.
pub(crate) fn events(
    state: &GameState,
    player: PlayerId,
    action: &ManualAction,
) -> Result<Vec<Event>, ManualRefusal> {
    let changes = match action {
        ManualAction::Move { object, to } => moves(state, player, *object, *to)?,
        ManualAction::MoveTop { count, to } => move_top(state, player, *count, *to)?,
        ManualAction::Counters {
            object,
            counter,
            delta,
        } => {
            let held = permanent(state, *object)?;
            let on = Target::Object(held.reference());
            let now = held.permanent().map_or(0, |p| p.counters_of(counter));
            counters(on, counter, *delta, now)?
        }
        ManualAction::PlayerCounters {
            player: who,
            counter,
            delta,
        } => {
            let own = own_seat(state, player, *who)?;
            counters(
                Target::Player(*who),
                counter,
                *delta,
                own.counters_of(counter),
            )?
        }
        ManualAction::Life { player: who, delta } => {
            own_seat(state, player, *who)?;
            life(state, *who, *delta)?
        }
        ManualAction::CreateToken { token, count } => create_tokens(state, player, token, *count)?,
        ManualAction::Tap { object } => {
            let held = permanent(state, *object)?;
            if held.permanent().is_some_and(PermanentState::tapped) {
                return Err(ManualRefusal::NoChange);
            }
            vec![Event::PermanentTapped { object: *object }]
        }
        ManualAction::Untap { object } => {
            let held = permanent(state, *object)?;
            if !held.permanent().is_some_and(PermanentState::tapped) {
                return Err(ManualRefusal::NoChange);
            }
            vec![Event::PermanentUntapped { object: *object }]
        }
        ManualAction::Draw { count } => {
            if !(1..=MAX_CARDS).contains(count) {
                return Err(ManualRefusal::OutOfRange("the number of cards"));
            }
            if cant_draw(state, player) {
                return Err(ManualRefusal::Forbidden("drawing"));
            }
            draw(state, player, usize::try_from(*count).unwrap_or(0))
        }
        ManualAction::Discard { object } => {
            let card = own(state, player, *object)?;
            if card.zone() != Zone::Hand(player) {
                return Err(ManualRefusal::WrongZone {
                    object: *object,
                    zone: card.zone(),
                });
            }
            vec![Event::CardDiscarded {
                player,
                object: *object,
            }]
        }
        ManualAction::Attach { object, to } => attach(state, player, *object, *to)?,
        ManualAction::Unattach { object } => {
            let attachment = attachment(state, player, *object)?;
            if state.host_of(attachment.id()).is_none() {
                return Err(ManualRefusal::NoChange);
            }
            vec![Event::Unattached { object: *object }]
        }
    };
    let mut events = Vec::with_capacity(changes.len() + 1);
    events.push(Event::ManualAction {
        player,
        action: Box::new(action.clone()),
    });
    events.extend(changes);
    Ok(events)
}

/// The object `id` when the seat owns or controls it.
fn own(state: &GameState, player: PlayerId, id: ObjectId) -> Result<&GameObject, ManualRefusal> {
    let object = state
        .object(id)
        .filter(|object| object.kind() != ObjectKind::Ability)
        .ok_or(ManualRefusal::Unknown(id))?;
    if object.owner() == player || object.controller() == player {
        Ok(object)
    } else {
        Err(ManualRefusal::NotYours)
    }
}

/// The permanent `id`, whoever controls it.
fn permanent(state: &GameState, id: ObjectId) -> Result<&GameObject, ManualRefusal> {
    let object = state
        .object(id)
        .filter(|object| object.kind() != ObjectKind::Ability)
        .ok_or(ManualRefusal::Unknown(id))?;
    if object.zone() == Zone::Battlefield {
        Ok(object)
    } else {
        Err(ManualRefusal::WrongZone {
            object: id,
            zone: object.zone(),
        })
    }
}

/// The seat `who` when it is the acting seat and still in the game.
fn own_seat(
    state: &GameState,
    player: PlayerId,
    who: PlayerId,
) -> Result<&crate::player::Player, ManualRefusal> {
    if who != player {
        return Err(ManualRefusal::NotYours);
    }
    state
        .player(who)
        .filter(|seat| seat.is_in_game())
        .ok_or(ManualRefusal::NotYours)
}

fn moves(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    to: ManualZone,
) -> Result<Vec<Event>, ManualRefusal> {
    let object = own(state, player, id)?;
    let from = object.zone();
    // A library card is not one the seat can name, and the stack holds spells that resolve or are countered.
    if matches!(from, Zone::Stack | Zone::Library(_)) {
        return Err(ManualRefusal::WrongZone {
            object: id,
            zone: from,
        });
    }
    let owner = object.owner();
    let there = to.zone(owner);
    let has_command_zone = state.format().extra_zones().contains(&ExtraZone::Command);
    if !state.zones.exists(there) || (to == ManualZone::Command && !has_command_zone) {
        return Err(ManualRefusal::NoDestination(to));
    }
    let spell = object.base_types().contains(CardType::Instant)
        || object.base_types().contains(CardType::Sorcery);
    if there == from && !matches!(to, ManualZone::LibraryTop | ManualZone::LibraryBottom) {
        return Err(ManualRefusal::NoChange);
    }
    if to == ManualZone::Battlefield && spell {
        return Err(ManualRefusal::NoDestination(to));
    }
    let mut events = vec![Event::ZoneChanged {
        object: id,
        from,
        to: there,
        cause: ZoneCause::Moved,
    }];
    if to == ManualZone::LibraryBottom && object.kind() != ObjectKind::Token {
        events.push(Event::LibraryBottomed {
            player: owner,
            cards: vec![id],
            random: false,
        });
    }
    if to == ManualZone::Battlefield {
        events.extend(loyalty::entering(state, id));
    }
    Ok(events)
}

fn move_top(
    state: &GameState,
    player: PlayerId,
    count: u32,
    to: ManualZone,
) -> Result<Vec<Event>, ManualRefusal> {
    if !(1..=MAX_CARDS).contains(&count) {
        return Err(ManualRefusal::OutOfRange("the number of cards"));
    }
    if !matches!(
        to,
        ManualZone::Hand | ManualZone::Graveyard | ManualZone::Exile | ManualZone::LibraryBottom
    ) {
        return Err(ManualRefusal::NoDestination(to));
    }
    let library = Zone::Library(player);
    let top: Vec<ObjectId> = state
        .zone(library)
        .iter()
        .rev()
        .take(usize::try_from(count).unwrap_or(0))
        .copied()
        .collect();
    if top.len() < usize::try_from(count).unwrap_or(usize::MAX) {
        return Err(ManualRefusal::NotEnough("cards in the library"));
    }
    if to == ManualZone::LibraryBottom {
        return Ok(vec![Event::LibraryBottomed {
            player,
            cards: top,
            random: false,
        }]);
    }
    Ok(top
        .into_iter()
        .map(|object| Event::ZoneChanged {
            object,
            from: library,
            to: to.zone(player),
            cause: ZoneCause::Moved,
        })
        .collect())
}

/// A change of `delta` counters of `kind` on `on`, which holds `now` of them.
fn counters(
    on: Target,
    kind: &CounterKind,
    delta: i32,
    now: u32,
) -> Result<Vec<Event>, ManualRefusal> {
    if delta == 0 || delta.abs() > MAX_COUNTERS {
        return Err(ManualRefusal::OutOfRange("the number of counters"));
    }
    if let CounterKind::Other(name) = kind {
        let name = name.trim();
        if name.is_empty()
            || name.chars().count() > MAX_WORD_CHARS
            || !name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '+' | '/'))
        {
            return Err(ManualRefusal::BadToken("a name of letters"));
        }
    }
    if delta < 0 && now < delta.unsigned_abs() {
        return Err(ManualRefusal::NotEnough("counters there"));
    }
    Ok(vec![Event::CountersChanged(CountersChanged {
        on,
        counter: kind.clone(),
        delta,
    })])
}

fn life(state: &GameState, who: PlayerId, delta: i32) -> Result<Vec<Event>, ManualRefusal> {
    if delta == 0 || delta.abs() > MAX_LIFE {
        return Err(ManualRefusal::OutOfRange("the life"));
    }
    let amount = delta.unsigned_abs();
    if delta > 0 {
        if cant_gain_life(state, who) {
            return Err(ManualRefusal::Forbidden("gaining life"));
        }
        Ok(vec![Event::LifeGained {
            player: who,
            amount,
        }])
    } else {
        Ok(vec![Event::LifeLost {
            player: who,
            amount,
        }])
    }
}

fn create_tokens(
    state: &GameState,
    player: PlayerId,
    token: &ManualToken,
    count: u32,
) -> Result<Vec<Event>, ManualRefusal> {
    if !(1..=MAX_TOKENS).contains(&count) {
        return Err(ManualRefusal::OutOfRange("the number of tokens"));
    }
    let spec = match token {
        ManualToken::Common(common) => common
            .spec()
            .ok_or(ManualRefusal::BadToken("a token of the list"))?,
        ManualToken::Custom(custom) => custom.spec()?,
    };
    let definition = Arc::new(token_definition(&spec, &caps()));
    let first = state.next_object().get();
    Ok((0..u64::from(count))
        .map(|made| {
            Event::TokenCreated(TokenCreated {
                object: ObjectId(first + made),
                copy_of: None,
                controller: player,
                name: definition.name.clone(),
                tapped: false,
                attacking: None,
                definition: Arc::clone(&definition),
            })
        })
        .collect())
}

/// The attachment `id` when the seat controls it on the battlefield.
fn attachment(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
) -> Result<&GameObject, ManualRefusal> {
    let object = own(state, player, id)?;
    if object.zone() != Zone::Battlefield {
        return Err(ManualRefusal::WrongZone {
            object: id,
            zone: object.zone(),
        });
    }
    if attach::kind_of(state, id).is_none() {
        return Err(ManualRefusal::NotAttachable(id));
    }
    Ok(object)
}

fn attach(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    to: ObjectId,
) -> Result<Vec<Event>, ManualRefusal> {
    attachment(state, player, id)?;
    permanent(state, to)?;
    if id == to {
        return Err(ManualRefusal::NotAttachable(id));
    }
    if state.host_of(id).is_some_and(|host| host == to) {
        return Err(ManualRefusal::NoChange);
    }
    Ok(vec![Event::Attached { object: id, to }])
}

#[cfg(test)]
mod tests {
    //! Goal: every token of the common list is a real description, a custom token is accepted only as a name, types, subtypes and a size that a token can have, a zone names the zone of the card's owner, and an action keeps the wire form it was given.
    use rstest::rstest;

    use super::*;

    #[test]
    fn every_common_token_is_described() {
        for token in CommonToken::ALL {
            let spec = token.spec().unwrap_or_else(|| panic!("{token:?}"));
            assert!(!spec.name.is_empty() && !spec.types.is_empty(), "{token:?}");
            let creature = spec.types.contains(&CardType::Creature);
            assert_eq!(spec.power.is_some(), creature, "{token:?}");
            assert_eq!(spec.toughness.is_some(), creature, "{token:?}");
        }
        let names: std::collections::BTreeSet<_> = CommonToken::ALL
            .iter()
            .filter_map(|token| token.spec())
            .map(|spec| spec.name)
            .collect();
        assert_eq!(
            names.len(),
            CommonToken::ALL.len(),
            "each is called differently"
        );
    }

    fn custom(
        name: &str,
        types: &[CardType],
        subtypes: &[&str],
        size: Option<(i32, i32)>,
    ) -> CustomToken {
        CustomToken {
            name: name.into(),
            colors: vec![Color::Red, Color::Red, Color::Blue],
            types: types.to_vec(),
            subtypes: subtypes.iter().map(|s| (*s).to_owned()).collect(),
            power: size.map(|(power, _)| power),
            toughness: size.map(|(_, toughness)| toughness),
        }
    }

    #[rstest]
    #[case::a_creature(custom("Imp", &[CardType::Creature], &["Imp"], Some((1, 2))), true)]
    #[case::a_name_with_spaces_trimmed(custom(" Big Imp ", &[CardType::Creature], &[], Some((0, 0))), true)]
    #[case::an_artifact_creature(custom("Gizmo", &[CardType::Artifact, CardType::Creature], &["Construct"], Some((99, 99))), true)]
    #[case::a_land(custom("Isle", &[CardType::Land], &["Island"], None), true)]
    #[case::an_enchantment(custom("Aura of Calm", &[CardType::Enchantment], &[], None), true)]
    #[case::a_name_of_forty_characters(custom(&"x".repeat(40), &[CardType::Artifact], &[], None), true)]
    #[case::a_name_too_long(custom(&"x".repeat(41), &[CardType::Artifact], &[], None), false)]
    #[case::no_name(custom("", &[CardType::Artifact], &[], None), false)]
    #[case::a_control_character(custom("Im\np", &[CardType::Artifact], &[], None), false)]
    #[case::no_type(custom("Imp", &[], &[], None), false)]
    #[case::a_type_that_is_no_token(custom("Imp", &[CardType::Planeswalker], &[], None), false)]
    #[case::a_creature_with_no_size(custom("Imp", &[CardType::Creature], &[], None), false)]
    #[case::a_land_with_a_size(custom("Isle", &[CardType::Land], &[], Some((1, 1))), false)]
    #[case::a_negative_power(custom("Imp", &[CardType::Creature], &[], Some((-1, 1))), false)]
    #[case::a_huge_toughness(custom("Imp", &[CardType::Creature], &[], Some((1, 100))), false)]
    #[case::a_subtype_with_a_digit(custom("Imp", &[CardType::Creature], &["Imp2"], Some((1, 1))), false)]
    #[case::a_blank_subtype(custom("Imp", &[CardType::Creature], &[" "], Some((1, 1))), false)]
    #[case::too_many_subtypes(custom("Imp", &[CardType::Creature], &["A", "B", "C", "D", "E", "F", "G"], Some((1, 1))), false)]
    fn a_custom_token_is_a_token_or_is_refused(#[case] token: CustomToken, #[case] accepted: bool) {
        assert_eq!(token.spec().is_ok(), accepted, "{token:?}");
    }

    #[test]
    fn a_custom_token_is_described_once_whatever_it_repeats() {
        let spec = custom(
            " Imp ",
            &[CardType::Creature, CardType::Creature],
            &[" Imp "],
            Some((1, 2)),
        )
        .spec()
        .unwrap();
        assert_eq!(spec.name, "Imp");
        assert_eq!(spec.colors, [Color::Blue, Color::Red]);
        assert_eq!(spec.types, [CardType::Creature]);
        assert_eq!(spec.subtypes, [Subtype("Imp".into())]);
    }

    #[rstest]
    #[case(ManualZone::Battlefield, Zone::Battlefield)]
    #[case(ManualZone::Hand, Zone::Hand(PlayerId(2)))]
    #[case(ManualZone::Graveyard, Zone::Graveyard(PlayerId(2)))]
    #[case(ManualZone::Exile, Zone::Exile)]
    #[case(ManualZone::LibraryTop, Zone::Library(PlayerId(2)))]
    #[case(ManualZone::LibraryBottom, Zone::Library(PlayerId(2)))]
    #[case(ManualZone::Command, Zone::Command(PlayerId(2)))]
    fn a_zone_names_the_zone_of_the_cards_owner(#[case] to: ManualZone, #[case] expected: Zone) {
        assert_eq!(to.zone(PlayerId(2)), expected);
    }

    #[rstest]
    #[case(ManualAction::Move { object: ObjectId(3), to: ManualZone::LibraryTop }, r#"{"Move":{"object":3,"to":"LibraryTop"}}"#)]
    #[case(ManualAction::Life { player: PlayerId(1), delta: -2 }, r#"{"Life":{"player":1,"delta":-2}}"#)]
    #[case(ManualAction::CreateToken { token: ManualToken::Common(CommonToken::Treasure), count: 2 }, r#"{"CreateToken":{"token":{"Common":"Treasure"},"count":2}}"#)]
    #[case(ManualAction::Counters { object: ObjectId(4), counter: CounterKind::PlusOnePlusOne, delta: 1 }, r#"{"Counters":{"object":4,"counter":"+1/+1","delta":1}}"#)]
    fn an_action_has_the_wire_form_clients_send(#[case] action: ManualAction, #[case] json: &str) {
        assert_eq!(serde_json::to_string(&action).unwrap(), json);
        assert_eq!(serde_json::from_str::<ManualAction>(json).unwrap(), action);
    }
}
