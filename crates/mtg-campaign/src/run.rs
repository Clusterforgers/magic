//! A run as the fold of its events: the party gathers in the lobby, fights the bosses of the ladder one after another
//! (each a fresh game; life carries over), takes loot after each win and votes for a camp between two bosses. Every
//! random draw comes from the run's seed keyed by room and hero, so folding the same events always gives the same run.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::CampaignError;
use crate::content::{Brain, Content, Outcome, Theme};
use crate::fight::{BossSeat, FightPlan, HAND, HeroSeat, MinionSeat};
use crate::map::{FloorMap, RoomKind, generate};
use crate::rng::Stream;
use crate::scale::Scale;

/// The most heroes a party has.
pub const PARTY: usize = crate::scale::MOST;
/// A hero's life with no relic.
pub const LIFE: i32 = 40;
/// The fights the party may lose and retry.
pub const HEARTS: u8 = 3;
/// Life each hero gets back after a boss falls.
pub const HEAL_AFTER_BOSS: i32 = 10;
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
}

/// A choice waiting for one hero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Offer {
    /// Take one card into the deck (it replaces a basic land) or skip for gold.
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
    /// Rest or reforge.
    Campfire,
    /// One option of an event.
    Event {
        /// The event's id.
        event: String,
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
}

/// A hero's answer to their first offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Pick {
    /// The item, relic, card or option with this index.
    Take(u8),
    /// Nothing (gold for a card pick); leaves a shop.
    Skip,
    /// At a campfire: life.
    Rest,
    /// At a campfire: a card pick.
    Reforge,
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
    /// The cards that joined their deck, each replacing a basic land.
    pub cards: Vec<String>,
    /// The choices waiting for them, first one first.
    pub offers: Vec<Offer>,
}

impl Hero {
    /// Their life cap: the base life plus their relics' bonus.
    pub fn max_life(&self, content: &Content) -> i32 {
        LIFE + self
            .relics
            .iter()
            .filter_map(|id| content.cards.get(id))
            .map(|c| c.life)
            .sum::<i32>()
    }

    fn heal(&mut self, content: &Content, amount: i32) {
        self.life = (self.life + amount).clamp(1, self.max_life(content));
    }
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
    /// The final boss fell.
    Victory,
    /// The party lost a fight with no hearts left.
    Defeat,
    /// The host ended the run.
    Abandoned,
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
    /// The fight's game ended unplayed (an admin ended it, or it expired): the fight waits for a new game.
    FightVoided,
    /// A hero answered their first offer.
    Picked {
        /// Their index.
        hero: u8,
        /// The answer.
        pick: Pick,
    },
    /// The host ended the run.
    Abandoned,
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
    /// The theme's id.
    pub theme: String,
    /// The ladder.
    pub map: FloorMap,
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
            theme: theme.to_owned(),
            map,
            heroes: Vec::new(),
            at: None,
            cleared: Vec::new(),
            votes: BTreeMap::new(),
            hearts: HEARTS,
            status: Status::Lobby,
            log: Vec::new(),
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

    fn theme<'a>(&self, content: &'a Content) -> Result<&'a Theme, CampaignError> {
        content.theme(&self.theme)
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
                    cards: Vec::new(),
                    offers: Vec::new(),
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
            RunEvent::Started => {
                self.lobby()?;
                if !self.heroes.iter().any(|h| h.user.is_some()) {
                    return Err(refused("the party needs a player"));
                }
                if let Some(h) = self.heroes.iter().find(|h| h.deck.is_none()) {
                    return Err(refused(format!("{} has no deck", h.name)));
                }
                self.status = Status::Map;
                self.log
                    .push("The party descends into the Warrens.".to_owned());
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
                if !matches!(self.status, Status::Fight { game: Some(_) }) {
                    return Err(refused("no fight is running"));
                }
                self.fight_ended(content, *won, lives, spirits)?;
            }
            RunEvent::FightVoided => {
                if !matches!(self.status, Status::Fight { game: Some(_) }) {
                    return Err(refused("no fight is running"));
                }
                self.status = Status::Fight { game: None };
            }
            RunEvent::Picked { hero, pick } => {
                if self.status != Status::Room {
                    return Err(refused("nothing to pick now"));
                }
                self.pick(content, *hero, *pick)?;
                if self.heroes.iter().all(|h| h.offers.is_empty()) {
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
        }
        Ok(())
    }

    fn lobby(&self) -> Result<(), CampaignError> {
        if self.status == Status::Lobby {
            Ok(())
        } else {
            Err(refused("the run has started"))
        }
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
                    let relics = self.relic_offer(content, node, hero, "treasure", 3);
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
                    for id in self.relic_offer(content, node, hero, "shop", 2) {
                        let price = content.cards.get(&id).map_or(60, |c| c.price);
                        items.push(ShopItem::Relic {
                            id,
                            price,
                            sold: false,
                        });
                    }
                    items.push(ShopItem::Draught);
                    self.push_offer(hero, Offer::Shop { items });
                }
            }
            RoomKind::Event => {
                let mut rng = Stream::new(self.seed, "event", u64::from(node), 0);
                let event = theme
                    .events
                    .get(rng.below(theme.events.len()))
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
                    self.push_offer(hero, Offer::Campfire);
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
                cols.chars().all(|c| colors.contains(c)) && !h.cards.contains(name)
            })
            .map(|(name, _)| name.clone())
            .collect();
        let salt = u64::try_from(h.offers.len() + h.cards.len()).unwrap_or(0);
        let mut rng = Stream::new(self.seed, label, u64::from(node) * 16 + salt, hero as u64);
        rng.pick(&fits, count)
    }

    /// Relics the hero lacks, drawn for one room and purpose; the risky ones only where `label` allows them.
    fn relic_offer(
        &self,
        content: &Content,
        node: u8,
        hero: usize,
        label: &str,
        count: usize,
    ) -> Vec<String> {
        let Some(h) = self.heroes.get(hero) else {
            return Vec::new();
        };
        let fits: Vec<String> = content
            .relics()
            .into_iter()
            .filter(|c| !h.relics.contains(&c.id) && (label != "shop" || c.price > 0))
            .map(|c| c.id.clone())
            .collect();
        let salt = u64::try_from(h.offers.len() + h.relics.len()).unwrap_or(0);
        let mut rng = Stream::new(self.seed, label, u64::from(node) * 16 + salt, hero as u64);
        rng.pick(&fits, count)
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
            self.hearts = self.hearts.saturating_sub(1);
            if self.hearts == 0 {
                self.status = Status::Defeat;
                self.log
                    .push("The party fell. The Warrens keep their dead.".to_owned());
                return Ok(());
            }
            // The party lost only once every hero fell: all regroup at half their life.
            for h in &mut self.heroes {
                h.life = h.max_life(content) / 2;
            }
            self.log.push(format!(
                "The party was beaten back and regroups ({} hearts left).",
                self.hearts
            ));
            self.status = Status::Fight { game: None };
            return Ok(());
        }
        self.cleared.push(node);
        let final_room = self.map.choices(Some(node)).is_empty();
        for (at, h) in self.heroes.iter_mut().enumerate() {
            let life = lives.get(at).copied().unwrap_or(h.life);
            let spirit = spirits.contains(&u8::try_from(at).unwrap_or(u8::MAX)) || life <= 0;
            h.life = if spirit { SPIRIT_RETURN } else { life };
            h.heal(content, HEAL_AFTER_BOSS);
        }
        self.log.push("The boss falls!".to_owned());
        if final_room {
            self.status = Status::Victory;
            self.log
                .push("The Warren King is dead. The party wins the delve!".to_owned());
            return Ok(());
        }
        let theme = self.theme(content)?.clone();
        for hero in 0..self.heroes.len() {
            if let Some(h) = self.heroes.get_mut(hero) {
                h.gold += 50;
            }
            let relics = self.relic_offer(content, node, hero, "loot", 3);
            self.push_offer(hero, Offer::Relics { relics });
            let cards = self.card_offer(&theme, node, hero, "loot", 3);
            self.push_offer(hero, Offer::Cards { cards });
        }
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
        let mut done = true;
        match (offer, pick) {
            (Offer::Cards { cards }, Pick::Take(i)) => {
                let card = cards
                    .get(index(i, cards.len())?)
                    .cloned()
                    .unwrap_or_default();
                h.cards.push(card);
            }
            (Offer::Cards { .. }, Pick::Skip) => h.gold += SKIP_GOLD,
            (Offer::Relics { relics }, Pick::Take(i)) => {
                let relic = relics
                    .get(index(i, relics.len())?)
                    .cloned()
                    .unwrap_or_default();
                h.relics.push(relic);
                let max = h.max_life(content);
                let bonus = content
                    .cards
                    .get(h.relics.last().map_or("", String::as_str))
                    .map_or(0, |c| c.life);
                h.life = (h.life + bonus).min(max);
            }
            (Offer::Relics { .. } | Offer::Shop { .. }, Pick::Skip) => {}
            (Offer::Shop { mut items }, Pick::Take(i)) => {
                let i = index(i, items.len())?;
                let price = match items.get(i) {
                    Some(ShopItem::Card { sold: false, .. }) => CARD_PRICE,
                    Some(ShopItem::Relic {
                        price, sold: false, ..
                    }) => *price,
                    Some(ShopItem::Draught) => DRAUGHT_PRICE,
                    _ => return Err(refused("sold out")),
                };
                if h.gold < price {
                    return Err(refused("not enough gold"));
                }
                h.gold -= price;
                match items.get_mut(i) {
                    Some(ShopItem::Card { name, sold }) => {
                        *sold = true;
                        h.cards.push(name.clone());
                    }
                    Some(ShopItem::Relic { id, sold, .. }) => {
                        *sold = true;
                        h.relics.push(id.clone());
                    }
                    Some(ShopItem::Draught) => h.heal(content, DRAUGHT),
                    None => {}
                }
                if let Some(first) = h.offers.first_mut() {
                    *first = Offer::Shop { items };
                }
                done = false;
            }
            (Offer::Campfire, Pick::Rest) => h.heal(content, REST),
            (Offer::Campfire, Pick::Reforge) => {
                // The card pick follows at once.
                done = true;
                follow.push(Offer::Cards { cards: Vec::new() });
            }
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
                resolve(&option.outcomes, &mut rng, &mut gains);
                for gain in gains {
                    match gain {
                        Outcome::Gold(n) => {
                            h.gold = u32::try_from((i64::from(h.gold) + i64::from(n)).max(0))
                                .unwrap_or(0);
                        }
                        Outcome::Life(n) => h.heal(content, n),
                        Outcome::Relic => follow.push(Offer::Relics { relics: Vec::new() }),
                        Outcome::CardPick => follow.push(Offer::Cards { cards: Vec::new() }),
                        Outcome::Gamble { .. } => {}
                    }
                }
            }
            _ => return Err(refused("that answer does not fit the offer")),
        }
        if done {
            h.offers.remove(0);
        }
        // Follow-up offers are drawn now that the hero's state is known.
        for (k, offer) in follow.into_iter().enumerate() {
            let label = format!("follow{k}");
            let offer = match offer {
                Offer::Relics { .. } => Offer::Relics {
                    relics: self.relic_offer(content, node, at, &label, 1),
                },
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

    /// The plan of the fight the party is in, if it is in one.
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
            })
            .collect();
        let scale = Scale::new(heroes.len(), self.heat);
        let life = scale.life(boss);
        let schemes = Stream::new(self.seed, "schemes", u64::from(node), 0)
            .pick(&theme.schemes, theme.schemes.len());
        let minions = scale
            .minions(theme, boss)
            .into_iter()
            .map(|minion| {
                let lieutenant = theme
                    .boss(&minion.deck_of)
                    .ok_or_else(|| refused(format!("no boss {}", minion.deck_of)))?;
                Ok(MinionSeat {
                    minion: minion.id.clone(),
                    name: minion.name.clone(),
                    life: scale.minion_life(minion),
                    hand: HAND,
                    brain: scale.brain(lieutenant.brain),
                    deck: lieutenant.deck.clone(),
                    rules: minion.rules.clone(),
                })
            })
            .collect::<Result<Vec<_>, CampaignError>>()?;
        Ok(Some(FightPlan {
            node,
            heroes,
            boss: BossSeat {
                boss: boss.id.clone(),
                name: boss.name.clone(),
                life,
                hand: scale.hand(boss),
                brain: scale.brain(boss.brain),
                deck: boss.deck.clone(),
                rules: scale.rules(boss),
                phases: scale.phases(boss, life),
                schemes,
            },
            minions,
            run: None,
        }))
    }

    /// The events bot allies send now: a vote once every player voted (with the most votes, the torch's on a tie),
    /// and an answer to each offer (the first relic and card, a rest when hurt, the first event option).
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
            Status::Room => {
                for (hero, h) in bots {
                    let pick = match h.offers.first() {
                        Some(Offer::Cards { cards }) if !cards.is_empty() => Pick::Take(0),
                        Some(Offer::Relics { relics }) if !relics.is_empty() => Pick::Take(0),
                        Some(Offer::Campfire) => {
                            if h.life < h.max_life(content) - 10 {
                                Pick::Rest
                            } else {
                                Pick::Reforge
                            }
                        }
                        Some(Offer::Event { .. }) => Pick::Take(0),
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

/// The plain outcomes of `outcomes`, gambles decided by `rng`.
fn resolve(outcomes: &[Outcome], rng: &mut Stream, out: &mut Vec<Outcome>) {
    for outcome in outcomes {
        match outcome {
            Outcome::Gamble { odds, win, lose } => {
                if rng.chance(*odds) {
                    resolve(win, rng, out);
                } else {
                    resolve(lose, rng, out);
                }
            }
            other => out.push(other.clone()),
        }
    }
}
