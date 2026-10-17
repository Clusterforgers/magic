//! Scenario builder: "given these cards in these zones, it is turn N, step X".

use std::sync::Arc;

use mtg_core::card::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_engine::PlayerSetup;
use mtg_engine::format::{FormatRules, Normal};
use mtg_engine::turn::Step;
use mtg_engine::{Engine, EngineError, GameSetup, PresetPermanent, Start};
use thiserror::Error;

use crate::cards::forest;

/// Cards in a default filler library, so scenarios never deck unless asked to.
const FILLER_LIBRARY: usize = 30;

/// A scenario that cannot be built.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ScenarioError {
    /// A builder call named a player that is not seated.
    #[error("scenario has no player {0}")]
    NoSuchPlayer(usize),
    /// The engine rejected the setup.
    #[error(transparent)]
    Engine(#[from] EngineError),
}

/// A permanent that starts untapped and ready to act.
pub fn ready(card: Arc<CardDefinition>) -> PresetPermanent {
    PresetPermanent::new(card)
}

/// A permanent that starts tapped.
pub fn tapped(card: Arc<CardDefinition>) -> PresetPermanent {
    PresetPermanent {
        tapped: true,
        ..PresetPermanent::new(card)
    }
}

/// A permanent that starts summoning sick.
pub fn sick(card: Arc<CardDefinition>) -> PresetPermanent {
    PresetPermanent {
        summoning_sick: true,
        ..PresetPermanent::new(card)
    }
}

/// A permanent that starts with `damage` marked on it.
pub fn damaged(card: Arc<CardDefinition>, damage: u32) -> PresetPermanent {
    PresetPermanent {
        damage,
        ..PresetPermanent::new(card)
    }
}

/// Builds a game state directly instead of playing to it.
#[derive(Debug, Clone)]
pub struct Scenario {
    players: Vec<PlayerSetup>,
    library_given: Vec<bool>,
    format: Arc<dyn FormatRules>,
    seed: u64,
    turn: u32,
    active: usize,
    step: Step,
    error: Option<ScenarioError>,
}

impl Scenario {
    /// A scenario with `players` seats at turn 1, player 0's first main phase.
    pub fn new(players: usize) -> Self {
        Scenario {
            players: vec![PlayerSetup::default(); players],
            library_given: vec![false; players],
            format: Arc::new(Normal),
            seed: 0,
            turn: 1,
            active: 0,
            step: Step::Main1,
            error: None,
        }
    }

    /// Sets the game seed.
    #[must_use]
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Sets the format.
    #[must_use]
    pub fn format(mut self, format: Arc<dyn FormatRules>) -> Self {
        self.format = format;
        self
    }

    /// Starts at the beginning of `step` of turn `turn`, with `active` as the active player.
    #[must_use]
    pub fn at(mut self, turn: u32, active: usize, step: Step) -> Self {
        self.turn = turn;
        self.active = active;
        self.step = step;
        self
    }

    fn seat(&mut self, player: usize) -> Option<&mut PlayerSetup> {
        if player >= self.players.len() {
            self.error
                .get_or_insert(ScenarioError::NoSuchPlayer(player));
            return None;
        }
        self.players.get_mut(player)
    }

    /// Sets a player's hand.
    #[must_use]
    pub fn hand(
        mut self,
        player: usize,
        cards: impl IntoIterator<Item = Arc<CardDefinition>>,
    ) -> Self {
        if let Some(seat) = self.seat(player) {
            seat.hand = cards.into_iter().collect();
        }
        self
    }

    /// Sets a player's library, top card first.
    #[must_use]
    pub fn library(
        mut self,
        player: usize,
        cards: impl IntoIterator<Item = Arc<CardDefinition>>,
    ) -> Self {
        if let Some(seat) = self.seat(player) {
            seat.library = cards.into_iter().collect();
            self.library_given[player] = true;
        }
        self
    }

    /// Sets a player's permanents.
    #[must_use]
    pub fn battlefield(
        mut self,
        player: usize,
        permanents: impl IntoIterator<Item = PresetPermanent>,
    ) -> Self {
        if let Some(seat) = self.seat(player) {
            seat.battlefield = permanents.into_iter().collect();
        }
        self
    }

    /// Sets a player's graveyard.
    #[must_use]
    pub fn graveyard(
        mut self,
        player: usize,
        cards: impl IntoIterator<Item = Arc<CardDefinition>>,
    ) -> Self {
        if let Some(seat) = self.seat(player) {
            seat.graveyard = cards.into_iter().collect();
        }
        self
    }

    /// Sets a player's life total.
    #[must_use]
    pub fn life(mut self, player: usize, life: i32) -> Self {
        if let Some(seat) = self.seat(player) {
            seat.life = Some(life);
        }
        self
    }

    /// The setup this scenario describes (decks are not validated).
    pub fn setup(self) -> Result<GameSetup, ScenarioError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let mut players = self.players;
        for (seat, given) in players.iter_mut().zip(&self.library_given) {
            if !given {
                seat.library = vec![forest(); FILLER_LIBRARY];
            }
        }
        Ok(GameSetup {
            format: self.format,
            seed: self.seed,
            players,
            start: Start::At {
                turn: self.turn,
                active: PlayerId::from_index(self.active),
                step: self.step,
            },
            validate_decks: false,
        })
    }

    /// Builds the setup and runs the engine to its first decision.
    pub fn build(self) -> Result<Engine, ScenarioError> {
        Ok(Engine::new(self.setup()?)?)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: scenarios place cards exactly where asked and report misuse instead of silently defaulting.
    use super::*;
    use crate::cards::{bear, elf};
    use mtg_engine::zones::Zone;
    use rstest::rstest;

    #[rstest]
    #[case(2)]
    #[case(4)]
    #[case(8)]
    fn zones_hold_the_requested_cards(#[case] seats: usize) {
        let engine = Scenario::new(seats)
            .hand(0, [bear(), elf()])
            .battlefield(1, [ready(bear()), tapped(elf())])
            .graveyard(0, [elf()])
            .library(1, [bear(), elf(), elf()])
            .build()
            .unwrap();
        let state = engine.state();
        let names = |zone: Zone| -> Vec<String> {
            state
                .zone(zone)
                .iter()
                .filter_map(|id| state.object(*id))
                .map(|o| o.card().name.clone())
                .collect()
        };
        assert_eq!(
            names(Zone::Hand(PlayerId(0))),
            ["Grizzly Bears", "Llanowar Elf"]
        );
        assert_eq!(names(Zone::Graveyard(PlayerId(0))), ["Llanowar Elf"]);
        assert_eq!(names(Zone::Battlefield), ["Grizzly Bears", "Llanowar Elf"]);
        // Library lists are top first; the top is the last element of the zone.
        assert_eq!(
            names(Zone::Library(PlayerId(1))),
            ["Llanowar Elf", "Llanowar Elf", "Grizzly Bears"]
        );
        assert_eq!(names(Zone::Library(PlayerId(0))).len(), FILLER_LIBRARY);
    }

    #[test]
    fn preset_flags_become_permanent_state() {
        let engine = Scenario::new(2)
            .battlefield(0, [tapped(bear()), sick(bear()), damaged(bear(), 1)])
            .build()
            .unwrap();
        let state = engine.state();
        let perms: Vec<_> = state
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| state.object(*id)?.permanent())
            .collect();
        assert!(perms[0].tapped() && !perms[0].summoning_sick());
        assert!(perms[1].summoning_sick() && !perms[1].tapped());
        assert_eq!(perms[2].damage(), 1);
    }

    #[test]
    fn unknown_players_are_reported() {
        let err = Scenario::new(2).hand(5, [bear()]).setup().unwrap_err();
        assert_eq!(err, ScenarioError::NoSuchPlayer(5));
    }

    #[rstest]
    #[case(1, 0)]
    #[case(3, 1)]
    #[case(7, 2)]
    fn scenario_starts_at_the_requested_turn_and_step(#[case] turn: u32, #[case] active: usize) {
        let engine = Scenario::new(3)
            .at(turn, active, Step::Main2)
            .build()
            .unwrap();
        assert_eq!(engine.state().turn().number(), turn);
        assert_eq!(engine.state().turn().active(), PlayerId::from_index(active));
        assert_eq!(engine.state().turn().step(), Step::Main2);
    }
}
