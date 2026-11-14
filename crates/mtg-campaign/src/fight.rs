//! A boss fight as a plan: the heroes' seats (carried life, relics as emblems, the Spirit rule) against the boss seat
//! (scaled life, extra cards, its boss cards, phases and scheme deck) and the boss's minions, seated after it, who fall
//! with it. The plan lays itself over a normal game setup, whose decks the caller resolves (the server from its card
//! database, the simulator from the corpus).

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
    /// Its scheme deck, in the order its schemes are set in motion.
    #[serde(default)]
    pub schemes: Vec<String>,
}

/// A minion's seat in a fight: a lieutenant on the boss's side that falls with the boss.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinionSeat {
    /// The minion's id.
    pub minion: String,
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
    /// The boss's minions, seated after it.
    #[serde(default)]
    pub minions: Vec<MinionSeat>,
    /// The id of the run the fight belongs to, which the table links back to; none in a simulation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

impl FightPlan {
    /// The number of seats.
    pub fn seats(&self) -> usize {
        self.heroes.len() + 1 + self.minions.len()
    }

    /// The boss's seat.
    pub fn boss_seat(&self) -> PlayerId {
        PlayerId::from_index(self.heroes.len())
    }

    /// The seat of the boss's minion at `at`.
    pub fn minion_seat(&self, at: usize) -> PlayerId {
        PlayerId::from_index(self.heroes.len() + 1 + at)
    }

    /// The decks of the boss's side in seat order: the boss's, then each minion's.
    pub fn boss_decks(&self) -> Vec<&[(String, u32)]> {
        std::iter::once(self.boss.deck.as_slice())
            .chain(self.minions.iter().map(|m| m.deck.as_slice()))
            .collect()
    }

    /// `base` with the fight's teams (the party against the boss and its minions) and each seat's life and opening
    /// hand.
    pub fn house_rules(&self, base: &HouseRules) -> HouseRules {
        let mut seats = vec![0; self.heroes.len()];
        seats.extend(std::iter::repeat_n(1, 1 + self.minions.len()));
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
        for (at, minion) in self.minions.iter().enumerate() {
            handicaps.push(Handicap {
                seat: self.minion_seat(at),
                starting_life: Some(minion.life),
                opening_hand_size: Some(minion.hand),
            });
        }
        HouseRules {
            teams: Some(Teams {
                mode: TeamMode::Teams,
                seats,
            }),
            handicaps,
            ..base.clone()
        }
    }

    /// `setup` (one deck for each seat: the heroes', the boss's, then the minions') made into this fight: its house
    /// rules, the relics and boss cards in the command zones, the heroes' Spirit emblem, the boss's phases and scheme
    /// deck, and the minions bound to the boss. The boss's deck is no Commander deck, so the decks are not checked
    /// against the format.
    pub fn apply(&self, content: &Content, setup: GameSetup) -> Result<GameSetup, CampaignError> {
        if setup.players.len() != self.seats() {
            return Err(CampaignError::Refused(format!(
                "a fight of {} seats got {} decks",
                self.seats(),
                setup.players.len()
            )));
        }
        let haunt = content.haunt().map(crate::content::CustomCard::definition);
        let cards = |ids: &[String]| {
            ids.iter()
                .map(|id| content.card(id).map(crate::content::CustomCard::definition))
                .collect::<Result<Vec<_>, _>>()
        };
        let mut emblems = Vec::new();
        for hero in &self.heroes {
            emblems.push(
                hero.relics
                    .iter()
                    .map(|id| content.card(id).map(crate::content::CustomCard::definition))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        emblems.push(cards(&self.boss.rules)?);
        for minion in &self.minions {
            emblems.push(cards(&minion.rules)?);
        }
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
        let boss_side = 1 + self.minions.len();
        let mut spirits = vec![haunt; self.heroes.len()];
        spirits.extend(std::iter::repeat_n(None, boss_side));
        let mut phase_lists = vec![Vec::new(); self.seats()];
        phase_lists[self.heroes.len()] = phases;
        let mut schemes = vec![Vec::new(); self.seats()];
        schemes[self.heroes.len()] = cards(&self.boss.schemes)?;
        let mut leaders = vec![None; self.heroes.len() + 1];
        leaders.extend(std::iter::repeat_n(
            Some(self.boss_seat()),
            self.minions.len(),
        ));
        let house = self.house_rules(&setup.house_rules);
        let mut setup = setup
            .with_house_rules(house)
            .with_emblems(emblems)
            .with_spirits(spirits)
            .with_phases(phase_lists)
            .with_schemes(schemes)
            .with_leaders(leaders);
        setup.validate_decks = false;
        Ok(setup)
    }
}
