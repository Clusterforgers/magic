//! Manual actions at random times, for fuzz games: a candidate of every kind drawn from the state and kept only when the
//! engine's own check allows it, so the games exercise the legal actions and the refusals the filter keeps out are tested
//! on their own.

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use mtg_engine::GameState;
use mtg_engine::combat::DamageTarget;
use mtg_engine::manual::{
    CommonToken, CustomToken, ManualAction, ManualToken, ManualZone, check as legal,
};
use mtg_engine::rng::GameRng;
use mtg_engine::zones::Zone;

/// Tries to draw a legal action before giving up for this priority.
const TRIES: usize = 16;

const ZONES: [ManualZone; 7] = [
    ManualZone::Battlefield,
    ManualZone::Hand,
    ManualZone::Graveyard,
    ManualZone::Exile,
    ManualZone::LibraryTop,
    ManualZone::LibraryBottom,
    ManualZone::Command,
];

/// Draws the moments of a game at which a seat takes a manual action, and the action.
#[derive(Debug, Clone)]
pub struct ManualScript {
    rng: GameRng,
    every: u64,
}

impl ManualScript {
    /// A script for the game with this seed that acts at one priority in `every` (never when 0).
    pub fn new(seed: u64, every: usize) -> Self {
        ManualScript {
            rng: GameRng::substream(seed, 90_000),
            every: every as u64,
        }
    }

    /// A script for out-of-band actions (`Engine::manual`) at one decision in `every`, of any kind and any seat, drawn
    /// apart from [`new`](ManualScript::new).
    pub fn anytime(seed: u64, every: usize) -> Self {
        ManualScript {
            rng: GameRng::substream(seed, 90_001),
            every: every as u64,
        }
    }

    /// A seat still in the game, drawn at random.
    pub fn seat(&mut self, state: &GameState) -> Option<PlayerId> {
        let alive: Vec<PlayerId> = state
            .players()
            .iter()
            .filter(|p| p.is_in_game())
            .map(mtg_engine::player::Player::id)
            .collect();
        self.pick(&alive)
    }

    /// Whether the priority decision now gets a manual action.
    pub fn due(&mut self) -> bool {
        self.every > 0 && self.rng.below(self.every) == 0
    }

    /// An action `player` may take now, or `None` when the draws found none.
    pub fn action(&mut self, state: &GameState, player: PlayerId) -> Option<ManualAction> {
        (0..TRIES)
            .map(|_| self.candidate(state, player))
            .find(|action| legal(state, player, action).is_ok())
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        (!items.is_empty()).then(|| items[self.rng.index(items.len())])
    }

    fn zone(&mut self) -> ManualZone {
        ZONES[self.rng.index(ZONES.len())]
    }

    fn counter(&mut self) -> CounterKind {
        match self.rng.below(5) {
            0 => CounterKind::PlusOnePlusOne,
            1 => CounterKind::MinusOneMinusOne,
            2 => CounterKind::Loyalty,
            3 => CounterKind::Charge,
            _ => CounterKind::Other("quest".to_owned()),
        }
    }

    fn delta(&mut self) -> i32 {
        let size = i32::try_from(self.rng.below(3)).unwrap_or(0) + 1;
        if self.rng.below(2) == 0 { size } else { -size }
    }

    /// Objects of the seat in a zone, or on the battlefield whoever controls them.
    fn objects(state: &GameState, player: PlayerId, scope: Scope) -> Vec<ObjectId> {
        let zones: Vec<Zone> = match scope {
            Scope::Mine => vec![
                Zone::Battlefield,
                Zone::Hand(player),
                Zone::Graveyard(player),
                Zone::Exile,
            ],
            Scope::Hand => vec![Zone::Hand(player)],
            Scope::Battlefield => vec![Zone::Battlefield],
        };
        zones
            .into_iter()
            .flat_map(|zone| state.zone(zone).iter().copied())
            .collect()
    }

    fn candidate(&mut self, state: &GameState, player: PlayerId) -> ManualAction {
        let nothing = ManualAction::Draw { count: 1 };
        match self.rng.below(20) {
            0..=2 => {
                let mine = Self::objects(state, player, Scope::Mine);
                let to = self.zone();
                self.pick(&mine)
                    .map_or(nothing, |object| ManualAction::Move { object, to })
            }
            3 => ManualAction::MoveTop {
                count: u32::try_from(self.rng.below(3)).unwrap_or(0) + 1,
                to: self.zone(),
            },
            4 | 5 => {
                let table = Self::objects(state, player, Scope::Battlefield);
                let (counter, delta) = (self.counter(), self.delta());
                self.pick(&table)
                    .map_or(nothing, |object| ManualAction::Counters {
                        object,
                        counter,
                        delta,
                    })
            }
            6 => ManualAction::PlayerCounters {
                player,
                counter: if self.rng.below(2) == 0 {
                    CounterKind::Poison
                } else {
                    CounterKind::Other("energy".to_owned())
                },
                delta: self.delta(),
            },
            7 | 8 => ManualAction::Life {
                player,
                delta: self.delta() * 2,
            },
            9 => ManualAction::CreateToken {
                token: self.token(),
                count: u32::try_from(self.rng.below(2)).unwrap_or(0) + 1,
            },
            10 => {
                let table = Self::objects(state, player, Scope::Battlefield);
                let tap = self.rng.below(2) == 0;
                self.pick(&table).map_or(nothing, |object| {
                    if tap {
                        ManualAction::Tap { object }
                    } else {
                        ManualAction::Untap { object }
                    }
                })
            }
            11 => {
                let hand = Self::objects(state, player, Scope::Hand);
                if self.rng.below(2) == 0 {
                    self.pick(&hand)
                        .map_or(nothing, |object| ManualAction::Discard { object })
                } else {
                    nothing
                }
            }
            12 => {
                let mine = Self::objects(state, player, Scope::Mine);
                let table = Self::objects(state, player, Scope::Battlefield);
                match (self.pick(&mine), self.pick(&table)) {
                    (Some(object), Some(to)) => ManualAction::Attach { object, to },
                    _ => nothing,
                }
            }
            13 => {
                let mine = Self::objects(state, player, Scope::Mine);
                self.pick(&mine)
                    .map_or(nothing, |object| ManualAction::Unattach { object })
            }
            14 => self.damage(state, player).unwrap_or(nothing),
            15 => ManualAction::AddMana {
                mana: [ManaType::Red, ManaType::Green, ManaType::Colorless][self.rng.index(3)],
                count: u32::try_from(self.rng.below(3)).unwrap_or(0) + 1,
            },
            16 => ManualAction::LookTop {
                count: u32::try_from(self.rng.below(3)).unwrap_or(0) + 1,
            },
            17 => self.arrange(state, player).unwrap_or(nothing),
            18 => ManualAction::Shuffle {},
            _ => {
                let table = Self::objects(state, player, Scope::Battlefield);
                let (power, toughness) = (self.delta(), self.delta());
                self.pick(&table)
                    .map_or(nothing, |object| ManualAction::Pump {
                        object,
                        power,
                        toughness,
                    })
            }
        }
    }

    /// The top cards of the library the seat has looked at (one to three), sorted at random into the three piles of
    /// an `ArrangeTop`; `None` when the top card is not known to the seat.
    fn arrange(&mut self, state: &GameState, player: PlayerId) -> Option<ManualAction> {
        let count = usize::try_from(self.rng.below(3)).unwrap_or(0) + 1;
        let seen: Vec<ObjectId> = state
            .zone(Zone::Library(player))
            .iter()
            .rev()
            .take(count)
            .take_while(|card| state.is_shown_to(player, **card))
            .copied()
            .collect();
        if seen.is_empty() {
            return None;
        }
        let (mut top, mut bottom, mut graveyard) = (Vec::new(), Vec::new(), Vec::new());
        for card in seen {
            match self.rng.below(3) {
                0 => top.push(card),
                1 => bottom.push(card),
                _ => graveyard.push(card),
            }
        }
        Some(ManualAction::ArrangeTop {
            top,
            bottom,
            graveyard,
        })
    }

    /// Damage from a card of the seat to a random player or permanent.
    fn damage(&mut self, state: &GameState, player: PlayerId) -> Option<ManualAction> {
        let source = self.pick(&Self::objects(state, player, Scope::Mine))?;
        let target = if self.rng.below(2) == 0 {
            DamageTarget::Player(self.seat(state)?)
        } else {
            DamageTarget::Object(self.pick(&Self::objects(state, player, Scope::Battlefield))?)
        };
        Some(ManualAction::Damage {
            source,
            target,
            amount: u32::try_from(self.rng.below(4)).unwrap_or(0) + 1,
        })
    }

    fn token(&mut self) -> ManualToken {
        if self.rng.below(3) == 0 {
            let power = i32::try_from(self.rng.below(4)).unwrap_or(0);
            return ManualToken::Custom(Box::new(CustomToken {
                name: "Imp".to_owned(),
                colors: vec![Color::Black],
                types: vec![CardType::Creature],
                subtypes: vec!["Imp".to_owned()],
                power: Some(power),
                toughness: Some(power + 1),
            }));
        }
        let all = CommonToken::ALL;
        ManualToken::Common(all[self.rng.index(all.len())])
    }
}

/// Which objects a candidate draws its subject from.
#[derive(Clone, Copy)]
enum Scope {
    /// The seat's battlefield, hand, graveyard and exile.
    Mine,
    /// The seat's hand.
    Hand,
    /// The battlefield, whoever controls the permanent.
    Battlefield,
}
