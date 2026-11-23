//! A run as the fold of its events: the party gathers in the lobby, fights the bosses of the ladder one after another
//! (each a fresh game; life carries over), takes loot after each win and votes for a camp between two bosses. Every
//! random draw comes from the run's seed keyed by room and hero, so folding the same events always gives the same run.

use std::collections::BTreeMap;

use mtg_core::house::Timers;
use serde::{Deserialize, Serialize};

use crate::CampaignError;
use crate::content::{
    Brain, Content, CustomCard, EventOption, MIN_LIFE_CAP, Needs, Outcome, Rarity, Theme,
};
use crate::fight::{FightPlan, HAND, HeroSeat};
use crate::map::{FloorMap, RoomKind, generate, generate_floor, generate_for};
use crate::relics::{
    DeckMakeup, FILTER_FROM, RARE_ODDS, RARITY_FROM, TREASURE_RARE_BONUS, offerable,
};
use crate::rng::Stream;
use crate::upgrade::{CommanderFacts, Upgrade, fitting};

/// The most heroes a party has.
pub const PARTY: usize = crate::scale::MOST;
/// A hero's life with no relic.
pub const LIFE: i32 = 40;
/// The fights the party may lose and retry.
pub const HEARTS: u8 = 3;
/// Life each hero gets back after a boss falls.
pub const HEAL_AFTER_BOSS: i32 = 15;
/// Gold every hero takes with a won boss's loot.
pub const LOOT_GOLD: u32 = 50;
/// Life a Campfire rest gives.
pub const REST: i32 = 20;
/// Life a hero who ended a won fight as a Spirit comes back with.
pub const SPIRIT_RETURN: i32 = 10;
/// Gold for skipping a card pick.
pub const SKIP_GOLD: u32 = 10;
/// The price of a card at the shop.
pub const CARD_PRICE: u32 = 35;
/// The price of the shop's healing draught.
pub const DRAUGHT_PRICE: u32 = 30;
/// Life the draught gives.
pub const DRAUGHT: i32 = 15;
/// The price of a commander upgrade at the shop.
pub const UPGRADE_PRICE: u32 = 50;
/// The content generation a new run is made with (`RunEvent::Content`); a run stored without one is generation 0. From 2
/// relic offers follow the deck and the relics of the larger pool are offered; from 3 relics have rarities and events are
/// known trade-offs; from 4 a ladder draws three lieutenants and one final boss; from 5 the roster grows by more
/// two-colour bosses and a cleared floor may descend; from 6 a boss stands for every colour pair; from 7 a descent
/// first gives the final boss's loot; from 8 a relic drops again for a hero who owns it; from 9 each descent leads into
/// another theme; from 10 the Drowned Spire holds a lieutenant for every colour and colour pair and a second final boss.
/// Every run plays the boss of `docs/delves.md` (Bosses), whatever its generation.
pub const CONTENT: u32 = SPIRE_GROWN_FROM;
/// The content generation from which each descent leads into another open theme (`Content::next_theme`).
pub const THEMES_FROM: u32 = 9;
/// The content generation from which the Drowned Spire's ladder draws from its grown roster.
pub const SPIRE_GROWN_FROM: u32 = 10;
/// The content generation from which owning a relic never lowers its chance in an offer, and copies stack.
pub const RELICS_AGAIN_FROM: u32 = 8;
/// The content generation from which the party may descend to a deeper floor after a floor's final boss.
pub const ENDLESS_FROM: u32 = 5;
/// The content generation from which a descent first gives the loot of the final boss that fell.
pub const DESCENT_LOOT_FROM: u32 = 7;
/// The hearts a descent gives back, up to [`HEARTS`].
pub const DESCENT_HEARTS: u8 = 1;
/// The deepest floor a run reaches; a party on it can only bank the win.
pub const DEEPEST: u8 = 99;

/// The deck a hero brings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct HeroDeck {
    /// The deck choice the server resolves (a precon or a stored deck).
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its colour identity as WUBRG letters, which card rewards follow.
    pub colors: String,
    /// Its commanders as their upgrades see them; none for a deck chosen before upgrades, which then gets none.
    #[serde(default)]
    pub commanders: Vec<CommanderFacts>,
    /// What it is made of, for the relic offers; none for a deck chosen before offers followed the deck, which then
    /// gets them unfiltered.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub makeup: Option<DeckMakeup>,
}

/// A choice waiting for one hero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Offer {
    /// Take one card into the Satchel or skip for gold.
    Cards {
        /// The cards offered.
        cards: Vec<String>,
    },
    /// Take one relic or none.
    Relics {
        /// The relics offered.
        relics: Vec<String>,
    },
    /// Buy what the purse allows, then leave.
    Shop {
        /// The wares.
        items: Vec<ShopItem>,
    },
    /// Rest, reforge, or temper: take one of these commander upgrades (none when nothing fits the commanders).
    Campfire {
        /// The upgrades offered.
        upgrades: Vec<Upgrade>,
    },
    /// At the Mirror Pool: copy a nonland card of the deck or Satchel into the Satchel, or skip.
    Duplicate,
    /// One option of an event.
    Event {
        /// The event's id.
        event: String,
    },
    /// An event's cost: name a card of the deck or Satchel to give up (`Pick::Discard`); no skipping.
    Discard,
    /// An event's cost: give up one of these relics (`Pick::Take`); no skipping.
    Surrender {
        /// The relics the hero holds.
        relics: Vec<String>,
    },
}

/// One ware of a shop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ShopItem {
    /// A card for the deck.
    Card {
        /// Its name.
        name: String,
        /// Whether it was bought.
        sold: bool,
    },
    /// A relic.
    Relic {
        /// Its id.
        id: String,
        /// Its price.
        price: u32,
        /// Whether it was bought.
        sold: bool,
    },
    /// A healing draught.
    Draught,
    /// A commander upgrade.
    Upgrade {
        /// The upgrade.
        upgrade: Upgrade,
        /// Whether it was bought.
        sold: bool,
    },
}

/// A hero's answer to their first offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Pick {
    /// The item, relic, card, upgrade or option with this index.
    Take(u8),
    /// Nothing (gold for a card pick); leaves a shop.
    Skip,
    /// At a campfire: life.
    Rest,
    /// At a campfire: a card pick.
    Reforge,
    /// At the Mirror Pool: the card to copy, by name, from the deck or Satchel.
    Duplicate(String),
    /// For an event's cost: the card to give up, by name, from the Satchel or else the deck.
    Discard(String),
}

/// One roll of a gamble an answer ran into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GambleRoll {
    /// The chance to win, in percent.
    pub odds: u32,
    /// Whether it was won.
    pub won: bool,
}

/// What a hero's answer did: the chronicle's line and the run page's result panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Resolved {
    /// The hero who answered.
    pub hero: u8,
    /// What they did, in past tense after their name: `chose “Push past”`, `took the relic Phoenix Feather`.
    pub what: String,
    /// What it changed, as it fell out: gambles decided, life and gold as they really moved.
    pub outcomes: Vec<Outcome>,
    /// The gambles it ran into.
    pub rolls: Vec<GambleRoll>,
}

/// A member of the party.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Hero {
    /// Their name at the table.
    pub name: String,
    /// The account that plays them; `None` for a bot ally.
    pub user: Option<String>,
    /// The brain of a bot ally.
    pub bot: Option<Brain>,
    /// Their deck, once chosen.
    pub deck: Option<HeroDeck>,
    /// Their life, carried from fight to fight.
    pub life: i32,
    /// Their gold.
    pub gold: u32,
    /// Their relics, in the order they got them.
    pub relics: Vec<String>,
    /// Their Satchel: every card they earned in the run, in the order they got it, castable from it in each fight.
    pub satchel: Vec<String>,
    /// Their commander upgrades, in the order they got them, and the banes events cost them.
    pub upgrades: Vec<Upgrade>,
    /// The choices waiting for them, first one first.
    pub offers: Vec<Offer>,
    /// How far events moved their life cap for the rest of the run (below 0 for a cut).
    #[serde(default)]
    pub life_cap: i32,
    /// Cards of their deck they gave up for the rest of the run, by name, a copy each.
    #[serde(default)]
    pub lost: Vec<String>,
}

impl Hero {
    /// Whether a copy of `card` is still in their deck: its library names the card more often than they gave it up. A
    /// deck whose library names were not kept holds any card; no deck holds none.
    fn deck_holds(&self, card: &str) -> bool {
        let Some(deck) = self.deck.as_ref() else {
            return false;
        };
        let Some(library) = deck
            .makeup
            .as_ref()
            .map(|makeup| &makeup.library)
            .filter(|library| !library.is_empty())
        else {
            return true;
        };
        let given = self.lost.iter().filter(|lost| *lost == card).count();
        let copies = library.get(card).copied().unwrap_or(0);
        usize::try_from(copies).unwrap_or(usize::MAX) > given
    }

    /// Their life cap: the base life plus their relics' bonus, moved by events.
    pub fn max_life(&self, content: &Content) -> i32 {
        LIFE + self.life_cap
            + self
                .relics
                .iter()
                .filter_map(|id| content.cards.get(id))
                .map(|c| c.life)
                .sum::<i32>()
    }

    /// Why this hero cannot take an option with these needs, if they cannot: gold, life or life cap short, no relic to
    /// give up, no commander (or no creature commander) to weaken.
    pub fn lacks(&self, content: &Content, needs: &Needs) -> Option<String> {
        if let Some(why) = needs.unmet(self.gold, self.life) {
            return Some(why);
        }
        let cap = self.max_life(content);
        if needs.max_life > 0
            && i64::from(cap) - i64::from(needs.max_life) < i64::from(MIN_LIFE_CAP)
        {
            return Some(format!(
                "needs a life cap of at least {MIN_LIFE_CAP} after the cut, yours is {cap}"
            ));
        }
        if needs.relic && self.relics.is_empty() {
            return Some("needs a relic to give up".to_owned());
        }
        let commanders = self
            .deck
            .as_ref()
            .map_or(&[][..], |d| d.commanders.as_slice());
        if needs.creature_commander && !commanders.first().is_some_and(|c| c.creature) {
            return Some("needs a creature commander".to_owned());
        }
        if needs.commander && commanders.is_empty() {
            return Some("needs a commander".to_owned());
        }
        None
    }

    fn heal(&mut self, content: &Content, amount: i32) {
        self.life = (self.life + amount).clamp(1, self.max_life(content));
    }
}

/// A fight whose game ended, for its log: the recorded game, the room and whether the party won.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FoughtFight {
    /// The game's id, whose replay holds the fight's log.
    pub game: String,
    /// The boss's room on its floor's ladder.
    pub room: u8,
    /// The floor it was fought on.
    #[serde(default = "first_floor")]
    pub floor: u8,
    /// The boss, by id.
    #[serde(default)]
    pub boss: Option<String>,
    /// Whether the party won.
    pub won: bool,
}

/// Where the run is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Status {
    /// The party gathers and chooses decks.
    Lobby,
    /// The party votes for the next room.
    Map,
    /// A boss fight: its game once created.
    Fight {
        /// The game's id.
        game: Option<String>,
    },
    /// The heroes answer their offers.
    Room,
    /// The floor's final boss fell: the party votes to bank the win or descend (the endless generation).
    FloorCleared,
    /// The final boss fell and the party banked the win.
    Victory,
    /// The party lost a fight with no hearts left.
    Defeat,
    /// The host ended the run.
    Abandoned,
}

/// Why a fight counts as lost with no result from its game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Forfeit {
    /// Its table was closed or expired before the game had a result.
    Closed,
    /// The host retreated from it.
    Retreat,
}

/// Something that happened in a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunEvent {
    /// A hero joined the party.
    Joined {
        /// Their name.
        name: String,
        /// Their account; `None` for a bot ally.
        user: Option<String>,
        /// A bot ally's brain.
        bot: Option<Brain>,
    },
    /// A hero left the lobby.
    Left {
        /// Their index.
        hero: u8,
    },
    /// A hero chose a deck.
    DeckChosen {
        /// Their index.
        hero: u8,
        /// The deck.
        deck: HeroDeck,
    },
    /// The content generation the run is made with, stored as it is created.
    Content {
        /// The generation (`CONTENT`).
        version: u32,
    },
    /// The host chose how long the party may take in a fight's game, in the lobby.
    Timers {
        /// The setting.
        timers: Timers,
    },
    /// The party left the lobby.
    Started,
    /// A hero voted for the next room.
    Voted {
        /// Their index.
        hero: u8,
        /// The room.
        node: u8,
    },
    /// The fight's game was created.
    FightCreated {
        /// Its id.
        game: String,
    },
    /// The fight's game ended.
    FightEnded {
        /// Whether the party won.
        won: bool,
        /// Each hero's life at the end.
        lives: Vec<i32>,
        /// The heroes who ended it as Spirits.
        spirits: Vec<u8>,
    },
    /// The fight's game ended unplayed (an admin ended it, or it failed to load): the fight waits for a new game.
    FightVoided,
    /// The fight counts as lost with no result from its game: the table closed or expired, or the host retreated.
    FightForfeited {
        /// Why.
        cause: Forfeit,
    },
    /// A hero answered their first offer.
    Picked {
        /// Their index.
        hero: u8,
        /// The answer.
        pick: Pick,
    },
    /// The host ended the run.
    Abandoned,
    /// A hero voted, on a cleared floor, to descend (`true`) or bank the win.
    FloorVoted {
        /// Their index.
        hero: u8,
        /// Whether they vote to descend.
        descend: bool,
    },
}

fn first_floor() -> u8 {
    1
}

/// A theme's name inside a sentence: "The Drowned Spire" as "the Drowned Spire".
pub fn in_sentence(name: &str) -> String {
    name.strip_prefix("The ")
        .map_or_else(|| name.to_owned(), |rest| format!("the {rest}"))
}

/// A run: its seed and theme, the ladder, the party and where they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Run {
    /// The seed every draw comes from; the web reads it as the run's code instead.
    #[cfg_attr(feature = "ts", ts(skip))]
    pub seed: u64,
    /// The difficulty level.
    pub heat: u8,
    /// The content generation it was made with: events of a later one stay out of it, so a stored run replays alike.
    #[serde(default)]
    pub content: u32,
    /// How long the party may take in each fight's game: relaxed unless the host chose otherwise.
    #[serde(default)]
    pub timers: Timers,
    /// The theme's id: the first floor's.
    pub theme: String,
    /// The theme of the floor the party is on, once a descent chose it; none while that is the first floor's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub floor_theme: Option<String>,
    /// The theme of the floor above, when the descent onto this floor left it for another.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub theme_from: Option<String>,
    /// The ladder of the floor the party is on.
    pub map: FloorMap,
    /// The floor the party is on, from 1.
    #[serde(default = "first_floor")]
    pub floor: u8,
    /// The floors whose final boss fell.
    #[serde(default)]
    pub floors_cleared: u8,
    /// Each hero's vote on a cleared floor: `true` to descend, `false` to bank the win.
    #[serde(default)]
    pub floor_votes: BTreeMap<u8, bool>,
    /// Whether the party voted to descend and answers the final boss's loot first.
    #[serde(default)]
    pub descending: bool,
    /// The party.
    pub heroes: Vec<Hero>,
    /// The room the party is in or last left.
    pub at: Option<u8>,
    /// The rooms cleared, in order.
    pub cleared: Vec<u8>,
    /// Each hero's vote for the next room.
    pub votes: BTreeMap<u8, u8>,
    /// The party's hearts: lost fights left before defeat.
    pub hearts: u8,
    /// Where the run is.
    pub status: Status,
    /// What happened, in words, newest last.
    pub log: Vec<String>,
    /// The fights whose game ended with a result or lost its table, oldest first.
    #[serde(default)]
    pub fights: Vec<FoughtFight>,
    /// What the heroes' answers in the camp last visited did, in order; cleared when the next camp opens or a fight is won.
    pub results: Vec<Resolved>,
}

fn refused(text: impl Into<String>) -> CampaignError {
    CampaignError::Refused(text.into())
}

impl Run {
    /// A new run of `theme` in its lobby.
    pub fn new(content: &Content, theme: &str, seed: u64, heat: u8) -> Result<Run, CampaignError> {
        let map = generate(content.theme(theme)?, seed);
        Ok(Run {
            seed,
            heat,
            content: 0,
            theme: theme.to_owned(),
            floor_theme: None,
            theme_from: None,
            timers: Timers::default(),
            map,
            floor: 1,
            floors_cleared: 0,
            floor_votes: BTreeMap::new(),
            descending: false,
            heroes: Vec::new(),
            at: None,
            cleared: Vec::new(),
            votes: BTreeMap::new(),
            hearts: HEARTS,
            status: Status::Lobby,
            log: Vec::new(),
            fights: Vec::new(),
            results: Vec::new(),
        })
    }

    /// The run after `events`.
    pub fn replay(
        content: &Content,
        theme: &str,
        seed: u64,
        heat: u8,
        events: &[RunEvent],
    ) -> Result<Run, CampaignError> {
        let mut run = Run::new(content, theme, seed, heat)?;
        for event in events {
            run.apply(content, event)?;
        }
        Ok(run)
    }

    /// The id of the theme of the floor the party is on.
    pub fn floor_theme(&self) -> &str {
        self.floor_theme.as_deref().unwrap_or(&self.theme)
    }

    /// The theme of the floor the party is on: its bosses, events and reward pool.
    fn theme<'a>(&self, content: &'a Content) -> Result<&'a Theme, CampaignError> {
        content.theme(self.floor_theme())
    }

    fn hero_mut(&mut self, hero: u8) -> Result<&mut Hero, CampaignError> {
        self.heroes
            .get_mut(usize::from(hero))
            .ok_or_else(|| refused(format!("no hero {hero}")))
    }

    /// The hero who breaks tied votes: it moves one seat on with every room cleared.
    pub fn torch(&self) -> u8 {
        let n = self.heroes.len().max(1);
        u8::try_from(self.cleared.len() % n).unwrap_or(0)
    }

    /// Refuses a new event that is not allowed now, which a stored log may still hold (`replay` folds it as before):
    /// an event option whose needs the hero lacks (`Hero::lacks`).
    pub fn accepts(&self, content: &Content, event: &RunEvent) -> Result<(), CampaignError> {
        let RunEvent::Picked {
            hero,
            pick: Pick::Take(i),
        } = event
        else {
            return Ok(());
        };
        let Some(h) = self.heroes.get(usize::from(*hero)) else {
            return Ok(());
        };
        if let Some(Offer::Event { event }) = h.offers.first()
            && let Some(option) = self
                .theme(content)?
                .event(event)
                .and_then(|spec| spec.options.get(usize::from(*i)))
            && let Some(why) = h.lacks(content, &option.needs())
        {
            return Err(refused(format!("“{}” {why}", option.label)));
        }
        Ok(())
    }

    /// Folds one event, or refuses it and changes nothing.
    pub fn apply(&mut self, content: &Content, event: &RunEvent) -> Result<(), CampaignError> {
        let mut next = self.clone();
        next.fold(content, event)?;
        *self = next;
        Ok(())
    }

    // One arm for each event, so the match is as long as the event list.
    #[allow(clippy::too_many_lines)]
    fn fold(&mut self, content: &Content, event: &RunEvent) -> Result<(), CampaignError> {
        match event {
            RunEvent::Joined { name, user, bot } => {
                self.lobby()?;
                if self.heroes.len() >= PARTY {
                    return Err(refused("the party is full"));
                }
                if user.is_some() && self.heroes.iter().any(|h| h.user == *user) {
                    return Err(refused("already in the party"));
                }
                if user.is_none() == bot.is_none() {
                    return Err(refused("a hero is a player or a bot"));
                }
                self.heroes.push(Hero {
                    name: name.clone(),
                    user: user.clone(),
                    bot: *bot,
                    deck: None,
                    life: LIFE,
                    gold: 0,
                    relics: Vec::new(),
                    satchel: Vec::new(),
                    upgrades: Vec::new(),
                    offers: Vec::new(),
                    life_cap: 0,
                    lost: Vec::new(),
                });
                self.log.push(format!("{name} joined the party."));
            }
            RunEvent::Left { hero } => {
                self.lobby()?;
                let at = usize::from(*hero);
                if at >= self.heroes.len() {
                    return Err(refused(format!("no hero {hero}")));
                }
                let gone = self.heroes.remove(at);
                self.log.push(format!("{} left the party.", gone.name));
            }
            RunEvent::DeckChosen { hero, deck } => {
                // In the lobby; or, while a fight waits for its game, in place of a deck that no longer plays.
                if self.status != Status::Lobby && self.status != (Status::Fight { game: None }) {
                    return Err(refused(
                        "decks change only through rewards once the run has started",
                    ));
                }
                if deck.colors.chars().any(|c| !"WUBRG".contains(c)) {
                    return Err(refused("colours are WUBRG letters"));
                }
                let h = self.hero_mut(*hero)?;
                h.deck = Some(deck.clone());
            }
            RunEvent::Timers { timers } => {
                self.lobby()?;
                self.timers = *timers;
            }
            RunEvent::Content { version } => {
                self.lobby()?;
                self.content = *version;
                // The ladder is drawn from the bosses of the run's generation.
                self.map = generate_for(self.theme(content)?, self.seed, *version);
            }
            RunEvent::Started => {
                self.lobby()?;
                if !self.heroes.iter().any(|h| h.user.is_some()) {
                    return Err(refused("the party needs a player"));
                }
                if let Some(h) = self.heroes.iter().find(|h| h.deck.is_none()) {
                    return Err(refused(format!("{} has no deck", h.name)));
                }
                self.status = Status::Map;
                // Older runs keep the words they were played with.
                let name = if self.content >= THEMES_FROM {
                    in_sentence(&self.theme(content)?.name)
                } else {
                    "the Warrens".to_owned()
                };
                self.log.push(format!("The party descends into {name}."));
                // The first room is the first boss: no vote.
                if let [only] = self.map.choices(None).as_slice() {
                    self.enter(content, *only)?;
                }
            }
            RunEvent::Voted { hero, node } => {
                // Stored runs hold votes for the boss after a camp, which the party now enters on its own.
                if self.status == (Status::Fight { game: None }) && self.at == Some(*node) {
                    return Ok(());
                }
                if self.status != Status::Map {
                    return Err(refused("no vote now"));
                }
                if usize::from(*hero) >= self.heroes.len() {
                    return Err(refused(format!("no hero {hero}")));
                }
                if !self.map.choices(self.at).contains(node) {
                    return Err(refused("that room is not next"));
                }
                self.votes.insert(*hero, *node);
                if self.votes.len() == self.heroes.len() {
                    let chosen = self.tally();
                    self.votes.clear();
                    self.enter(content, chosen)?;
                }
            }
            RunEvent::FightCreated { game } => match &self.status {
                Status::Fight { game: None } => {
                    self.status = Status::Fight {
                        game: Some(game.clone()),
                    };
                }
                _ => return Err(refused("no fight waits for a game")),
            },
            RunEvent::FightEnded {
                won,
                lives,
                spirits,
            } => {
                let Status::Fight { game: Some(game) } = &self.status else {
                    return Err(refused("no fight is running"));
                };
                self.record_fight(game.clone(), *won);
                self.fight_ended(content, *won, lives, spirits)?;
            }
            RunEvent::FightVoided => {
                if !matches!(self.status, Status::Fight { game: Some(_) }) {
                    return Err(refused("no fight is running"));
                }
                self.status = Status::Fight { game: None };
            }
            RunEvent::FightForfeited { cause } => {
                // A closed table had a game; a retreat also ends a fight still waiting for one.
                let live = matches!(
                    (&self.status, cause),
                    (Status::Fight { game: Some(_) }, _)
                        | (Status::Fight { game: None }, Forfeit::Retreat)
                );
                if !live {
                    return Err(refused("there is no fight to leave"));
                }
                if let Status::Fight { game: Some(game) } = &self.status {
                    self.record_fight(game.clone(), false);
                }
                let how = match cause {
                    Forfeit::Closed => "lost its table",
                    Forfeit::Retreat => "retreated",
                };
                self.fight_lost(content, how);
            }
            RunEvent::Picked { hero, pick } => {
                if self.status != Status::Room {
                    return Err(refused("nothing to pick now"));
                }
                self.pick(content, *hero, pick.clone())?;
                if self.heroes.iter().all(|h| h.offers.is_empty()) {
                    if self.descending {
                        self.descending = false;
                        return self.descend(content);
                    }
                    // A single way on (a camp's next boss) needs no vote: the party walks into it.
                    match self.map.choices(self.at).as_slice() {
                        [] => self.status = Status::Victory,
                        [only] => self.enter(content, *only)?,
                        _ => self.status = Status::Map,
                    }
                }
            }
            RunEvent::Abandoned => {
                if self.is_over() {
                    return Err(refused("the run is over"));
                }
                self.status = Status::Abandoned;
                self.log.push("The party turned back.".to_owned());
            }
            RunEvent::FloorVoted { hero, descend } => {
                if self.status != Status::FloorCleared {
                    return Err(refused("no floor to leave now"));
                }
                if usize::from(*hero) >= self.heroes.len() {
                    return Err(refused(format!("no hero {hero}")));
                }
                if *descend && self.floor >= DEEPEST {
                    return Err(refused("there is no deeper floor"));
                }
                self.floor_votes.insert(*hero, *descend);
                if self.floor_votes.len() == self.heroes.len() {
                    let descend = self.floor_tally();
                    self.floor_votes.clear();
                    if descend && self.content >= DESCENT_LOOT_FROM {
                        self.descent_loot(content)?;
                    } else if descend {
                        self.descend(content)?;
                    } else {
                        self.bank();
                    }
                }
            }
        }
        Ok(())
    }

    /// Whether the floor votes descend: more votes to descend than to bank, a tie going to the torch bearer's vote.
    fn floor_tally(&self) -> bool {
        let down = self.floor_votes.values().filter(|d| **d).count();
        let bank = self.floor_votes.len() - down;
        match down.cmp(&bank) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => self
                .floor_votes
                .get(&self.torch())
                .copied()
                .unwrap_or(false),
        }
    }

    /// The party banks the win: the run ends in victory.
    fn bank(&mut self) {
        self.status = Status::Victory;
        let floors = self.floors_cleared;
        self.log.push(format!(
            "The party banks the win after {floors} floor{}.",
            if floors == 1 { "" } else { "s" }
        ));
    }

    /// The party voted to descend: every hero first takes the final boss's loot, as after a lieutenant (gold, a relic
    /// pick and a card pick drawn for the floor), and the descent follows once every hero answered.
    fn descent_loot(&mut self, content: &Content) -> Result<(), CampaignError> {
        let node = self.at.ok_or_else(|| refused("no room"))?;
        self.results.clear();
        self.loot(content, node, &format!("loot-floor{}", self.floor))?;
        self.descending = true;
        self.status = Status::Room;
        self.log
            .push("The party takes the final boss's loot before the descent.".to_owned());
        Ok(())
    }

    /// Every hero takes a won boss's loot in room `node`: gold, a relic pick and a card pick drawn under `label`.
    fn loot(&mut self, content: &Content, node: u8, label: &str) -> Result<(), CampaignError> {
        let theme = self.theme(content)?.clone();
        for hero in 0..self.heroes.len() {
            if let Some(h) = self.heroes.get_mut(hero) {
                h.gold += LOOT_GOLD;
            }
            let relics = self.relic_offer(content, node, hero, label, 3, None);
            self.push_offer(hero, Offer::Relics { relics });
            let cards = self.card_offer(&theme, node, hero, label, 3);
            self.push_offer(hero, Offer::Cards { cards });
        }
        Ok(())
    }

    /// The party descends: the next floor's ladder, drawn from the seed and avoiding the bosses just fought, a heart
    /// back (up to [`HEARTS`]); the heroes keep everything else, and the floor's first boss awaits.
    fn descend(&mut self, content: &Content) -> Result<(), CampaignError> {
        let fought: Vec<String> = self
            .map
            .nodes
            .iter()
            .filter_map(|n| n.boss.clone())
            .collect();
        let avoid: Vec<&str> = fought.iter().map(String::as_str).collect();
        if let Some(next) = self.next_floor_theme(content)? {
            let from = self.floor_theme().to_owned();
            self.floor_theme = Some(next.id.clone());
            self.theme_from = (next.id != from).then_some(from);
        }
        self.floor = self.floor.saturating_add(1);
        self.map = generate_floor(
            self.theme(content)?,
            self.seed,
            self.content,
            self.floor,
            &avoid,
        );
        self.hearts = self.hearts.saturating_add(DESCENT_HEARTS).min(HEARTS);
        self.at = None;
        self.cleared.clear();
        self.votes.clear();
        self.results.clear();
        self.status = Status::Map;
        self.log.push(format!(
            "The party descends to floor {} ({} hearts).",
            self.floor, self.hearts
        ));
        if self.theme_from.is_some() {
            let name = in_sentence(&self.theme(content)?.name);
            self.log.push(format!("The party descends into {name}."));
        }
        if let [only] = self.map.choices(None).as_slice() {
            self.enter(content, *only)?;
        }
        Ok(())
    }

    /// The theme of the floor below, from [`THEMES_FROM`] on: another open theme drawn by the seed keyed by that floor
    /// (`Content::next_theme`); none for an older run, whose floors all play the first floor's theme.
    pub fn next_floor_theme<'a>(
        &self,
        content: &'a Content,
    ) -> Result<Option<&'a Theme>, CampaignError> {
        if self.content < THEMES_FROM {
            return Ok(None);
        }
        let below = self.floor.saturating_add(1);
        content
            .next_theme(self.seed, self.content, below, self.floor_theme())
            .map(Some)
    }

    /// What a descent changes, one line each, for the choice on a cleared floor: empty while there is none to make.
    pub fn descent_words(&self, content: &Content) -> Vec<String> {
        if self.status != Status::FloorCleared {
            return Vec::new();
        }
        let next = self.floor.saturating_add(1);
        let into = self
            .next_floor_theme(content)
            .ok()
            .flatten()
            .filter(|theme| theme.id != self.floor_theme())
            .map_or_else(String::new, |theme| {
                format!(" in {}", in_sentence(&theme.name))
            });
        let depth = self.defeated();
        let hearts = self.hearts.saturating_add(DESCENT_HEARTS).min(HEARTS);
        let loot = (self.content >= DESCENT_LOOT_FROM).then(|| {
            format!(
                "First the final boss's loot: {LOOT_GOLD} gold, a relic and a card for each hero"
            )
        });
        let mut words: Vec<String> = loot.into_iter().collect();
        words.extend([
            format!(
                "Floor {next}{into}: new bosses, their innates at level {} from the first fight and growing, more relics the deeper the party goes",
                crate::scale::Scale::innate_level(depth, self.heat)
            ),
            format!(
                "The party keeps its life, relics, Satchels and upgrades, and has {hearts} hearts"
            ),
            "Bank the win to end the run in victory now".to_owned(),
        ]);
        words
    }

    fn lobby(&self) -> Result<(), CampaignError> {
        if self.status == Status::Lobby {
            Ok(())
        } else {
            Err(refused("the run has started"))
        }
    }

    /// Whether bot allies vote to descend from a cleared floor: the party holds at least two
    /// hearts and 60 % of its life cap.
    pub fn bots_descend(&self, content: &Content) -> bool {
        let life: i64 = self.heroes.iter().map(|h| i64::from(h.life.max(0))).sum();
        let cap: i64 = self
            .heroes
            .iter()
            .map(|h| i64::from(h.max_life(content)))
            .sum();
        self.hearts >= 2 && life * 100 >= cap * 60 && self.floor < DEEPEST
    }

    /// How deep the party went, as the leaderboard ranks it: the floors whose final boss fell, then the bosses beaten
    /// on the floor below them (none while the party stands on a cleared floor).
    pub fn depth(&self) -> (u8, u8) {
        let beyond = if self.floors_cleared >= self.floor {
            0
        } else {
            let bosses = self
                .cleared
                .iter()
                .filter(|id| self.map.node(**id).is_some_and(|n| n.boss.is_some()))
                .count();
            u8::try_from(bosses).unwrap_or(u8::MAX)
        };
        (self.floors_cleared, beyond)
    }

    /// Whether the run ended.
    pub fn is_over(&self) -> bool {
        matches!(
            self.status,
            Status::Victory | Status::Defeat | Status::Abandoned
        )
    }

    /// The room the votes choose: the most votes, a tie going to the torch bearer's vote, else the lowest room.
    fn tally(&self) -> u8 {
        let mut counts: BTreeMap<u8, usize> = BTreeMap::new();
        for node in self.votes.values() {
            *counts.entry(*node).or_default() += 1;
        }
        let best = counts.values().copied().max().unwrap_or(0);
        let tied: Vec<u8> = counts
            .iter()
            .filter(|(_, n)| **n == best)
            .map(|(node, _)| *node)
            .collect();
        match self.votes.get(&self.torch()) {
            Some(vote) if tied.contains(vote) => *vote,
            _ => tied.first().copied().unwrap_or(0),
        }
    }

    fn enter(&mut self, content: &Content, node: u8) -> Result<(), CampaignError> {
        let room = self
            .map
            .node(node)
            .cloned()
            .ok_or_else(|| refused(format!("no room {node}")))?;
        self.at = Some(node);
        if room.kind != RoomKind::Boss {
            self.results.clear();
        }
        let theme = self.theme(content)?.clone();
        match room.kind {
            RoomKind::Boss => {
                let name = room
                    .boss
                    .as_deref()
                    .and_then(|id| theme.boss(id))
                    .map_or("a boss", |b| b.name.as_str());
                self.log.push(format!("{name} awaits."));
                self.status = Status::Fight { game: None };
                return Ok(());
            }
            RoomKind::Treasure => {
                self.log.push("A treasure chamber.".to_owned());
                for hero in 0..self.heroes.len() {
                    let relics = self.relic_offer(content, node, hero, "treasure", 3, None);
                    self.push_offer(hero, Offer::Relics { relics });
                }
            }
            RoomKind::Shop => {
                self.log
                    .push("A goblin trader opens their pack.".to_owned());
                for hero in 0..self.heroes.len() {
                    let mut items: Vec<ShopItem> = self
                        .card_offer(&theme, node, hero, "shop", 3)
                        .into_iter()
                        .map(|name| ShopItem::Card { name, sold: false })
                        .collect();
                    for id in self.relic_offer(content, node, hero, "shop", 2, None) {
                        let price = content.cards.get(&id).map_or(60, |c| {
                            if self.content >= RARITY_FROM {
                                crate::relics::price(c.rarity)
                            } else {
                                c.price
                            }
                        });
                        items.push(ShopItem::Relic {
                            id,
                            price,
                            sold: false,
                        });
                    }
                    items.push(ShopItem::Draught);
                    // After the draught, so the wares of a stored run keep their places.
                    if let Some(upgrade) = self.upgrade_offer(node, hero, "shop", 1).pop() {
                        items.push(ShopItem::Upgrade {
                            upgrade,
                            sold: false,
                        });
                    }
                    self.push_offer(hero, Offer::Shop { items });
                }
            }
            RoomKind::Event => {
                let mut rng = Stream::new(self.seed, "event", u64::from(node), 0);
                // Only the events of the run's content generation, so a stored run draws as it did.
                let pool: Vec<&crate::content::EventSpec> = theme
                    .events
                    .iter()
                    .filter(|e| e.in_generation(self.content))
                    .collect();
                let event = pool
                    .get(rng.below(pool.len()))
                    .map(|e| e.id.clone())
                    .ok_or_else(|| refused("the theme has no events"))?;
                if let Some(spec) = theme.event(&event) {
                    self.log.push(format!("{}.", spec.title));
                }
                for hero in 0..self.heroes.len() {
                    self.push_offer(
                        hero,
                        Offer::Event {
                            event: event.clone(),
                        },
                    );
                }
            }
            RoomKind::Campfire => {
                self.log.push("A campfire burns low.".to_owned());
                for hero in 0..self.heroes.len() {
                    let upgrades = self.upgrade_offer(node, hero, "temper", 3);
                    self.push_offer(hero, Offer::Campfire { upgrades });
                }
            }
        }
        self.status = Status::Room;
        Ok(())
    }

    fn push_offer(&mut self, hero: usize, offer: Offer) {
        if let Some(h) = self.heroes.get_mut(hero) {
            h.offers.push(offer);
        }
    }

    /// Cards of the pool in the hero's colours, drawn for one room and purpose.
    fn card_offer(
        &self,
        theme: &Theme,
        node: u8,
        hero: usize,
        label: &str,
        count: usize,
    ) -> Vec<String> {
        let Some(h) = self.heroes.get(hero) else {
            return Vec::new();
        };
        let colors = h.deck.as_ref().map_or("", |d| d.colors.as_str());
        let fits: Vec<String> = theme
            .pool
            .iter()
            .filter(|(name, cols)| {
                cols.chars().all(|c| colors.contains(c)) && !h.satchel.contains(name)
            })
            .map(|(name, _)| name.clone())
            .collect();
        let salt = u64::try_from(h.offers.len() + h.satchel.len()).unwrap_or(0);
        let mut rng = Stream::new(self.seed, label, u64::from(node) * 16 + salt, hero as u64);
        rng.pick(&fits, count)
    }

    /// Upgrades that fit the hero's commanders, drawn for one room and purpose.
    fn upgrade_offer(&self, node: u8, hero: usize, label: &str, count: usize) -> Vec<Upgrade> {
        let Some(h) = self.heroes.get(hero) else {
            return Vec::new();
        };
        let commanders: &[CommanderFacts] =
            h.deck.as_ref().map_or(&[], |d| d.commanders.as_slice());
        let fits = fitting(commanders, &h.upgrades);
        let salt = u64::try_from(h.upgrades.len()).unwrap_or(0);
        let mut rng = Stream::new(
            self.seed,
            &format!("upgrade-{label}"),
            u64::from(node) * 16 + salt,
            hero as u64,
        );
        rng.pick(&fits, count)
    }

    /// Relics the hero lacks, drawn for one room and purpose; the risky ones only where `label` allows them, from the
    /// filter's generation on only those the hero's deck can use, and from the rarity generation on by the rarity of
    /// each slot: all of `rarity` when given, else as `relic_slots` lays them out.
    fn relic_offer(
        &self,
        content: &Content,
        node: u8,
        hero: usize,
        label: &str,
        count: usize,
        rarity: Option<Rarity>,
    ) -> Vec<String> {
        let Some(h) = self.heroes.get(hero) else {
            return Vec::new();
        };
        let shop = label == "shop";
        let rarities = self.content >= RARITY_FROM;
        let mut fits: Vec<&CustomCard> = content
            .relics()
            .into_iter()
            .filter(|c| c.since <= self.content)
            .filter(|c| {
                // The shop never sells a risky relic: priced 0 before the rarity generation.
                let sold = if rarities {
                    c.role != "Risky"
                } else {
                    c.price > 0
                };
                // From its generation on a relic drops again: owning it never lowers its chance, and copies stack.
                (self.content >= RELICS_AGAIN_FROM || !h.relics.contains(&c.id)) && (!shop || sold)
            })
            .collect();
        // From the filter's generation on, only relics the deck (and its Satchel) can use.
        if self.content >= FILTER_FROM
            && let Some(makeup) = h.deck.as_ref().and_then(|d| d.makeup.as_ref())
            && let Ok(theme) = self.theme(content)
        {
            let with = if self.content >= THEMES_FROM {
                // A Satchel of several themes: each card counts by the first pool that names it, the floor's first.
                let mut with = makeup.clone();
                let mut left = h.satchel.clone();
                let others = content.themes.iter().filter(|t| t.id != theme.id);
                for pool in std::iter::once(theme).chain(others) {
                    let (named, rest): (Vec<String>, Vec<String>) = left
                        .into_iter()
                        .partition(|card| pool.pool.iter().any(|(name, _)| name == card));
                    with = with.with_satchel(&named, pool);
                    left = rest;
                }
                with
            } else {
                makeup.with_satchel(&h.satchel, theme)
            };
            fits = offerable(fits, &with);
        }
        let salt = u64::try_from(h.offers.len() + h.relics.len()).unwrap_or(0);
        let mut rng = Stream::new(self.seed, label, u64::from(node) * 16 + salt, hero as u64);
        if self.content >= RARITY_FROM {
            let slots = match rarity {
                Some(tier) => vec![tier; count],
                None => self.relic_slots(node, label, count, &mut rng),
            };
            // A Mythic relic comes only where a slot asks for one.
            if !slots.contains(&Rarity::Mythic) {
                fits.retain(|c| c.rarity != Rarity::Mythic);
            }
            return crate::relics::draw(&fits, &slots, &mut rng);
        }
        let fits: Vec<String> = fits.into_iter().map(|c| c.id.clone()).collect();
        rng.pick(&fits, count)
    }

    /// The rarity of each slot of a relic offer of the rarity generation: the shop one Common and one Rare; loot and
    /// treasure Common slots that turn Rare by chance, more often on later floors and at a treasure, and the loot of the
    /// last lieutenant a Mythic first slot.
    fn relic_slots(&self, node: u8, label: &str, count: usize, rng: &mut Stream) -> Vec<Rarity> {
        if label == "shop" {
            return [Rarity::Common, Rarity::Rare]
                .into_iter()
                .cycle()
                .take(count)
                .collect();
        }
        let rank = self.boss_rank(node);
        let (rank, bonus) = if label == "treasure" {
            (rank.saturating_sub(1), TREASURE_RARE_BONUS)
        } else {
            (rank, 0)
        };
        let odds = RARE_ODDS[rank.min(RARE_ODDS.len() - 1)] + bonus;
        let mut slots: Vec<Rarity> = (0..count)
            .map(|_| {
                if rng.chance(odds) {
                    Rarity::Rare
                } else {
                    Rarity::Common
                }
            })
            .collect();
        let bosses = self.map.nodes.iter().filter(|n| n.boss.is_some()).count();
        if label == "loot"
            && self.map.node(node).is_some_and(|n| n.boss.is_some())
            && self.boss_rank(node) + 2 == bosses
            && let Some(first) = slots.first_mut()
        {
            *first = Rarity::Mythic;
        }
        slots
    }

    /// Keeps the fight of `game` in the room the party is in for the run's fight log.
    fn record_fight(&mut self, game: String, won: bool) {
        if let Some(room) = self.at {
            let boss = self.map.node(room).and_then(|n| n.boss.clone());
            self.fights.push(FoughtFight {
                game,
                room,
                floor: self.floor,
                boss,
                won,
            });
        }
    }

    /// A lost fight costs a heart: with none left the run is lost, else every hero regroups and the fight is made
    /// again, at full life against the first boss and at half life against a later one. `how` is the party's part
    /// of the chronicle line.
    fn fight_lost(&mut self, content: &Content, how: &str) {
        self.hearts = self.hearts.saturating_sub(1);
        if self.hearts == 0 {
            self.status = Status::Defeat;
            self.log
                .push("The party fell. The Warrens keep their dead.".to_owned());
            return;
        }
        // The party lost only once every hero fell: all regroup, whole before the first boss and at half life later.
        let first = self.at.is_none_or(|node| self.boss_rank(node) == 0);
        for h in &mut self.heroes {
            let max = h.max_life(content);
            h.life = if first { max } else { max / 2 };
        }
        self.log.push(format!(
            "The party {how} and regroups ({} hearts left).",
            self.hearts
        ));
        self.status = Status::Fight { game: None };
    }

    fn fight_ended(
        &mut self,
        content: &Content,
        won: bool,
        lives: &[i32],
        spirits: &[u8],
    ) -> Result<(), CampaignError> {
        let node = self.at.ok_or_else(|| refused("no room"))?;
        if !won {
            self.fight_lost(content, "was beaten back");
            return Ok(());
        }
        self.cleared.push(node);
        self.results.clear();
        let final_room = self.map.choices(Some(node)).is_empty();
        for (at, h) in self.heroes.iter_mut().enumerate() {
            let life = lives.get(at).copied().unwrap_or(h.life);
            let spirit = spirits.contains(&u8::try_from(at).unwrap_or(u8::MAX)) || life <= 0;
            h.life = if spirit { SPIRIT_RETURN } else { life };
            h.heal(content, HEAL_AFTER_BOSS);
        }
        self.log.push("The boss falls!".to_owned());
        if final_room {
            self.floors_cleared = self.floors_cleared.saturating_add(1);
            if self.content >= ENDLESS_FROM {
                self.status = Status::FloorCleared;
                self.log.push(format!(
                    "Floor {} is cleared. Bank the win or descend?",
                    self.floor
                ));
                return Ok(());
            }
            self.status = Status::Victory;
            self.log
                .push("The Warren King is dead. The party wins the delve!".to_owned());
            return Ok(());
        }
        self.loot(content, node, "loot")?;
        self.status = Status::Room;
        Ok(())
    }

    // One arm for each kind of offer and answer.
    #[allow(clippy::too_many_lines)]
    fn pick(&mut self, content: &Content, hero: u8, pick: Pick) -> Result<(), CampaignError> {
        let node = self.at.unwrap_or(0);
        let theme = self.theme(content)?.clone();
        let at = usize::from(hero);
        let seed = self.seed;
        let h = self.hero_mut(hero)?;
        let offer = h
            .offers
            .first()
            .cloned()
            .ok_or_else(|| refused("nothing waits for this hero"))?;
        let index = |i: u8, len: usize| {
            let i = usize::from(i);
            if i < len {
                Ok(i)
            } else {
                Err(refused("no such choice"))
            }
        };
        let mut follow: Vec<Offer> = Vec::new();
        // The rarity of a follow-up relic pick, by its place among the follow-ups.
        let mut follow_rarity: BTreeMap<usize, Rarity> = BTreeMap::new();
        let mut done = true;
        // What the answer did, for the chronicle and the result panel.
        let mut said = String::new();
        let mut outcomes: Vec<Outcome> = Vec::new();
        let mut rolls: Vec<GambleRoll> = Vec::new();
        match (offer, pick) {
            (Offer::Cards { cards }, Pick::Take(i)) => {
                let card = cards
                    .get(index(i, cards.len())?)
                    .cloned()
                    .unwrap_or_default();
                said = format!("took the card {card}");
                h.satchel.push(card);
            }
            (Offer::Cards { .. }, Pick::Skip) => {
                h.gold += SKIP_GOLD;
                said.push_str("skipped the card pick");
                outcomes.push(Outcome::Gold(i32::try_from(SKIP_GOLD).unwrap_or(0)));
            }
            (Offer::Relics { relics }, Pick::Take(i)) => {
                let relic = relics
                    .get(index(i, relics.len())?)
                    .cloned()
                    .unwrap_or_default();
                said = format!("took the relic {}", relic_name(content, &relic));
                h.relics.push(relic);
                let max = h.max_life(content);
                let bonus = content
                    .cards
                    .get(h.relics.last().map_or("", String::as_str))
                    .map_or(0, |c| c.life);
                h.life = (h.life + bonus).min(max);
            }
            (Offer::Relics { .. }, Pick::Skip) => said.push_str("took no relic"),
            (Offer::Shop { .. }, Pick::Skip) => said.push_str("left the shop"),
            (Offer::Shop { mut items }, Pick::Take(i)) => {
                let i = index(i, items.len())?;
                let price = match items.get(i) {
                    Some(ShopItem::Card { sold: false, .. }) => CARD_PRICE,
                    Some(ShopItem::Relic {
                        price, sold: false, ..
                    }) => *price,
                    Some(ShopItem::Draught) => DRAUGHT_PRICE,
                    Some(ShopItem::Upgrade { sold: false, .. }) => UPGRADE_PRICE,
                    _ => return Err(refused("sold out")),
                };
                if h.gold < price {
                    return Err(refused("not enough gold"));
                }
                h.gold -= price;
                outcomes.push(Outcome::Gold(-i32::try_from(price).unwrap_or(0)));
                match items.get_mut(i) {
                    Some(ShopItem::Card { name, sold }) => {
                        *sold = true;
                        said = format!("bought the card {name}");
                        h.satchel.push(name.clone());
                    }
                    Some(ShopItem::Relic { id, sold, .. }) => {
                        *sold = true;
                        said = format!("bought the relic {}", relic_name(content, id));
                        h.relics.push(id.clone());
                    }
                    Some(ShopItem::Draught) => {
                        said.push_str("bought a healing draught");
                        let before = h.life;
                        h.heal(content, DRAUGHT);
                        outcomes.push(Outcome::Life(h.life - before));
                    }
                    Some(ShopItem::Upgrade { upgrade, sold }) => {
                        *sold = true;
                        said = format!("bought the upgrade: {}", upgrade.words());
                        h.upgrades.push(upgrade.clone());
                    }
                    None => {}
                }
                if let Some(first) = h.offers.first_mut() {
                    *first = Offer::Shop { items };
                }
                done = false;
            }
            (Offer::Campfire { .. }, Pick::Rest) => {
                let before = h.life;
                h.heal(content, REST);
                said.push_str("rested at the fire");
                outcomes.push(Outcome::Life(h.life - before));
            }
            (Offer::Campfire { .. }, Pick::Reforge) => {
                // The card pick follows at once.
                done = true;
                said.push_str("reforged at the fire");
                outcomes.push(Outcome::CardPick);
                follow.push(Offer::Cards { cards: Vec::new() });
            }
            (Offer::Campfire { upgrades }, Pick::Take(i)) => {
                let upgrade = upgrades
                    .get(index(i, upgrades.len())?)
                    .cloned()
                    .ok_or_else(|| refused("no such choice"))?;
                said = format!("tempered at the fire: {}", upgrade.words());
                h.upgrades.push(upgrade);
            }
            (Offer::Duplicate, Pick::Duplicate(card)) => {
                if card.trim().is_empty() {
                    return Err(refused("name the card to copy"));
                }
                said = format!("copied {card} into the Satchel");
                h.satchel.push(card);
            }
            (Offer::Duplicate, Pick::Skip) => said.push_str("left the pool"),
            (Offer::Event { event }, Pick::Take(i)) => {
                let spec = theme
                    .event(&event)
                    .ok_or_else(|| CampaignError::Unknown(format!("event {event}")))?;
                let option = spec
                    .options
                    .get(index(i, spec.options.len())?)
                    .cloned()
                    .ok_or_else(|| refused("no such choice"))?;
                let mut rng = Stream::new(seed, "gamble", u64::from(node), at as u64);
                let mut gains = Vec::new();
                resolve(&option.outcomes, &mut rng, &mut gains, &mut rolls);
                said = format!("chose “{}”", option.label);
                for gain in gains {
                    match gain {
                        Outcome::Gold(n) => {
                            let before = h.gold;
                            h.gold = u32::try_from((i64::from(h.gold) + i64::from(n)).max(0))
                                .unwrap_or(0);
                            let moved = i64::from(h.gold) - i64::from(before);
                            outcomes.push(Outcome::Gold(i32::try_from(moved).unwrap_or(0)));
                        }
                        Outcome::Life(n) => {
                            let before = h.life;
                            h.heal(content, n);
                            outcomes.push(Outcome::Life(h.life - before));
                        }
                        Outcome::Relic => {
                            follow.push(Offer::Relics { relics: Vec::new() });
                            outcomes.push(Outcome::Relic);
                        }
                        Outcome::RelicOf(rarity) => {
                            follow.push(Offer::Relics { relics: Vec::new() });
                            follow_rarity.insert(follow.len() - 1, rarity);
                            outcomes.push(Outcome::RelicOf(rarity));
                        }
                        Outcome::CardPick => {
                            follow.push(Offer::Cards { cards: Vec::new() });
                            outcomes.push(Outcome::CardPick);
                        }
                        Outcome::Duplicate => {
                            follow.push(Offer::Duplicate);
                            outcomes.push(Outcome::Duplicate);
                        }
                        Outcome::MaxLife(n) => {
                            h.life_cap += n;
                            h.life = h.life.min(h.max_life(content)).max(1);
                            outcomes.push(Outcome::MaxLife(n));
                        }
                        Outcome::LoseCard => {
                            follow.push(Offer::Discard);
                            outcomes.push(Outcome::LoseCard);
                        }
                        Outcome::LoseRelic => {
                            follow.push(Offer::Surrender {
                                relics: h.relics.clone(),
                            });
                            outcomes.push(Outcome::LoseRelic);
                        }
                        Outcome::Bane(bane) => {
                            let first = h
                                .deck
                                .as_ref()
                                .and_then(|d| d.commanders.first())
                                .map(|c| c.name.clone());
                            if let Some(commander) = first {
                                h.upgrades.push(Upgrade {
                                    commander,
                                    boon: bane.boon(),
                                });
                                outcomes.push(Outcome::Bane(bane));
                            }
                        }
                        Outcome::Gamble { .. } => {}
                    }
                }
            }
            (Offer::Discard, Pick::Discard(card)) => {
                if card.trim().is_empty() {
                    return Err(refused("name the card to give up"));
                }
                // A Satchel card goes first; else a copy leaves the deck for the rest of the run.
                if let Some(at) = h.satchel.iter().position(|c| *c == card) {
                    h.satchel.remove(at);
                } else if h.deck_holds(&card) {
                    h.lost.push(card.clone());
                } else {
                    return Err(refused("that card is not in your deck or Satchel"));
                }
                said = format!("gave up the card {card}");
            }
            (Offer::Surrender { relics }, Pick::Take(i)) => {
                let relic = relics
                    .get(index(i, relics.len())?)
                    .cloned()
                    .unwrap_or_default();
                let held = h
                    .relics
                    .iter()
                    .position(|r| *r == relic)
                    .ok_or_else(|| refused("that relic is not yours"))?;
                h.relics.remove(held);
                h.life = h.life.min(h.max_life(content)).max(1);
                said = format!("gave up the relic {}", relic_name(content, &relic));
            }
            _ => return Err(refused("that answer does not fit the offer")),
        }
        let name = h.name.clone();
        if done {
            h.offers.remove(0);
        }
        self.log.push(format!("{name} {said}."));
        self.results.push(Resolved {
            hero,
            what: said,
            outcomes,
            rolls,
        });
        // Follow-up offers are drawn now that the hero's state is known.
        for (k, offer) in follow.into_iter().enumerate() {
            let label = format!("follow{k}");
            let offer = match offer {
                Offer::Relics { .. } => {
                    let rarity = follow_rarity.get(&k).copied();
                    // A relic of a named rarity is a pick of three; an older event's relic a pick of one.
                    let count = if rarity.is_some() { 3 } else { 1 };
                    Offer::Relics {
                        relics: self.relic_offer(content, node, at, &label, count, rarity),
                    }
                }
                Offer::Cards { .. } => Offer::Cards {
                    cards: self.card_offer(&theme, node, at, &label, 3),
                },
                other => other,
            };
            if let Some(h) = self.heroes.get_mut(at) {
                h.offers.insert(k, offer);
            }
        }
        Ok(())
    }

    /// The place of the boss room `node` on the ladder: 0 for the first boss.
    pub fn boss_rank(&self, node: u8) -> usize {
        let row = self.map.node(node).map_or(0, |n| n.row);
        self.map
            .nodes
            .iter()
            .filter(|n| n.boss.is_some() && n.row < row)
            .count()
    }

    /// Each boss of the ladder by id with the life it starts its fight with against this party.
    pub fn boss_lives(&self, content: &Content) -> Vec<(String, i32)> {
        let Ok(theme) = self.theme(content) else {
            return Vec::new();
        };
        let life = crate::scale::Scale::boss_life(self.heroes.len());
        self.map
            .nodes
            .iter()
            .filter_map(|n| Some((theme.boss(n.boss.as_deref()?)?.id.clone(), life)))
            .collect()
    }

    /// The plan of the fight the party is in, if it is in one: the heroes against the boss of its room at the depth of
    /// the bosses the party defeated.
    pub fn fight_plan(&self, content: &Content) -> Result<Option<FightPlan>, CampaignError> {
        if !matches!(self.status, Status::Fight { .. }) {
            return Ok(None);
        }
        let node = self.at.ok_or_else(|| refused("no room"))?;
        let theme = self.theme(content)?;
        let boss = self
            .map
            .node(node)
            .and_then(|n| n.boss.as_deref())
            .and_then(|id| theme.boss(id))
            .ok_or_else(|| refused("no boss in this room"))?;
        let heroes: Vec<HeroSeat> = self
            .heroes
            .iter()
            .enumerate()
            .map(|(at, h)| HeroSeat {
                hero: u8::try_from(at).unwrap_or(0),
                life: h.life.max(1),
                hand: HAND
                    + h.relics
                        .iter()
                        .filter_map(|id| content.cards.get(id))
                        .map(|c| c.hand)
                        .sum::<u32>(),
                relics: h.relics.clone(),
                upgrades: h.upgrades.clone(),
            })
            .collect();
        let boss = self.boss_seat(content, boss, node, (heroes.len(), self.defeated()))?;
        Ok(Some(FightPlan {
            node,
            heroes,
            boss,
            run: None,
        }))
    }

    /// A bot hero's answer to `event`: in an older run the first option it can afford, else the best trade it can make
    /// (`worth`), the first of equals.
    fn bot_event_pick(&self, content: &Content, h: &Hero, event: &str) -> Pick {
        self.theme(content)
            .ok()
            .and_then(|theme| theme.event(event))
            .and_then(|spec| {
                if self.content < RARITY_FROM {
                    return spec
                        .options
                        .iter()
                        .position(|o| o.needs().unmet(h.gold, h.life).is_none());
                }
                let mut best: Option<(usize, i32)> = None;
                for (at, option) in spec.options.iter().enumerate() {
                    if let Some(worth) = worth(content, h, option)
                        && best.is_none_or(|(_, b)| worth > b)
                    {
                        best = Some((at, worth));
                    }
                }
                best.map(|(at, _)| at)
            })
            .and_then(|i| u8::try_from(i).ok())
            .map_or(Pick::Take(0), Pick::Take)
    }

    /// The events bot allies send now: a vote once every player voted (with the most votes, the torch's on a tie),
    /// and an answer to each offer (the first relic and card, a rest when hurt, the first event option, the newest Satchel card at the Mirror Pool).
    pub fn bot_moves(&self, content: &Content) -> Vec<RunEvent> {
        let mut out = Vec::new();
        let bots = self
            .heroes
            .iter()
            .enumerate()
            .filter(|(_, h)| h.bot.is_some())
            .map(|(at, h)| (u8::try_from(at).unwrap_or(0), h));
        match self.status {
            Status::Map => {
                let players_done = self
                    .heroes
                    .iter()
                    .enumerate()
                    .filter(|(_, h)| h.user.is_some())
                    .all(|(at, _)| self.votes.contains_key(&u8::try_from(at).unwrap_or(0)));
                if !players_done {
                    return out;
                }
                let choice = if self.votes.is_empty() {
                    self.map.choices(self.at).first().copied()
                } else {
                    Some(self.tally())
                };
                if let Some(node) = choice {
                    for (hero, _) in bots {
                        if !self.votes.contains_key(&hero) {
                            out.push(RunEvent::Voted { hero, node });
                        }
                    }
                }
            }
            Status::FloorCleared => {
                let players_done = self
                    .heroes
                    .iter()
                    .enumerate()
                    .filter(|(_, h)| h.user.is_some())
                    .all(|(at, _)| {
                        self.floor_votes
                            .contains_key(&u8::try_from(at).unwrap_or(0))
                    });
                if !players_done {
                    return out;
                }
                let descend = self.bots_descend(content);
                for (hero, _) in bots {
                    if !self.floor_votes.contains_key(&hero) {
                        out.push(RunEvent::FloorVoted { hero, descend });
                    }
                }
            }
            Status::Room => {
                for (hero, h) in bots {
                    let pick = match h.offers.first() {
                        Some(Offer::Cards { cards }) if !cards.is_empty() => Pick::Take(0),
                        Some(Offer::Relics { relics }) if !relics.is_empty() => Pick::Take(0),
                        Some(Offer::Duplicate) => h
                            .satchel
                            .last()
                            .map_or(Pick::Skip, |card| Pick::Duplicate(card.clone())),
                        Some(Offer::Campfire { .. }) => {
                            if h.life < h.max_life(content) - 10 {
                                Pick::Rest
                            } else {
                                Pick::Reforge
                            }
                        }
                        Some(Offer::Event { event }) => self.bot_event_pick(content, h, event),
                        Some(Offer::Discard) => h
                            .satchel
                            .last()
                            .map_or(Pick::Skip, |card| Pick::Discard(card.clone())),
                        // The least rare relic, the oldest of those.
                        Some(Offer::Surrender { relics }) => relics
                            .iter()
                            .enumerate()
                            .min_by_key(|(at, id)| {
                                (content.cards.get(id.as_str()).map(|c| c.rarity), *at)
                            })
                            .and_then(|(at, _)| u8::try_from(at).ok())
                            .map_or(Pick::Skip, Pick::Take),
                        Some(Offer::Shop { items }) => items
                            .iter()
                            .position(|item| match item {
                                ShopItem::Relic {
                                    price, sold: false, ..
                                } => h.gold >= *price,
                                _ => false,
                            })
                            .and_then(|i| u8::try_from(i).ok())
                            .map_or(Pick::Skip, Pick::Take),
                        Some(_) => Pick::Skip,
                        None => continue,
                    };
                    out.push(RunEvent::Picked { hero, pick });
                }
            }
            _ => {}
        }
        out
    }
}

/// What an event option is worth to a bot hero, in points (a Common relic is 8); `None` when the hero cannot take it, or
/// would have to name a deck card it cannot see (a card to give up with an empty Satchel). Life counts double when the
/// hero is below half their cap, and a cut to the cap weighs double when the hero is hurt or the cap already low.
fn worth(content: &Content, h: &Hero, option: &EventOption) -> Option<i32> {
    if h.lacks(content, &option.needs()).is_some() {
        return None;
    }
    let cap = h.max_life(content);
    let hurt = h.life * 2 < cap;
    let mut points = 0;
    for outcome in &option.outcomes {
        points += match outcome {
            Outcome::Gold(n) => n / 5,
            Outcome::Life(n) if *n > 0 => {
                let healed = (*n).min(cap - h.life);
                if hurt { healed } else { healed / 2 }
            }
            Outcome::Life(n) => {
                if hurt {
                    n * 2
                } else {
                    *n
                }
            }
            Outcome::MaxLife(n) => {
                if hurt || cap <= 30 {
                    n * 3
                } else {
                    n * 3 / 2
                }
            }
            Outcome::Relic | Outcome::RelicOf(Rarity::Common) => 8,
            Outcome::RelicOf(Rarity::Rare) => 15,
            Outcome::RelicOf(Rarity::Mythic) => 28,
            Outcome::CardPick => 6,
            Outcome::Duplicate => 5,
            Outcome::LoseCard if h.satchel.is_empty() => return None,
            Outcome::LoseCard => -6,
            Outcome::LoseRelic => -12,
            Outcome::Bane(crate::upgrade::Bane::Tax(n)) => -8 * i32::try_from(*n).unwrap_or(1),
            Outcome::Bane(crate::upgrade::Bane::Shrink(n)) => -10 * n,
            Outcome::Gamble { .. } => 0,
        };
    }
    Some(points)
}

/// The plain outcomes of `outcomes`, gambles decided by `rng`.
fn resolve(
    outcomes: &[Outcome],
    rng: &mut Stream,
    out: &mut Vec<Outcome>,
    rolls: &mut Vec<GambleRoll>,
) {
    for outcome in outcomes {
        match outcome {
            Outcome::Gamble { odds, win, lose } => {
                let won = rng.chance(*odds);
                rolls.push(GambleRoll { odds: *odds, won });
                resolve(if won { win } else { lose }, rng, out, rolls);
            }
            other => out.push(other.clone()),
        }
    }
}

/// A relic's name in the content, its id when the content lacks it.
fn relic_name<'a>(content: &'a Content, id: &'a str) -> &'a str {
    content.cards.get(id).map_or(id, |c| c.name.as_str())
}
