//! What a seat may read of a game: the shared zones, its own hand and every player's counts, never another seat's hand
//! or any library's order.

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::characteristics::{Characteristics, characteristics, controller_of, sidelined};
use mtg_engine::keywords::keywords_of;
use mtg_engine::object::GameObject;
use mtg_engine::state::{GameResult, GameState};
use mtg_engine::turn::TurnState;
use mtg_engine::zones::Zone;

/// One seat's reading of a game state: every accessor answers only what that seat may know.
#[derive(Debug, Clone, Copy)]
pub struct Seen<'a> {
    state: &'a GameState,
    seat: PlayerId,
}

/// What a seat reads of one creature or permanent: its controller, characteristics and combat keywords.
// Independent facts about one permanent.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct Body {
    /// The object.
    pub id: ObjectId,
    /// Who controls it.
    pub controller: PlayerId,
    /// Its characteristics now.
    pub chars: Characteristics,
    /// Whether it is tapped.
    pub tapped: bool,
    /// Its mana value as the seat sees it (0 face down).
    pub mana_value: u32,
    /// How many rules its card prints, a rough measure of what it does beyond its body.
    pub rules: usize,
    /// Whether a printed ability of it adds mana.
    pub makes_mana: bool,
    /// Its loyalty counters, for a planeswalker.
    pub loyalty: u32,
    /// The keywords that matter in combat.
    pub keys: Keys,
    /// Whether effects keep it from attacking.
    pub cant_attack: bool,
    /// Whether effects keep it from blocking.
    pub cant_block: bool,
}

/// The combat keywords of a permanent.
// Independent facts read off one keyword list.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Keys {
    /// Flying.
    pub flying: bool,
    /// Reach.
    pub reach: bool,
    /// First strike.
    pub first_strike: bool,
    /// Double strike.
    pub double_strike: bool,
    /// Deathtouch.
    pub deathtouch: bool,
    /// Lifelink.
    pub lifelink: bool,
    /// Trample.
    pub trample: bool,
    /// Menace.
    pub menace: bool,
    /// Vigilance.
    pub vigilance: bool,
    /// Defender.
    pub defender: bool,
    /// Indestructible.
    pub indestructible: bool,
    /// Hexproof or shroud.
    pub hexproof: bool,
}

impl Keys {
    /// The combat keywords among `keywords`.
    fn of(keywords: &[&Keyword]) -> Keys {
        let mut keys = Keys::default();
        for keyword in keywords {
            match keyword {
                Keyword::Flying => keys.flying = true,
                Keyword::Reach => keys.reach = true,
                Keyword::FirstStrike => keys.first_strike = true,
                Keyword::DoubleStrike => keys.double_strike = true,
                Keyword::Deathtouch => keys.deathtouch = true,
                Keyword::Lifelink => keys.lifelink = true,
                Keyword::Trample => keys.trample = true,
                Keyword::Menace => keys.menace = true,
                Keyword::Vigilance => keys.vigilance = true,
                Keyword::Defender => keys.defender = true,
                Keyword::Indestructible => keys.indestructible = true,
                Keyword::Hexproof | Keyword::Shroud | Keyword::HexproofFrom(_) => {
                    keys.hexproof = true;
                }
                _ => {}
            }
        }
        keys
    }
}

impl Body {
    /// Its power, at least 0; 0 for a permanent without one.
    pub fn power(&self) -> i32 {
        self.chars.power.unwrap_or(0).max(0)
    }

    /// Its toughness, at least 0.
    pub fn toughness(&self) -> i32 {
        self.chars.toughness.unwrap_or(0).max(0)
    }

    /// Whether it is a creature now.
    pub fn is_creature(&self) -> bool {
        self.chars.is_creature()
    }

    /// Whether it is a land.
    pub fn is_land(&self) -> bool {
        self.chars.types.contains(CardType::Land)
    }

    /// Whether `blocker` may block it as far as flying goes.
    pub fn blockable_by(&self, blocker: &Body) -> bool {
        !self.keys.flying || blocker.keys.flying || blocker.keys.reach
    }
}

impl<'a> Seen<'a> {
    /// `seat`'s reading of `state`.
    pub const fn new(state: &'a GameState, seat: PlayerId) -> Seen<'a> {
        Seen { state, seat }
    }

    /// The seat reading.
    pub const fn seat(&self) -> PlayerId {
        self.seat
    }

    /// The turn: its number, active player and step.
    pub fn turn(&self) -> &TurnState {
        self.state.turn()
    }

    /// The stack, top last.
    pub fn stack(&self) -> &[ObjectId] {
        self.state.stack()
    }

    /// Whether the game is over, and whether the seat's team won it.
    pub fn over(&self) -> Option<bool> {
        Some(match self.state.result()? {
            GameResult::Winner(winner) => self.state.are_teammates(winner, self.seat),
            GameResult::Team(team) => self.state.team_of(self.seat) == team,
            GameResult::Draw => false,
        })
    }

    /// Whether `player` plays on the seat's team (the seat included).
    pub fn is_teammate(&self, player: PlayerId) -> bool {
        self.state.are_teammates(self.seat, player)
    }

    /// Whether `a` and `b` play on one team.
    pub fn same_team(&self, a: PlayerId, b: PlayerId) -> bool {
        self.state.are_teammates(a, b)
    }

    /// The team number of `player`.
    pub fn team_of(&self, player: PlayerId) -> u32 {
        self.state.team_of(player)
    }

    /// Whether teams share a life total.
    pub fn shares_life(&self) -> bool {
        self.state.shares_turns()
    }

    /// Every player still in the game.
    pub fn alive(&self) -> Vec<PlayerId> {
        self.state.alive_players().collect()
    }

    /// Whether `player` is still in the game.
    pub fn is_alive(&self, player: PlayerId) -> bool {
        self.state.is_alive(player)
    }

    /// The seat's opponents still in the game, in turn order from the active player.
    pub fn opponents(&self) -> Vec<PlayerId> {
        self.state.opponents_of(self.seat)
    }

    /// Whether it is the turn of `player`'s team.
    pub fn is_active(&self, player: PlayerId) -> bool {
        self.state.is_active(player)
    }

    /// The life total of `player`.
    pub fn life(&self, player: PlayerId) -> i32 {
        self.state
            .player(player)
            .map_or(0, mtg_engine::player::Player::life)
    }

    /// The poison counters of `player`.
    pub fn poison(&self, player: PlayerId) -> u32 {
        self.state
            .player(player)
            .map_or(0, |seat| seat.counters_of(&CounterKind::Poison))
    }

    /// How many cards `player` holds (a count every seat sees).
    pub fn hand_size(&self, player: PlayerId) -> usize {
        self.state.zone(Zone::Hand(player)).len()
    }

    /// How many cards are left in `player`'s library (a count every seat sees).
    pub fn library_size(&self, player: PlayerId) -> usize {
        self.state.zone(Zone::Library(player)).len()
    }

    /// The seat's own hand.
    pub fn own_hand(&self) -> &[ObjectId] {
        self.state.zone(Zone::Hand(self.seat))
    }

    /// Whether the seat may identify `id`: it is on the battlefield, the stack, in a graveyard, in exile face up, in a
    /// command zone or in the seat's own hand.
    pub fn visible(&self, id: ObjectId) -> bool {
        self.state
            .object(id)
            .is_some_and(|object| match object.zone() {
                Zone::Battlefield | Zone::Stack | Zone::Graveyard(_) | Zone::Command(_) => true,
                Zone::Exile => object.face_down().is_none(),
                Zone::Hand(owner) => owner == self.seat,
                Zone::Library(_) | Zone::PhasedOut => false,
            })
    }

    /// The object `id` when the seat may identify it.
    fn object(&self, id: ObjectId) -> Option<&'a GameObject> {
        self.visible(id).then(|| self.state.object(id)).flatten()
    }

    /// Who controls `id`, when the seat may identify it.
    pub fn controller(&self, id: ObjectId) -> Option<PlayerId> {
        self.object(id)?;
        controller_of(self.state, id)
    }

    /// The zone of `id`, when the seat may identify it.
    pub fn zone(&self, id: ObjectId) -> Option<Zone> {
        self.object(id).map(GameObject::zone)
    }

    /// The characteristics of `id`, when the seat may identify it.
    pub fn chars(&self, id: ObjectId) -> Option<Characteristics> {
        self.object(id)?;
        characteristics(self.state, id)
    }

    /// The mana value of `id` as the seat sees it: 0 for a face-down permanent.
    pub fn mana_value(&self, id: ObjectId) -> u32 {
        self.object(id)
            .filter(|object| object.face_down().is_none())
            .map_or(0, GameObject::mana_value)
    }

    /// The permanents on the battlefield.
    pub fn battlefield(&self) -> &[ObjectId] {
        self.state.zone(Zone::Battlefield)
    }

    /// What the seat reads of the permanent `id`.
    pub fn body(&self, id: ObjectId) -> Option<Body> {
        let object = self.object(id)?;
        let chars = characteristics(self.state, id)?;
        let controller = controller_of(self.state, id)?;
        let hidden = object.face_down().is_some();
        let card = object.card();
        let (rules, makes_mana) = if hidden {
            (0, false)
        } else {
            (
                card.rules.len(),
                // A plain mana ability is an `AbilityDef`; one with more to it is a rule.
                !card.abilities.is_empty() || card.rules.iter().any(|rule| rule.is_mana_ability()),
            )
        };
        let loyalty = object
            .permanent()
            .map_or(0, |permanent| permanent.counters_of(&CounterKind::Loyalty));
        let (cant_attack, cant_block) = sidelined(self.state, id);
        Some(Body {
            id,
            controller,
            tapped: object
                .permanent()
                .is_some_and(mtg_engine::object::PermanentState::tapped),
            mana_value: if hidden { 0 } else { object.mana_value() },
            rules,
            makes_mana,
            loyalty,
            keys: Keys::of(&keywords_of(self.state, id)),
            cant_attack,
            cant_block,
            chars,
        })
    }

    /// Every permanent on the battlefield as the seat reads it.
    pub fn bodies(&self) -> Vec<Body> {
        self.battlefield()
            .iter()
            .filter_map(|id| self.body(*id))
            .collect()
    }
}
