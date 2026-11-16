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
    /// Its deck: card names and copies.
    pub deck: Vec<(String, u32)>,
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
    /// A chance: `win` with `odds` percent, `lose` otherwise.
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
}

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
            Outcome::Gamble { odds, win, lose } => format!(
                "{odds}%: {} · {}%: {}",
                list_words(win),
                100u32.saturating_sub(*odds),
                list_words(lose)
            ),
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

    /// What a hero must have to take it: gold and life for its sure costs.
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
}

impl Content {
    /// Reads the content from its JSON texts.
    pub fn parse(cards: &str, themes: &[&str]) -> Result<Content, CampaignError> {
        let list: Vec<CustomCard> =
            serde_json::from_str(cards).map_err(|e| CampaignError::Content(e.to_string()))?;
        let mut cards: BTreeMap<String, CustomCard> =
            list.into_iter().map(|c| (c.id.clone(), c)).collect();
        let variants = cards
            .values()
            .filter(|c| c.kind == CardKind::Enrage)
            .flat_map(|c| {
                crate::scale::ENRAGE_TURN
                    .iter()
                    .filter(move |turn| **turn != c.turn)
                    .map(move |turn| enrage_at(c, *turn))
            })
            .collect::<Result<Vec<_>, _>>()?;
        cards.extend(variants.into_iter().map(|c| (c.id.clone(), c)));
        let themes = themes
            .iter()
            .map(|text| {
                serde_json::from_str(text).map_err(|e| CampaignError::Content(e.to_string()))
            })
            .collect::<Result<Vec<Theme>, _>>()?;
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

/// The Enrage `card` made to begin on the heroes' turn `turn` (`scale::ENRAGE_TURN`): its id, `turn`, rules text and
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
                include_str!("../content/cards.json"),
                &[include_str!("../content/warrens.json")],
            )
        })
        .as_ref()
        .map_err(Clone::clone)
}
