//! The initial setup of a game and the state it builds.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::combat::CombatState;
use crate::error::SetupError;
use crate::format::FormatRules;
use crate::object::{GameObject, PermanentState};
use crate::player::{MulliganStatus, Player};
use crate::state::{GameState, Stage};
use crate::turn::{Step, TurnState};
use crate::zones::{Zone, Zones};

/// A permanent that starts on the battlefield (scenario setups).
#[derive(Debug, Clone)]
pub struct PresetPermanent {
    /// The card.
    pub card: Arc<CardDefinition>,
    /// Starts tapped.
    pub tapped: bool,
    /// Starts summoning sick.
    pub summoning_sick: bool,
    /// Damage already marked.
    pub damage: u32,
}

impl PresetPermanent {
    /// An untapped, ready permanent.
    pub fn new(card: Arc<CardDefinition>) -> Self {
        PresetPermanent {
            card,
            tapped: false,
            summoning_sick: false,
            damage: 0,
        }
    }
}

impl From<Arc<CardDefinition>> for PresetPermanent {
    fn from(card: Arc<CardDefinition>) -> Self {
        PresetPermanent::new(card)
    }
}

/// One seat's starting contents.
#[derive(Debug, Clone, Default)]
pub struct PlayerSetup {
    /// Library, top card first.
    pub library: Vec<Arc<CardDefinition>>,
    /// Starting hand (scenario setups; normal games deal from the library).
    pub hand: Vec<Arc<CardDefinition>>,
    /// Permanents already in play.
    pub battlefield: Vec<PresetPermanent>,
    /// Graveyard, oldest card first.
    pub graveyard: Vec<Arc<CardDefinition>>,
    /// Starting life; the format's default when `None`.
    pub life: Option<i32>,
}

/// Where the game starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// Pick a first player, shuffle, deal opening hands and mulligan.
    Mulligan,
    /// Skip the opening: the game begins at the start of this step (its turn-based actions still run).
    At {
        /// Turn number, from 1.
        turn: u32,
        /// The active player.
        active: PlayerId,
        /// The step.
        step: Step,
    },
}

/// Everything needed to reproduce a game: replaying events over this setup yields the state.
#[derive(Debug, Clone)]
pub struct GameSetup {
    /// The format's rules.
    pub format: Arc<dyn FormatRules>,
    /// Seed for shuffles and the first-player roll.
    pub seed: u64,
    /// Seats in turn order.
    pub players: Vec<PlayerSetup>,
    /// Where the game starts.
    pub start: Start,
    /// Whether decks are checked against the format (off for scenario setups).
    pub validate_decks: bool,
}

impl GameSetup {
    /// A normal game start with one deck per seat.
    pub fn new(
        format: Arc<dyn FormatRules>,
        seed: u64,
        decks: Vec<Vec<Arc<CardDefinition>>>,
    ) -> Self {
        GameSetup {
            format,
            seed,
            players: decks
                .into_iter()
                .map(|library| PlayerSetup {
                    library,
                    ..PlayerSetup::default()
                })
                .collect(),
            start: Start::Mulligan,
            validate_decks: true,
        }
    }

    /// Checks seat count, decks and the starting point.
    pub fn validate(&self) -> Result<(), SetupError> {
        if self.players.len() < 2 {
            return Err(SetupError::TooFewPlayers(self.players.len()));
        }
        if self.validate_decks {
            for (seat, player) in self.players.iter().enumerate() {
                let deck: Vec<_> = player
                    .library
                    .iter()
                    .chain(&player.hand)
                    .chain(player.battlefield.iter().map(|p| &p.card))
                    .chain(&player.graveyard)
                    .cloned()
                    .collect();
                self.format
                    .validate_deck(&deck)
                    .map_err(|source| SetupError::InvalidDeck {
                        player: PlayerId::from_index(seat),
                        source,
                    })?;
            }
        }
        if let Start::At { turn, active, step } = self.start {
            if turn == 0 {
                return Err(SetupError::InvalidStart("turns start at 1"));
            }
            if matches!(
                step,
                Step::DeclareBlockers | Step::FirstStrikeDamage | Step::CombatDamage
            ) {
                return Err(SetupError::InvalidStart(
                    "combat can only be entered at declare attackers",
                ));
            }
            if active.index() >= self.players.len() {
                return Err(SetupError::InvalidStart("active player does not exist"));
            }
        }
        Ok(())
    }
}

impl GameState {
    /// Builds the initial state; objects get sequential ids per seat: library, hand, battlefield, graveyard.
    pub fn from_setup(setup: &GameSetup) -> Result<GameState, SetupError> {
        setup.validate()?;
        let seats = setup.players.len();
        let life = setup.format.starting_life();
        let mut objects = BTreeMap::new();
        let mut zones = Zones::new(seats);
        let mut players = Vec::with_capacity(seats);
        let mut next_id = 0u64;
        let mut create = |owner: PlayerId, card: &Arc<CardDefinition>, zone: Zone| {
            let id = ObjectId(next_id);
            next_id += 1;
            zones.insert(zone, id, crate::zones::Placement::Top);
            objects.insert(id, GameObject::new(id, owner, Arc::clone(card), zone));
            id
        };
        let mut presets = Vec::new();
        for (seat, setup_player) in setup.players.iter().enumerate() {
            let owner = PlayerId::from_index(seat);
            for card in setup_player.library.iter().rev() {
                create(owner, card, Zone::Library(owner));
            }
            for card in &setup_player.hand {
                create(owner, card, Zone::Hand(owner));
            }
            for preset in &setup_player.battlefield {
                presets.push((
                    create(owner, &preset.card, Zone::Battlefield),
                    preset.clone(),
                ));
            }
            for card in &setup_player.graveyard {
                create(owner, card, Zone::Graveyard(owner));
            }
            let status = match setup.start {
                Start::Mulligan => MulliganStatus::NeedsHand,
                Start::At { .. } => MulliganStatus::Kept,
            };
            players.push(Player::new(
                owner,
                setup_player.life.unwrap_or(life),
                status,
            ));
        }
        let triggers_possible = objects
            .values()
            .any(|object| crate::triggers::can_trigger(object.card()));
        for (id, preset) in presets {
            if let Some(object) = objects.get_mut(&id) {
                object.permanent = Some(PermanentState {
                    tapped: preset.tapped,
                    damage: preset.damage,
                    summoning_sick: preset.summoning_sick,
                });
            }
        }
        let (stage, turn, first_player) = match setup.start {
            Start::Mulligan => (Stage::PreGame, (0, PlayerId(0), Step::Untap), PlayerId(0)),
            Start::At { turn, active, step } => {
                let back = usize::try_from(turn - 1).unwrap_or(0) % seats;
                let first = PlayerId::from_index((active.index() + seats - back) % seats);
                (Stage::Turns, (turn, active, step), first)
            }
        };
        Ok(GameState {
            format: Arc::clone(&setup.format),
            seed: setup.seed,
            rng_index: 0,
            stage,
            first_player,
            players,
            objects,
            zones,
            turn: TurnState {
                number: turn.0,
                active: turn.1,
                step: turn.2,
                lands_played: 0,
            },
            priority: None,
            combat: CombatState::default(),
            effects: Vec::new(),
            next_effect: 0,
            next_timestamp: 0,
            next_object: next_id,
            resolving: None,
            pending_triggers: Vec::new(),
            trigger_round: 0,
            delayed: Vec::new(),
            next_delayed: 0,
            limits_spent: BTreeSet::new(),
            triggers_possible,
            result: None,
        })
    }
}
