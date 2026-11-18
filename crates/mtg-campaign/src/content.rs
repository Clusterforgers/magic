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
    /// A boss's card, in its command zone from the start.
    Boss,
    /// A boss phase, which begins at a life threshold.
    Phase,
    /// The emblem a hero gets on becoming a Spirit.
    Haunt,
    /// A scheme of a boss's scheme deck, set in motion as one of its scheme turns begins.
    Scheme,
    /// A boss's Enrage, which begins once the heroes have had its `turn` turns.
    Enrage,
    /// A boss buff drawn for each fight from the theme's pool.
    Modifier,
    /// A boss's own risky relic, which it brings from the second boss of the ladder on.
    BossRelic,
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
    /// The heroes' turn from which an Enrage applies; 0 for other cards.
    #[serde(default)]
    pub turn: u32,
    /// What a relic's rules care about in a deck, so offers skip it for a deck with too few such cards; none for a relic
    /// of use to any deck.
    #[serde(default)]
    pub wants: Vec<Want>,
    /// The content generation from which a relic is offered (`RunEvent::Content`); a run of an earlier one never meets it.
    #[serde(default)]
    pub since: u32,
    /// A relic's rarity, which from the rarity generation on decides where it is offered and its shop price.
    #[serde(default)]
    pub rarity: Rarity,
    /// Its abilities, in the card DSL.
    pub abilities: Vec<Ability>,
}

impl CustomCard {
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

impl Brain {
    /// One level stronger, Very Hard staying itself.
    #[must_use]
    pub fn stronger(self) -> Brain {
        match self {
            Brain::Easy => Brain::Normal,
            Brain::Normal => Brain::Hard,
            Brain::Hard | Brain::VeryHard => Brain::VeryHard,
        }
    }

    /// One level weaker, Easy staying itself.
    #[must_use]
    pub fn weaker(self) -> Brain {
        match self {
            Brain::Easy | Brain::Normal => Brain::Easy,
            Brain::Hard => Brain::Normal,
            Brain::VeryHard => Brain::Hard,
        }
    }
}

/// A boss seat: a themed deck of real cards, a brain, life that grows with the party, its boss card and phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boss {
    /// Its id in the content.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Whether it is the theme's final boss, fought last.
    #[serde(default, rename = "final")]
    pub final_boss: bool,
    /// Whether it is an uber boss, which a hot ladder may meet in place of its final boss (`map::generate_for`); never a
    /// lieutenant or a final boss otherwise.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub uber: bool,
    /// Its colours, as WUBRG letters.
    pub colors: String,
    /// How its seat is played.
    pub brain: Brain,
    /// Its life with one hero.
    pub life: i32,
    /// Life it gets for each hero after the first.
    pub life_per_hero: i32,
    /// Its opening hand size.
    pub hand: u32,
    /// Its deck as written before brackets, for runs of an earlier content generation: card names and copies.
    #[serde(default)]
    pub deck: Vec<(String, u32)>,
    /// Its deck by bracket (`scale::BRACKETS`), for runs from `scale::BRACKETS_FROM`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brackets: Option<BracketDeck>,
    /// Its commander card: a real card it casts free from its command zone as its last phase begins.
    #[serde(default)]
    pub commander: String,
    /// The content generation it joined the theme in: a run made before it never meets it.
    #[serde(default)]
    pub since: u32,
    /// Its upgraded form, which may stand as the final boss of a ladder it is not on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upgrade: Option<BossUpgrade>,
    /// The boss an upgraded form is made from; none for a boss as written.
    #[serde(skip)]
    pub upgrade_of: Option<String>,
    /// Custom cards it starts with in its command zone: its passive.
    #[serde(default)]
    pub rules: Vec<String>,
    /// Its Enrage card.
    #[serde(default)]
    pub enrage: String,
    /// Its own risky relic, which it brings from the second boss of the ladder on.
    #[serde(default)]
    pub relic: String,
    /// Its phases: a phase card and the share of its starting life, in percent, at or below which it begins.
    #[serde(default)]
    pub phases: Vec<(String, u32)>,
    /// What it says as the fight begins.
    #[serde(default)]
    pub line: String,
    /// The real card whose art pictures it.
    pub art: BossArt,
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

/// A boss's deck by bracket: the deck of bracket 1 and, for each bracket above it, the cards that bracket adds and
/// cuts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BracketDeck {
    /// The bracket 1 deck: card names and copies.
    pub base: Vec<(String, u32)>,
    /// Brackets 2 and up, in order: each changes the bracket below it.
    pub steps: Vec<DeckStep>,
}

/// How one bracket's deck differs from the bracket below it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckStep {
    /// Cards added: names and copies.
    #[serde(default)]
    pub add: Vec<(String, u32)>,
    /// Cards taken out: names and copies.
    #[serde(default)]
    pub cut: Vec<(String, u32)>,
}

impl BracketDeck {
    /// The deck at `bracket` (from 1; past the last step the last deck): card names and copies, in the order they were
    /// first named.
    pub fn at(&self, bracket: u8) -> Vec<(String, u32)> {
        let mut deck = self.base.clone();
        for step in self
            .steps
            .iter()
            .take(usize::from(bracket.saturating_sub(1)))
        {
            for (name, n) in &step.cut {
                if let Some(entry) = deck.iter_mut().find(|(have, _)| have == name) {
                    entry.1 = entry.1.saturating_sub(*n);
                }
            }
            for (name, n) in &step.add {
                match deck.iter_mut().find(|(have, _)| have == name) {
                    Some(entry) => entry.1 += n,
                    None => deck.push((name.clone(), *n)),
                }
            }
            deck.retain(|(_, n)| *n > 0);
        }
        deck
    }
}

/// A boss's upgraded form: the same identity with a new name and an extra effect, fought as a final boss.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BossUpgrade {
    /// The upgraded form's name.
    pub name: String,
    /// The boss cards it has in addition to the boss's passive.
    pub rules: Vec<String>,
    /// What it says as the fight begins.
    #[serde(default)]
    pub line: String,
    /// The content generation from which the upgraded form stands, at least the boss's own and the bracket generation.
    #[serde(default)]
    pub since: u32,
}

/// The id suffix of a boss's upgraded form.
pub const UPGRADED: &str = "_ascendant";

impl Boss {
    /// The boss whose deck, minions and identity this one shares: the boss it is upgraded from, or itself.
    pub fn identity(&self) -> &str {
        self.upgrade_of.as_deref().unwrap_or(&self.id)
    }

    /// Its deck at `bracket`; the deck as written for bracket 0 or a boss with no bracket decks.
    pub fn deck_at(&self, bracket: u8) -> Vec<(String, u32)> {
        match &self.brackets {
            Some(decks) if bracket > 0 => decks.at(bracket),
            _ => self.deck.clone(),
        }
    }

    /// Its upgraded form as a final boss, if it has one: its own rules and the upgrade's, from the bracket generation.
    pub fn upgraded(&self) -> Option<Boss> {
        let upgrade = self.upgrade.as_ref()?;
        let mut rules = self.rules.clone();
        rules.extend(upgrade.rules.iter().cloned());
        Some(Boss {
            id: format!("{}{UPGRADED}", self.id),
            name: upgrade.name.clone(),
            final_boss: true,
            rules,
            line: upgrade.line.clone(),
            since: self
                .since
                .max(upgrade.since)
                .max(crate::scale::BRACKETS_FROM),
            upgrade: None,
            upgrade_of: Some(self.id.clone()),
            ..self.clone()
        })
    }
}

/// A boss's minion: a lieutenant seat on the boss's side for a big party, with another boss's deck; it falls with the
/// boss.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Minion {
    /// Its id in the content.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The boss whose deck it plays; a fight against that boss does not field it.
    pub deck_of: String,
    /// Its life at Heat 0.
    pub life: i32,
    /// Custom cards it starts with in its command zone.
    #[serde(default)]
    pub rules: Vec<String>,
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
    /// The bosses' scheme deck, shuffled for each fight.
    #[serde(default)]
    pub schemes: Vec<String>,
    /// Every how many boss turns a scheme is set in motion, from the boss's third.
    #[serde(default = "every_turn")]
    pub scheme_every: u32,
    /// The buff modifiers one of which each fight draws for the boss.
    #[serde(default)]
    pub modifiers: Vec<String>,
    /// The minions a boss fields for a big party, in the order they join.
    #[serde(default)]
    pub minions: Vec<Minion>,
    /// The upgraded forms of its bosses, made as the content is read (`Boss::upgraded`).
    #[serde(skip)]
    pub upgraded: Vec<Boss>,
}

impl Theme {
    /// The boss with this id, an upgraded form included.
    pub fn boss(&self, id: &str) -> Option<&Boss> {
        self.bosses
            .iter()
            .chain(&self.upgraded)
            .find(|m| m.id == id)
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
}

impl Content {
    /// Reads the content from its JSON texts: the custom card files and the themes.
    pub fn parse(card_files: &[&str], themes: &[&str]) -> Result<Content, CampaignError> {
        let mut list: Vec<CustomCard> = Vec::new();
        for text in card_files {
            let more: Vec<CustomCard> =
                serde_json::from_str(text).map_err(|e| CampaignError::Content(e.to_string()))?;
            list.extend(more);
        }
        let mut cards: BTreeMap<String, CustomCard> =
            list.into_iter().map(|c| (c.id.clone(), c)).collect();
        let variants = cards
            .values()
            .filter(|c| c.kind == CardKind::Enrage)
            .flat_map(|c| {
                crate::scale::ENRAGE_TURNS
                    .filter(move |turn| *turn != c.turn)
                    .map(move |turn| enrage_at(c, turn))
            })
            .collect::<Result<Vec<_>, _>>()?;
        cards.extend(variants.into_iter().map(|c| (c.id.clone(), c)));
        let mut themes = themes
            .iter()
            .map(|text| {
                serde_json::from_str(text).map_err(|e| CampaignError::Content(e.to_string()))
            })
            .collect::<Result<Vec<Theme>, _>>()?;
        for theme in &mut themes {
            theme.upgraded = theme.bosses.iter().filter_map(Boss::upgraded).collect();
        }
        Ok(Content { cards, themes })
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

fn every_turn() -> u32 {
    1
}

/// The Enrage `card` made to begin on the heroes' turn `turn` (`scale::ENRAGE_TURNS`): its id, `turn`, rules text and
/// the turn counts its abilities compare with and count beyond.
fn enrage_at(card: &CustomCard, turn: u32) -> Result<CustomCard, CampaignError> {
    const WORDS: [&str; 13] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve",
    ];
    let word = |n: u32| {
        WORDS
            .get(usize::try_from(n).unwrap_or(usize::MAX))
            .copied()
            .unwrap_or("many")
    };
    let fail = |e: serde_json::Error| CampaignError::Content(e.to_string());
    let mut abilities = serde_json::to_value(&card.abilities).map_err(fail)?;
    retime(&mut abilities, card.turn, turn);
    let text = card
        .text
        .replace(
            &format!("have had {} turns", word(card.turn)),
            &format!("have had {} turns", word(turn)),
        )
        .replace(
            &format!("beyond {}", word(card.turn.saturating_sub(1))),
            &format!("beyond {}", word(turn.saturating_sub(1))),
        );
    Ok(CustomCard {
        id: crate::scale::enrage_id(&card.id, turn),
        text,
        turn,
        abilities: serde_json::from_value(abilities).map_err(fail)?,
        ..card.clone()
    })
}

/// Moves the heroes' turn counts in `value` from an Enrage written for turn `from` to one beginning on `to`: a
/// comparison of the opponents' turns with `from` and a count of their turns beyond `from − 1`.
fn retime(value: &mut serde_json::Value, from: u32, to: u32) {
    use serde_json::{Value, json};
    let theirs = json!({ "TurnsTaken": "Opponents" });
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(compare)) = map.get_mut("Compare")
                && compare.get("left") == Some(&theirs)
                && compare.get("right") == Some(&json!(from))
            {
                compare.insert("right".into(), json!(to));
            }
            if let Some(Value::Array(pair)) = map.get_mut("Difference")
                && pair.first() == Some(&theirs)
                && pair.get(1) == Some(&json!(from.saturating_sub(1)))
                && let Some(count) = pair.get_mut(1)
            {
                *count = json!(to.saturating_sub(1));
            }
            map.values_mut().for_each(|v| retime(v, from, to));
        }
        Value::Array(items) => items.iter_mut().for_each(|v| retime(v, from, to)),
        _ => {}
    }
}

/// The content shipped with the crate, read once.
pub fn content() -> Result<&'static Content, CampaignError> {
    static CONTENT: OnceLock<Result<Content, CampaignError>> = OnceLock::new();
    CONTENT
        .get_or_init(|| {
            Content::parse(
                &[
                    include_str!("../content/cards.json"),
                    include_str!("../content/boss_cards.json"),
                ],
                &[include_str!("../content/warrens.json")],
            )
        })
        .as_ref()
        .map_err(Clone::clone)
}
