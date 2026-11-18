//! A boss fight as a plan: the heroes' seats (carried life, relics as emblems, the Spirit rule) against the boss seat
//! (its precon, innate, modifier, relics, phases by round and satchel). The plan lays itself over a normal game setup,
//! whose decks the caller resolves (the server from its card database, the simulator from the corpus).

use mtg_core::house::{Handicap, HouseRules};
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::{GameSetup, PlayerSetup};
use serde::{Deserialize, Serialize};

use crate::CampaignError;
use crate::content::{Brain, Content, CustomCard, SatchelCard};

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
    /// Its commander upgrades, each an emblem in its command zone beside the relics.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upgrades: Vec<crate::upgrade::Upgrade>,
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
    /// Its precon: card names and copies.
    pub deck: Vec<(String, u32)>,
    /// Its innate at its level ([`crate::innate`]), in its command zone from the start.
    #[serde(default)]
    pub innate: String,
    /// The fight's encounter modifier ([`crate::modifiers`]), in its command zone from the start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>,
    /// The relics it brings, copies included ([`crate::boss_relics`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relics: Vec<String>,
    /// Its phases: the phase card and the round whose last turn's end step begins it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub round_phases: Vec<(String, u32)>,
    /// Its satchel, whose cards the caller resolves as the last cards of [`FightPlan::boss_deck`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub satchel: Vec<SatchelCard>,
}

impl BossSeat {
    /// The custom cards in its command zone from the start: its innate, modifier and relics (copies stack).
    pub fn emblems(&self) -> Vec<String> {
        std::iter::once(self.innate.clone())
            .chain(self.modifier.iter().cloned())
            .chain(self.relics.iter().cloned())
            .collect()
    }

    /// Its phases, each beginning by its round.
    fn phases(&self, content: &Content) -> Result<Vec<BossPhase>, CampaignError> {
        self.round_phases
            .iter()
            .map(|(id, round)| {
                Ok(BossPhase {
                    begins: PhaseStart::AtRound(*round),
                    emblem: content.card(id)?.definition(),
                })
            })
            .collect()
    }
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
    /// The id of the run the fight belongs to, which the table links back to; none in a simulation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
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

    /// The boss's deck for the caller to resolve: its precon, then one copy of each satchel card, which
    /// [`FightPlan::apply`] takes out of the library into the satchel again.
    pub fn boss_deck(&self) -> Vec<(String, u32)> {
        let mut deck = self.boss.deck.clone();
        deck.extend(self.boss.satchel.iter().map(|s| (s.card.clone(), 1)));
        deck
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
                mode: TeamMode::SharedTurns,
                seats,
            }),
            handicaps,
            ..base.clone()
        }
    }

    /// `setup` (one deck for each seat: the heroes', then [`FightPlan::boss_deck`]) made into this fight: its house
    /// rules, the relics, commander upgrades and boss cards in the command zones, the heroes' Spirit emblem, the boss's
    /// phases, satchel, sheet and opening-hand rule (7 cards with 2 to 4 lands, no mulligan). The boss's deck is no
    /// Commander deck, so the decks are not checked against the format.
    pub fn apply(&self, content: &Content, setup: GameSetup) -> Result<GameSetup, CampaignError> {
        if setup.players.len() != self.seats() {
            return Err(CampaignError::Refused(format!(
                "a fight of {} seats got {} decks",
                self.seats(),
                setup.players.len()
            )));
        }
        let cards = |ids: &[String]| {
            ids.iter()
                .map(|id| content.card(id).map(CustomCard::definition))
                .collect::<Result<Vec<_>, _>>()
        };
        let mut emblems = Vec::new();
        for hero in &self.heroes {
            let mut own = cards(&hero.relics)?;
            for upgrade in &hero.upgrades {
                own.push(upgrade.definition()?);
            }
            emblems.push(own);
        }
        emblems.push(cards(&self.boss.emblems())?);
        let haunt = content.haunt().map(CustomCard::definition);
        let mut spirits = vec![haunt; self.heroes.len()];
        spirits.push(None);
        let mut phases = vec![Vec::new(); self.seats()];
        phases[self.heroes.len()] = self.boss.phases(content)?;
        let house = self.house_rules(&setup.house_rules);
        let mut setup = setup
            .with_house_rules(house)
            .with_emblems(emblems)
            .with_spirits(spirits)
            .with_phases(phases);
        let boss = setup
            .players
            .get_mut(self.heroes.len())
            .ok_or_else(|| CampaignError::Refused("no boss seat".into()))?;
        boss.opening_lands = Some(mtg_engine::opening_lands::OpeningLands::BOSS);
        take_satchel(boss, &self.boss.satchel);
        boss.boss_sheet = Some(std::sync::Arc::new(self.boss.sheet(content)?));
        setup.validate_decks = false;
        Ok(setup)
    }
}

/// Moves the satchel's cards, which [`FightPlan::boss_deck`] put last in the boss's deck, from its library into its
/// satchel: a reusable card among the reusable ones, a once card among the others. A card the caller could not
/// resolve (a database without it) stays out, as a deck card would.
fn take_satchel(boss: &mut PlayerSetup, satchel: &[SatchelCard]) {
    for spell in satchel.iter().rev() {
        let Some(at) = boss
            .library
            .iter()
            .rposition(|card| card.name == spell.card)
        else {
            continue;
        };
        let card = boss.library.remove(at);
        if spell.reusable {
            boss.satchel_reusable.insert(0, card);
        } else {
            boss.satchel.insert(0, card);
        }
    }
}
