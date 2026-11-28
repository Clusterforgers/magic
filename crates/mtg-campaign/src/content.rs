//! The campaign's content: custom cards (relics, boss cards and phases, the Spirit's haunt) and the themes with their
//! bosses, events and reward pool, read from the JSON files of `content/`.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use serde::{Deserialize, Serialize};

use crate::CampaignError;
use crate::relics::Want;

/// What a custom card is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum CardKind {
    /// A boon a hero keeps for the run, in their command zone at every fight.
    Relic,
    /// A boss phase, which begins by round.
    Phase,
    /// The emblem a hero gets on becoming a Spirit.
    Haunt,
    /// A boss's innate, its one fixed effect, written once with a number that grows over five levels
    /// ([`crate::innate`]).
    Innate,
    /// An encounter modifier rolled for a fight from the shared pool ([`crate::modifiers`]).
    Encounter,
    /// What a boss brings for each hero after the first ([`crate::scale::Scale::reinforcements`]).
    Reinforcement,
}

/// How rare a relic is: stronger effects sit in rarer tiers, and the tier decides where the relic comes from
/// (`docs/delves.md`, Relics).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Rarity {
    /// Loot, treasure and the shop offer it most.
    #[default]
    Common,
    /// Loot and treasure offer it now and then, more on later floors; the shop sells it dear.
    Rare,
    /// Run-defining: only events and the loot before the final boss offer it.
    Mythic,
}

impl Rarity {
    /// The tier below, `None` below Common.
    pub fn lower(self) -> Option<Rarity> {
        match self {
            Rarity::Mythic => Some(Rarity::Rare),
            Rarity::Rare => Some(Rarity::Common),
            Rarity::Common => None,
        }
    }

    /// Its word: "Common", "Rare", "Mythic".
    pub fn word(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Rare => "Rare",
            Rarity::Mythic => "Mythic",
        }
    }
}

/// A card that is not on Scryfall: its printed side and the abilities it has as an emblem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomCard {
    /// Its id in the content.
    pub id: String,
    /// Its name.
    pub name: String,
    /// What it is for.
    pub kind: CardKind,
    /// Its type line.
    pub type_line: String,
    /// Its rules text.
    pub text: String,
    /// The key of its art placeholder.
    #[serde(default)]
    pub art: String,
    /// The team role a relic favours: Healer, Removal, Damage, Support, or Risky for one that helps the boss too.
    #[serde(default)]
    pub role: String,
    /// A relic's price at the shop; 0 for one the shop never sells.
    #[serde(default)]
    pub price: u32,
    /// Life its holder starts each fight with in addition.
    #[serde(default)]
    pub life: i32,
    /// Cards its holder's opening hand has in addition.
    #[serde(default)]
    pub hand: u32,
    /// What a relic's rules care about in a deck, so offers skip it for a deck with too few such cards; none for a relic
    /// of use to any deck.
    #[serde(default)]
    pub wants: Vec<Want>,
    /// The content generation from which a relic is offered (`RunEvent::Content`); a run of an earlier one never meets it.
    #[serde(default)]
    pub since: u32,
    /// The last content generation that offers a relic; a later run meets the relic that replaced it instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<u32>,
    /// A relic's rarity, which from the rarity generation on decides where it is offered and its shop price.
    #[serde(default)]
    pub rarity: Rarity,
    /// A relic that names the holder's teammates (a Spirit on the team, say): a boss never brings it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub team: bool,
    /// The level of an innate made on loading, 1 to [`crate::innate::LEVELS`]; 0 for every other card.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub level: u8,
    /// Its abilities, in the card DSL.
    pub abilities: Vec<Ability>,
}

impl CustomCard {
    /// Whether a run of content generation `version` may meet it.
    pub fn in_generation(&self, version: u32) -> bool {
        self.since <= version && self.until.is_none_or(|last| version <= last)
    }

    /// The card as an emblem the engine plays.
    pub fn definition(&self) -> Arc<CardDefinition> {
        Arc::new(mtg_engine::w6b_emblems::emblem_definition(
            &self.name,
            &self.abilities,
        ))
    }
}

/// How a boss seat is played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Brain {
    /// Random legal answers.
    Easy,
    /// The heuristic bot.
    #[default]
    Normal,
    /// The bot with a deeper search.
    Hard,
    /// The strongest bot.
    VeryHard,
}

/// A boss: its precon deck, its innate, two phases by round and its satchel (`docs/delves.md`, Bosses).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boss {
    /// Its id in the content.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Whether it is one of the theme's final bosses, one of which a ladder meets last.
    #[serde(default, rename = "final")]
    pub final_boss: bool,
    /// Its colours, as WUBRG letters: its deck's.
    pub colors: String,
    /// The content generation it joined the theme in: a run made before it never meets it.
    #[serde(default)]
    pub since: u32,
    /// Its deck file (`content/decks/<code>-<name>.json`), a stored Commander precon.
    pub deck_file: String,
    /// Its innate card, written with five levels ([`crate::innate`]).
    pub innate: String,
    /// Its Phase 1 and Phase 2 cards, which begin by round ([`crate::scale::PHASE_ROUNDS`]).
    pub round_phases: Vec<String>,
    /// Its satchel: four spells in its command zone, cast like cards in hand.
    pub satchel: Vec<SatchelCard>,
    /// What it says as the fight begins.
    #[serde(default)]
    pub line: String,
    /// The real card whose art pictures it.
    pub art: BossArt,
    /// How it weighs the heroes' threat when it eyes one, from [`crate::run::PERSONALITY_FROM`].
    pub personality: mtg_core::personality::Personality,
}

/// A spell of a boss's satchel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SatchelCard {
    /// The real card's name.
    pub card: String,
    /// Whether it returns to the satchel as it leaves the stack (each cast costs {2} more); otherwise once per fight.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reusable: bool,
    /// When the boss's bot casts it; none leaves it to the bot's own judgement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cue: Option<mtg_core::cue::SatchelCue>,
}

/// The real card whose art crop pictures a boss: its name and its Scryfall oracle id, which the client's pictures go by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BossArt {
    /// The card's name.
    pub card: String,
    /// The card's Scryfall oracle id.
    pub oracle_id: String,
}

/// What an event option does to the hero who takes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Outcome {
    /// Gold gained (or lost, never below 0).
    Gold(i32),
    /// Life gained (up to the hero's maximum) or lost (never below 1).
    Life(i32),
    /// A relic drawn from those the hero lacks.
    Relic,
    /// A card pick in the hero's colours, for their Satchel.
    CardPick,
    /// A copy of a nonland card from the hero's deck or Satchel, of their choice, for their Satchel.
    Duplicate,
    /// A pick of relics of this rarity the hero lacks (of a lower one when the deck can use none of it).
    RelicOf(Rarity),
    /// The hero's life cap moves by this much for the rest of the run; a cut also lowers their life to the new cap.
    MaxLife(i32),
    /// The hero gives up a card of their choice from their deck or Satchel for the rest of the run.
    LoseCard,
    /// The hero gives up a relic of their choice.
    LoseRelic,
    /// The hero's first commander is weakened for the rest of the run, as an emblem like an upgrade.
    Bane(crate::upgrade::Bane),
    /// A chance: `win` with `odds` percent, `lose` otherwise. Only events before the trade generation have one.
    Gamble {
        /// The chance to win, in percent.
        odds: u32,
        /// What a win gives.
        win: Vec<Outcome>,
        /// What a loss gives.
        lose: Vec<Outcome>,
    },
}

/// One choice of an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventOption {
    /// The choice in a few words.
    pub label: String,
    /// What it does.
    pub outcomes: Vec<Outcome>,
}

/// What a hero must have to take an option: its sure costs, never its gambles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Needs {
    /// The gold a sure cost takes.
    pub gold: u32,
    /// The life a sure cost takes; the hero must keep more than 0 of it.
    pub life: u32,
    /// The life cap a cut takes; the hero must keep at least `MIN_LIFE_CAP` of it.
    #[serde(default)]
    pub max_life: u32,
    /// Whether the hero must hold a relic to give up.
    #[serde(default)]
    pub relic: bool,
    /// Whether the hero's deck must have a commander to weaken.
    #[serde(default)]
    pub commander: bool,
    /// Whether that commander must be a creature.
    #[serde(default)]
    pub creature_commander: bool,
}

/// The least life cap a cut may leave a hero, so an event never leaves them unable to go on.
pub const MIN_LIFE_CAP: i32 = 20;

impl Needs {
    /// Why a hero with this gold and life cannot take the option, if they cannot.
    pub fn unmet(&self, gold: u32, life: i32) -> Option<String> {
        if gold < self.gold {
            return Some(format!("needs {} gold, you have {gold}", self.gold));
        }
        if self.life > 0 && i64::from(life) <= i64::from(self.life) {
            return Some(format!(
                "needs more than {} life, you have {life}",
                self.life
            ));
        }
        None
    }
}

impl Outcome {
    /// The outcome in plain words: "+40 gold", "−8 life", "a relic", "a card for your Satchel", or a chance with its odds.
    pub fn words(&self) -> String {
        match self {
            Outcome::Gold(n) => format!("{} gold", signed(*n)),
            Outcome::Life(n) => format!("{} life", signed(*n)),
            Outcome::Relic => "a relic".to_owned(),
            Outcome::CardPick => "a card for your Satchel".to_owned(),
            Outcome::Duplicate => "a copy of a card from your deck or Satchel".to_owned(),
            Outcome::RelicOf(rarity) => format!("a {} relic", rarity.word()),
            Outcome::MaxLife(n) => format!("{} max life", signed(*n)),
            Outcome::LoseCard => "lose a card of your deck or Satchel".to_owned(),
            Outcome::LoseRelic => "give up a relic".to_owned(),
            Outcome::Bane(bane) => bane.words("your commander"),
            Outcome::Gamble { odds, win, lose } => format!(
                "{odds}%: {} · {}%: {}",
                list_words(win),
                100u32.saturating_sub(*odds),
                list_words(lose)
            ),
        }
    }

    /// Whether it takes something from the hero: gold, life or life cap lost, a card, a relic, a commander bane.
    pub fn is_cost(&self) -> bool {
        match self {
            Outcome::Gold(n) | Outcome::Life(n) | Outcome::MaxLife(n) => *n < 0,
            Outcome::LoseCard | Outcome::LoseRelic | Outcome::Bane(_) => true,
            Outcome::Relic
            | Outcome::CardPick
            | Outcome::Duplicate
            | Outcome::RelicOf(_)
            | Outcome::Gamble { .. } => false,
        }
    }
}

/// A number with its sign, a minus as "−": "+40", "−8", "0".
fn signed(n: i32) -> String {
    match n {
        0 => "0".to_owned(),
        n if n > 0 => format!("+{n}"),
        n => format!("−{}", n.unsigned_abs()),
    }
}

/// Outcomes in plain words, "nothing" for none.
pub fn list_words(outcomes: &[Outcome]) -> String {
    if outcomes.is_empty() {
        return "nothing".to_owned();
    }
    outcomes
        .iter()
        .map(Outcome::words)
        .collect::<Vec<_>>()
        .join(", ")
}

impl EventOption {
    /// What the option does, from its outcomes: "−8 life, +40 gold", "nothing" for an option with none.
    pub fn summary(&self) -> String {
        list_words(&self.outcomes)
    }

    /// What it takes from the hero, in plain words ("−8 life, give up a relic"), "nothing" for a free option.
    pub fn cost(&self) -> String {
        let costs: Vec<Outcome> = self
            .outcomes
            .iter()
            .filter(|o| o.is_cost())
            .cloned()
            .collect();
        list_words(&costs)
    }

    /// What it gives the hero, in plain words ("a Mythic relic"), "nothing" for an option that only costs.
    pub fn reward(&self) -> String {
        let gains: Vec<Outcome> = self
            .outcomes
            .iter()
            .filter(|o| !o.is_cost())
            .cloned()
            .collect();
        list_words(&gains)
    }

    /// What a hero must have to take it: gold, life and life cap for its sure costs, a relic to give up, a commander to
    /// weaken.
    pub fn needs(&self) -> Needs {
        let cost = |pick: fn(&Outcome) -> Option<i32>| {
            let sum: i64 = self
                .outcomes
                .iter()
                .filter_map(pick)
                .filter(|n| *n < 0)
                .map(i64::from)
                .sum();
            u32::try_from((-sum).max(0)).unwrap_or(u32::MAX)
        };
        Needs {
            gold: cost(|o| {
                if let Outcome::Gold(n) = o {
                    Some(*n)
                } else {
                    None
                }
            }),
            life: cost(|o| {
                if let Outcome::Life(n) = o {
                    Some(*n)
                } else {
                    None
                }
            }),
            max_life: cost(|o| {
                if let Outcome::MaxLife(n) = o {
                    Some(*n)
                } else {
                    None
                }
            }),
            relic: self.outcomes.contains(&Outcome::LoseRelic),
            commander: self.outcomes.iter().any(|o| matches!(o, Outcome::Bane(_))),
            creature_commander: self
                .outcomes
                .iter()
                .any(|o| matches!(o, Outcome::Bane(bane) if bane.for_creature())),
        }
    }
}

/// A story room: a short text and the choices each hero makes for themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSpec {
    /// Its id in the content.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its story text.
    pub text: String,
    /// Its choices.
    pub options: Vec<EventOption>,
    /// The content generation it joined the theme in: a run made before it never meets it (`Run.content`).
    #[serde(default)]
    pub since: u32,
    /// The last content generation that meets it; a later run meets the trade version that replaced it instead.
    #[serde(default)]
    pub until: Option<u32>,
}

impl EventSpec {
    /// Whether a run of content generation `version` may meet it.
    pub fn in_generation(&self, version: u32) -> bool {
        self.since <= version && self.until.is_none_or(|last| version <= last)
    }
}

/// The content generation the Ember Foundry joins in; a run of an earlier one never meets it.
pub const FOUNDRY_FROM: u32 = 11;

/// The least Heat of a run that meets the Ember Foundry.
pub const FOUNDRY_HEAT: u8 = 3;

/// The content generation the Gloamwood joins in; a run of an earlier one never meets it.
pub const GLOAMWOOD_FROM: u32 = 18;

/// A dungeon theme: its bosses, events and the cards its rewards offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// Its id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its story text.
    pub text: String,
    /// Its bosses: the lieutenants in any order, then the final boss.
    pub bosses: Vec<Boss>,
    /// Its events.
    pub events: Vec<EventSpec>,
    /// The cards rewards offer: a name and its colours as WUBRG letters (empty for colourless).
    pub pool: Vec<(String, String)>,
    /// The type line of each pool card ("Creature — Angel"), which counts the Satchel's cards for the relic filter.
    #[serde(default)]
    pub pool_types: BTreeMap<String, String>,
    /// The least Heat of a run that may draw it; 0 for every Heat.
    #[serde(default)]
    pub min_heat: u8,
    /// The [`SUPPLIES`](crate::scale::SUPPLIES) each of its bosses brings against two or more heroes, from generation
    /// [`THEME_SUPPLIES_FROM`](crate::run::THEME_SUPPLIES_FROM).
    #[serde(default)]
    pub boss_supplies: usize,
    /// The most relics each of its bosses brings from generation
    /// [`THEME_RELICS_FROM`](crate::run::THEME_RELICS_FROM); none for [`MOST_BOSS_RELICS`](crate::boss_relics::MOST_BOSS_RELICS).
    #[serde(default)]
    pub boss_relics_most: Option<usize>,
    /// The most [`REINFORCEMENT`](crate::scale::REINFORCEMENT)s each of its bosses brings from generation
    /// [`THEME_REINFORCEMENTS_FROM`](crate::run::THEME_REINFORCEMENTS_FROM), each further one coming as
    /// [`SUPPLIES`](crate::scale::SUPPLIES); none for one reinforcement for each hero after the first.
    #[serde(default)]
    pub boss_reinforcements_most: Option<usize>,
}

impl Theme {
    /// The boss with this id.
    pub fn boss(&self, id: &str) -> Option<&Boss> {
        self.bosses.iter().find(|m| m.id == id)
    }

    /// The event with this id.
    pub fn event(&self, id: &str) -> Option<&EventSpec> {
        self.events.iter().find(|e| e.id == id)
    }
}

/// Everything the campaign is made of.
#[derive(Debug, Clone, PartialEq)]
pub struct Content {
    /// The custom cards by id.
    pub cards: BTreeMap<String, CustomCard>,
    /// The themes.
    pub themes: Vec<Theme>,
    /// The encounter modifiers both themes roll from ([`crate::modifiers`]).
    pub modifiers: Vec<crate::modifiers::EncounterModifier>,
    /// The bosses' deck files by file name (`content/decks/`).
    pub decks: BTreeMap<String, DeckFile>,
}

/// A boss's deck file: a stored Commander precon as the boss plays it, its face commander shuffled in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckFile {
    /// The precon's set code.
    pub code: String,
    /// The precon's name.
    pub name: String,
    /// Its face commanders, which play as ordinary cards of the library.
    pub commander: Vec<String>,
    /// The cards played by hand that the file swaps for basic lands or automatic cards of their type.
    #[serde(default)]
    pub swaps: Vec<DeckSwap>,
    /// Its cards and copies, the swaps made: 100 cards.
    pub cards: Vec<(String, u32)>,
    /// What the deck is made of, which the boss's relics follow (counted by the `boss_decks` test).
    #[serde(default)]
    pub makeup: crate::relics::DeckMakeup,
}

/// A card of a precon played by hand, swapped for a basic land or an automatic card of its type and colours.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckSwap {
    /// The card taken out.
    pub out: String,
    /// The card put in.
    pub into: String,
}

impl Content {
    /// The cue of every boss Satchel card that has one, by card name; the first boss's wins when two differ.
    pub fn satchel_cues(&self) -> BTreeMap<String, mtg_core::cue::SatchelCue> {
        let mut cues = BTreeMap::new();
        for card in self
            .themes
            .iter()
            .flat_map(|theme| &theme.bosses)
            .flat_map(|boss| &boss.satchel)
        {
            if let Some(cue) = card.cue {
                cues.entry(card.card.clone()).or_insert(cue);
            }
        }
        cues
    }

    /// Reads the content from its JSON texts: the custom card files (innates made at each level), the themes and the
    /// encounter modifier pool.
    pub fn parse(
        card_files: &[&str],
        themes: &[&str],
        modifiers: &str,
    ) -> Result<Content, CampaignError> {
        let fail = |e: serde_json::Error| CampaignError::Content(e.to_string());
        let mut list: Vec<CustomCard> = Vec::new();
        for text in card_files {
            let entries: Vec<serde_json::Value> = serde_json::from_str(text).map_err(fail)?;
            for entry in crate::innate::expand(entries)? {
                list.push(serde_json::from_value(entry).map_err(fail)?);
            }
        }
        let modifiers = serde_json::from_str(modifiers).map_err(fail)?;
        let cards: BTreeMap<String, CustomCard> =
            list.into_iter().map(|c| (c.id.clone(), c)).collect();
        let themes = themes
            .iter()
            .map(|text| {
                serde_json::from_str(text).map_err(|e| CampaignError::Content(e.to_string()))
            })
            .collect::<Result<Vec<Theme>, _>>()?;
        Ok(Content {
            cards,
            themes,
            modifiers,
            decks: BTreeMap::new(),
        })
    }

    /// This content with the deck files of `files` (file name, JSON text).
    pub fn with_decks(mut self, files: &[(&str, &str)]) -> Result<Content, CampaignError> {
        for (file, text) in files {
            let deck: DeckFile = serde_json::from_str(text)
                .map_err(|e| CampaignError::Content(format!("{file}: {e}")))?;
            self.decks.insert((*file).to_owned(), deck);
        }
        Ok(self)
    }

    /// The deck file with this name.
    pub fn deck(&self, file: &str) -> Result<&DeckFile, CampaignError> {
        self.decks
            .get(file)
            .ok_or_else(|| CampaignError::Unknown(format!("deck file {file}")))
    }

    /// The custom card with this id.
    pub fn card(&self, id: &str) -> Result<&CustomCard, CampaignError> {
        self.cards
            .get(id)
            .ok_or_else(|| CampaignError::Unknown(format!("card {id}")))
    }

    /// The theme with this id.
    pub fn theme(&self, id: &str) -> Result<&Theme, CampaignError> {
        self.themes
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| CampaignError::Unknown(format!("theme {id}")))
    }

    /// The themes a run of content generation `generation` at `heat` may play, in content order: those whose final boss
    /// that generation meets and whose least Heat it reaches (only the Warrens before the Drowned Spire's generation; the
    /// Ember Foundry from Heat 3).
    pub fn open_themes(&self, generation: u32, heat: u8) -> Vec<&Theme> {
        self.themes
            .iter()
            .filter(|t| {
                t.min_heat <= heat
                    && t.bosses
                        .iter()
                        .any(|b| b.final_boss && b.since <= generation)
            })
            .collect()
    }

    /// The theme of `floor` (from 2) after a floor of theme `from`: drawn by the seed keyed by the floor among the themes
    /// open at `generation` and `heat` other than `from`, or `from` again while no other is open.
    pub fn next_theme(
        &self,
        seed: u64,
        generation: u32,
        heat: u8,
        floor: u8,
        from: &str,
    ) -> Result<&Theme, CampaignError> {
        let others: Vec<&Theme> = self
            .open_themes(generation, heat)
            .into_iter()
            .filter(|t| t.id != from)
            .collect();
        if others.is_empty() {
            return self.theme(from);
        }
        let at = crate::rng::Stream::new(seed, "theme", u64::from(floor), 0).below(others.len());
        others
            .get(at)
            .copied()
            .ok_or_else(|| CampaignError::Unknown(format!("a theme of generation {generation}")))
    }

    /// The theme a new run of content generation `generation` plays at Heat 0, drawn by its seed ([`Content::theme_at`]).
    pub fn theme_for(&self, seed: u64, generation: u32) -> Result<&Theme, CampaignError> {
        self.theme_at(seed, generation, 0)
    }

    /// The theme a new run of content generation `generation` at `heat` plays, drawn by its seed among the open themes.
    pub fn theme_at(&self, seed: u64, generation: u32, heat: u8) -> Result<&Theme, CampaignError> {
        let open = self.open_themes(generation, heat);
        let at = crate::rng::Stream::new(seed, "theme", 0, 0).below(open.len());
        open.get(at)
            .copied()
            .ok_or_else(|| CampaignError::Unknown(format!("a theme of generation {generation}")))
    }

    /// The relics, in id order.
    pub fn relics(&self) -> Vec<&CustomCard> {
        self.cards
            .values()
            .filter(|c| c.kind == CardKind::Relic)
            .collect()
    }

    /// The emblem a hero gets on becoming a Spirit.
    pub fn haunt(&self) -> Option<&CustomCard> {
        self.cards.values().find(|c| c.kind == CardKind::Haunt)
    }
}

/// Serde passes the field by reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// The content shipped with the crate, read once.
pub fn content() -> Result<&'static Content, CampaignError> {
    static CONTENT: OnceLock<Result<Content, CampaignError>> = OnceLock::new();
    CONTENT
        .get_or_init(|| {
            Content::parse(
                &[
                    include_str!("../content/cards.json"),
                    include_str!("../content/encounters.json"),
                    include_str!("../content/rework_warrens_a.json"),
                    include_str!("../content/rework_spire.json"),
                    include_str!("../content/rework_warrens_b.json"),
                    include_str!("../content/rework_foundry.json"),
                    include_str!("../content/rework_gloamwood.json"),
                ],
                &[
                    include_str!("../content/warrens.json"),
                    include_str!("../content/spire.json"),
                    include_str!("../content/foundry.json"),
                    include_str!("../content/gloamwood.json"),
                ],
                include_str!("../content/modifiers.json"),
            )
            .and_then(|content| content.with_decks(DECK_FILES))
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// The bosses' deck files shipped with the crate.
const DECK_FILES: &[(&str, &str)] = &[
    (
        "40k-necron-dynasties.json",
        include_str!("../content/decks/40k-necron-dynasties.json"),
    ),
    (
        "afc-draconic-rage.json",
        include_str!("../content/decks/afc-draconic-rage.json"),
    ),
    (
        "afc-planar-portal-foundry.json",
        include_str!("../content/decks/afc-planar-portal-foundry.json"),
    ),
    (
        "afc-planar-portal.json",
        include_str!("../content/decks/afc-planar-portal.json"),
    ),
    (
        "blc-squirreled-away.json",
        include_str!("../content/decks/blc-squirreled-away.json"),
    ),
    (
        "brc-mishra-s-burnished-banner.json",
        include_str!("../content/decks/brc-mishra-s-burnished-banner.json"),
    ),
    (
        "c14-built-from-scratch-foundry.json",
        include_str!("../content/decks/c14-built-from-scratch-foundry.json"),
    ),
    (
        "c14-built-from-scratch.json",
        include_str!("../content/decks/c14-built-from-scratch.json"),
    ),
    (
        "c14-forged-in-stone-foundry.json",
        include_str!("../content/decks/c14-forged-in-stone-foundry.json"),
    ),
    (
        "c14-forged-in-stone.json",
        include_str!("../content/decks/c14-forged-in-stone.json"),
    ),
    (
        "c14-guided-by-nature-foundry.json",
        include_str!("../content/decks/c14-guided-by-nature-foundry.json"),
    ),
    (
        "c14-guided-by-nature.json",
        include_str!("../content/decks/c14-guided-by-nature.json"),
    ),
    (
        "c14-peer-through-time-foundry.json",
        include_str!("../content/decks/c14-peer-through-time-foundry.json"),
    ),
    (
        "c14-peer-through-time.json",
        include_str!("../content/decks/c14-peer-through-time.json"),
    ),
    (
        "c14-sworn-to-darkness.json",
        include_str!("../content/decks/c14-sworn-to-darkness.json"),
    ),
    (
        "c15-plunder-the-graves.json",
        include_str!("../content/decks/c15-plunder-the-graves.json"),
    ),
    (
        "c15-seize-control.json",
        include_str!("../content/decks/c15-seize-control.json"),
    ),
    (
        "c15-swell-the-host.json",
        include_str!("../content/decks/c15-swell-the-host.json"),
    ),
    (
        "c17-feline-ferocity.json",
        include_str!("../content/decks/c17-feline-ferocity.json"),
    ),
    (
        "c18-exquisite-invention.json",
        include_str!("../content/decks/c18-exquisite-invention.json"),
    ),
    (
        "c21-lorehold-legacies.json",
        include_str!("../content/decks/c21-lorehold-legacies.json"),
    ),
    (
        "c21-prismari-performance.json",
        include_str!("../content/decks/c21-prismari-performance.json"),
    ),
    (
        "c21-quantum-quandrix.json",
        include_str!("../content/decks/c21-quantum-quandrix.json"),
    ),
    (
        "c21-silverquill-statement-foundry.json",
        include_str!("../content/decks/c21-silverquill-statement-foundry.json"),
    ),
    (
        "c21-silverquill-statement.json",
        include_str!("../content/decks/c21-silverquill-statement.json"),
    ),
    (
        "cmm-eldrazi-unbound.json",
        include_str!("../content/decks/cmm-eldrazi-unbound.json"),
    ),
    (
        "cmr-reap-the-tides.json",
        include_str!("../content/decks/cmr-reap-the-tides.json"),
    ),
    (
        "dsc-death-toll-foundry.json",
        include_str!("../content/decks/dsc-death-toll-foundry.json"),
    ),
    (
        "dsc-death-toll.json",
        include_str!("../content/decks/dsc-death-toll.json"),
    ),
    (
        "dsc-jump-scare.json",
        include_str!("../content/decks/dsc-jump-scare.json"),
    ),
    (
        "fdc-calling-all-angels.json",
        include_str!("../content/decks/fdc-calling-all-angels.json"),
    ),
    (
        "fdc-tramplesaurus-rex.json",
        include_str!("../content/decks/fdc-tramplesaurus-rex.json"),
    ),
    (
        "fdc-wretched-ranks.json",
        include_str!("../content/decks/fdc-wretched-ranks.json"),
    ),
    (
        "khc-phantom-premonition-foundry.json",
        include_str!("../content/decks/khc-phantom-premonition-foundry.json"),
    ),
    (
        "khc-phantom-premonition.json",
        include_str!("../content/decks/khc-phantom-premonition.json"),
    ),
    (
        "lcc-ahoy-mateys.json",
        include_str!("../content/decks/lcc-ahoy-mateys.json"),
    ),
    (
        "lcc-blood-rites.json",
        include_str!("../content/decks/lcc-blood-rites.json"),
    ),
    (
        "lcc-explorers-of-the-deep.json",
        include_str!("../content/decks/lcc-explorers-of-the-deep.json"),
    ),
    (
        "lcc-veloci-ramp-tor.json",
        include_str!("../content/decks/lcc-veloci-ramp-tor.json"),
    ),
    (
        "mic-coven-counters-foundry.json",
        include_str!("../content/decks/mic-coven-counters-foundry.json"),
    ),
    (
        "mic-coven-counters.json",
        include_str!("../content/decks/mic-coven-counters.json"),
    ),
    (
        "mic-undead-unleashed.json",
        include_str!("../content/decks/mic-undead-unleashed.json"),
    ),
    (
        "mkc-blame-game.json",
        include_str!("../content/decks/mkc-blame-game.json"),
    ),
    (
        "mkc-revenant-recon.json",
        include_str!("../content/decks/mkc-revenant-recon.json"),
    ),
    (
        "nec-upgrades-unleashed-foundry.json",
        include_str!("../content/decks/nec-upgrades-unleashed-foundry.json"),
    ),
    (
        "nec-upgrades-unleashed.json",
        include_str!("../content/decks/nec-upgrades-unleashed.json"),
    ),
    (
        "onc-rebellion-rising.json",
        include_str!("../content/decks/onc-rebellion-rising.json"),
    ),
    (
        "sld-goblin-storm.json",
        include_str!("../content/decks/sld-goblin-storm.json"),
    ),
    (
        "voc-spirit-squadron.json",
        include_str!("../content/decks/voc-spirit-squadron.json"),
    ),
    (
        "woc-fae-dominion.json",
        include_str!("../content/decks/woc-fae-dominion.json"),
    ),
    (
        "znc-sneak-attack.json",
        include_str!("../content/decks/znc-sneak-attack.json"),
    ),
];
