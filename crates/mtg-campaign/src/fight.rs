//! A boss fight as a plan: the heroes' seats (carried life, relics as emblems, the Spirit rule) against one boss seat
//! (scaled life, extra cards, its boss card and phases). The plan lays itself over a normal game setup, whose decks the
//! caller resolves (the server from its card database, the simulator from the corpus).

use mtg_core::house::{Handicap, HouseRules};
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::GameSetup;
use mtg_engine::phases::BossPhase;
use serde::{Deserialize, Serialize};

use crate::CampaignError;
use crate::content::{Brain, Content};

/// The opening hand of a seat with no bonus.
pub const HAND: u32 = 7;

/// A hero's seat in a fight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroSeat {
    /// The hero's index in the party.
    pub hero: u8,
    /// The life it starts the fight with.
    pub life: i32,
    /// Its opening hand size.
    pub hand: u32,
    /// The relics in its command zone.
    pub relics: Vec<String>,
}

/// The boss's seat in a fight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BossSeat {
    /// The boss's id.
    pub boss: String,
    /// Its name.
    pub name: String,
    /// The life it starts with.
    pub life: i32,
    /// Its opening hand size.
    pub hand: u32,
    /// How the seat is played.
    pub brain: Brain,
    /// Its deck: card names and copies.
    pub deck: Vec<(String, u32)>,
    /// The custom cards in its command zone from the start.
    pub rules: Vec<String>,
    /// Its phases: the phase card and the life at or below which it begins.
    pub phases: Vec<(String, i32)>,
}

/// One boss fight of a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FightPlan {
    /// The ladder room.
    pub node: u8,
    /// The heroes, in seat order from seat 0.
    pub heroes: Vec<HeroSeat>,
    /// The boss, seated after the heroes.
    pub boss: BossSeat,
}

impl FightPlan {
    /// The number of seats.
    pub fn seats(&self) -> usize {
        self.heroes.len() + 1
    }

    /// The boss's seat.
    pub fn boss_seat(&self) -> PlayerId {
        PlayerId::from_index(self.heroes.len())
    }

    /// `base` with the fight's teams (the party against the boss) and each seat's life and opening hand.
    pub fn house_rules(&self, base: &HouseRules) -> HouseRules {
        let mut seats = vec![0; self.heroes.len()];
        seats.push(1);
        let mut handicaps: Vec<Handicap> = self
            .heroes
            .iter()
            .enumerate()
            .map(|(at, hero)| Handicap {
                seat: PlayerId::from_index(at),
                starting_life: Some(hero.life),
                opening_hand_size: Some(hero.hand),
            })
            .collect();
        handicaps.push(Handicap {
            seat: self.boss_seat(),
            starting_life: Some(self.boss.life),
            opening_hand_size: Some(self.boss.hand),
        });
        HouseRules {
            teams: Some(Teams {
                mode: TeamMode::Teams,
                seats,
            }),
            handicaps,
            ..base.clone()
        }
    }

    /// `setup` (one deck for each seat, the boss last) made into this fight: its house rules, the relics and boss
    /// cards in the command zones, the heroes' Spirit emblem and the boss's phases. The boss's deck is no Commander
    /// deck, so the decks are not checked against the format.
    pub fn apply(&self, content: &Content, setup: GameSetup) -> Result<GameSetup, CampaignError> {
        if setup.players.len() != self.seats() {
            return Err(CampaignError::Refused(format!(
                "a fight of {} seats got {} decks",
                self.seats(),
                setup.players.len()
            )));
        }
        let haunt = content.haunt().map(crate::content::CustomCard::definition);
        let mut emblems = Vec::new();
        for hero in &self.heroes {
            emblems.push(
                hero.relics
                    .iter()
                    .map(|id| content.card(id).map(crate::content::CustomCard::definition))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        emblems.push(
            self.boss
                .rules
                .iter()
                .map(|id| content.card(id).map(crate::content::CustomCard::definition))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let phases = self
            .boss
            .phases
            .iter()
            .map(|(id, at_life)| {
                content.card(id).map(|card| BossPhase {
                    at_life: *at_life,
                    emblem: card.definition(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut spirits = vec![haunt; self.heroes.len()];
        spirits.push(None);
        let mut phase_lists = vec![Vec::new(); self.heroes.len()];
        phase_lists.push(phases);
        let house = self.house_rules(&setup.house_rules);
        let mut setup = setup
            .with_house_rules(house)
            .with_emblems(emblems)
            .with_spirits(spirits)
            .with_phases(phase_lists);
        setup.validate_decks = false;
        Ok(setup)
    }
}
