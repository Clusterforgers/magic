//! A boss fight as a plan: the heroes' seats (carried life, relics as emblems, the Spirit rule) against the boss seat
//! (scaled life, extra cards, its boss cards, phases, scheme deck and commander card) and the boss's minions, seated
//! after it, who fall with it. The plan lays itself over a normal game setup, whose decks the caller resolves (the
//! server from its card database, the simulator from the corpus).

use mtg_core::house::{Handicap, HouseRules};
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::GameSetup;
use mtg_engine::phases::{BossPhase, PhaseStart};
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
    /// Its deck: card names and copies.
    pub deck: Vec<(String, u32)>,
    /// Its commander card: a real card that waits in its command zone and is cast free as its last phase begins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commander: Option<String>,
    /// The custom cards in its command zone from the start.
    pub rules: Vec<String>,
    /// Its phases: the phase card and the life at or below which it begins.
    pub phases: Vec<(String, i32)>,
    /// Its scheme deck, in the order its schemes are set in motion.
    #[serde(default)]
    pub schemes: Vec<String>,
    /// Every how many of its turns a scheme is set in motion, from its third.
    #[serde(default = "every_turn")]
    pub scheme_every: u32,
    /// The reworked boss's parts ([`crate::boss_plan`]); none for a boss of an older generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rework: Option<Box<Reworked>>,
}

/// The parts of a reworked boss's seat.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reworked {
    /// Its innate at its level ([`crate::innate`]), in its command zone from the start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub innate: Option<String>,
    /// The fight's encounter modifier ([`crate::modifiers`]), in its command zone from the start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>,
    /// The relics it brings, copies included ([`crate::boss_relics`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relics: Vec<String>,
    /// Its phases: the phase card and the round whose last turn's end step begins it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub round_phases: Vec<(String, u32)>,
    /// Its satchel, whose cards the caller resolves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub satchel: Vec<crate::content::SatchelCard>,
}

fn every_turn() -> u32 {
    1
}

impl BossSeat {
    /// A reworked boss's phases, each beginning by its round.
    fn round_phases<'a>(
        &'a self,
        content: &'a Content,
    ) -> impl Iterator<Item = Result<BossPhase, CampaignError>> + 'a {
        let phases = self.rework.iter().flat_map(|r| r.round_phases.iter());
        phases.map(|(id, round)| {
            Ok(BossPhase {
                begins: PhaseStart::AtRound(*round),
                emblem: content.card(id)?.definition(),
            })
        })
    }

    /// The custom cards in its command zone from the start: its rules, then a reworked boss's innate, modifier and
    /// relics (copies stack).
    pub fn emblems(&self) -> Vec<String> {
        let mut ids = self.rules.clone();
        if let Some(rework) = &self.rework {
            ids.extend(rework.innate.iter().cloned());
            ids.extend(rework.modifier.iter().cloned());
            ids.extend(rework.relics.iter().cloned());
        }
        ids
    }
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

    /// The decks of the boss's side in seat order: the boss's with its commander card last, then each minion's. The
    /// caller resolves them; [`FightPlan::apply`] takes the commander card out of the boss's library again.
    pub fn boss_decks(&self) -> Vec<Vec<(String, u32)>> {
        let mut boss = self.boss.deck.clone();
        boss.extend(self.boss.commander.iter().map(|name| (name.clone(), 1)));
        let satchel = self.boss.rework.iter().flat_map(|r| r.satchel.iter());
        boss.extend(satchel.map(|spell| (spell.card.clone(), 1)));
        std::iter::once(boss)
            .chain(self.minions.iter().map(|m| m.deck.clone()))
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
                mode: TeamMode::SharedTurns,
                seats,
            }),
            handicaps,
            ..base.clone()
        }
    }

    /// `setup` (one deck for each seat: the heroes', the boss's, then the minions') made into this fight: its house
    /// rules, the relics, commander upgrades and boss cards in the command zones, the heroes' Spirit emblem, the boss's phases and scheme
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
            let mut own = hero
                .relics
                .iter()
                .map(|id| content.card(id).map(crate::content::CustomCard::definition))
                .collect::<Result<Vec<_>, _>>()?;
            for upgrade in &hero.upgrades {
                own.push(upgrade.definition()?);
            }
            emblems.push(own);
        }
        emblems.push(cards(&self.boss.emblems())?);
        for minion in &self.minions {
            emblems.push(cards(&minion.rules)?);
        }
        // The commander card waits in the command zone; the boss's last own phase casts it.
        let last_own = self
            .boss
            .phases
            .iter()
            .rposition(|(id, _)| id != crate::scale::BULWARK);
        let phases = self
            .boss
            .phases
            .iter()
            .enumerate()
            .map(|(at, (id, at_life))| {
                let card = content.card(id)?;
                let emblem = match &self.boss.commander {
                    Some(name) if Some(at) == last_own => {
                        let mut card = card.clone();
                        card.abilities.push(cast_commander(name)?);
                        card.definition()
                    }
                    _ => card.definition(),
                };
                Ok(BossPhase {
                    begins: PhaseStart::AtLife(*at_life),
                    emblem,
                })
            })
            .chain(self.boss.round_phases(content))
            .collect::<Result<Vec<_>, CampaignError>>()?;
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
            .with_scheme_every(self.boss.scheme_every)
            .with_leaders(leaders);
        if let Some(name) = &self.boss.commander {
            let boss = setup
                .players
                .get_mut(self.heroes.len())
                .ok_or_else(|| CampaignError::Refused("no boss seat".into()))?;
            let at = boss
                .library
                .iter()
                .rposition(|card| card.name == *name)
                .ok_or_else(|| CampaignError::Refused(format!("the boss deck lacks {name}")))?;
            let card = boss.library.remove(at);
            boss.held.push(card);
        }
        // A reworked boss draws 7 with 2 to 4 lands and takes no mulligan.
        if self.boss.rework.is_some()
            && let Some(boss) = setup.players.get_mut(self.heroes.len())
        {
            boss.opening_lands = Some(mtg_engine::opening_lands::OpeningLands::BOSS);
            if let Some(rework) = &self.boss.rework {
                take_satchel(boss, &rework.satchel)?;
                boss.boss_sheet = Some(std::sync::Arc::new(rework.sheet(content)?));
            }
        }
        setup.validate_decks = false;
        Ok(setup)
    }
}

/// Moves the satchel's cards, which [`FightPlan::boss_decks`] put last in the boss's deck, from its library into its
/// satchel: a reusable card among the reusable ones, a once card among the others.
fn take_satchel(
    boss: &mut mtg_engine::PlayerSetup,
    satchel: &[crate::content::SatchelCard],
) -> Result<(), CampaignError> {
    for spell in satchel.iter().rev() {
        let at = boss
            .library
            .iter()
            .rposition(|card| card.name == spell.card)
            .ok_or_else(|| CampaignError::Refused(format!("the boss deck lacks {}", spell.card)))?;
        let card = boss.library.remove(at);
        if spell.reusable {
            boss.satchel_reusable.insert(0, card);
        } else {
            boss.satchel.insert(0, card);
        }
    }
    Ok(())
}

/// The ability the boss's last phase gets for its commander card `name`: while the card waits in the boss's command
/// zone, the boss casts it without paying its mana cost; a card that can't be cast then goes to its graveyard.
fn cast_commander(name: &str) -> Result<mtg_cards::ability::Ability, CampaignError> {
    let card = serde_json::json!({ "Each": [{ "In": "Command" }, { "Named": name }, { "OwnedBy": "You" }] });
    let ability = serde_json::json!({ "Triggered": {
        "when": { "State": { "condition": { "Compare": {
            "left": { "Count": card["Each"] }, "op": "Gt", "right": 0
        } } } },
        "effects": [
            { "Bind": { "what": card, "as": "commander" } },
            { "CastFromZone": {
                "what": { "Bound": "commander" },
                "free": true,
                "must": true,
                "otherwise": [{ "Move": { "what": { "Bound": "commander" }, "to": "Graveyard" } }]
            } }
        ]
    } });
    serde_json::from_value(ability).map_err(|e| CampaignError::Content(e.to_string()))
}
