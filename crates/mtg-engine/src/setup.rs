//! The initial setup of a game and the state it builds.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Supertype};
use mtg_core::house::HouseRules;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::combat::CombatState;
use crate::commanders::Commanders;
use crate::companions::CompanionStage;
use crate::error::SetupError;
use crate::format::{ExtraZone, FormatRules, RuleSet};
use crate::layers::LayersCache;
use crate::object::{Designations, GameObject, ObjectRef, PermanentState};
use crate::player::{MulliganStatus, Player};
use crate::rules::RulesCache;
use crate::state::{GameState, Possible, Stage, Tallies};
use crate::turn::{Step, TurnState};
use crate::zones::{Zone, Zones};

/// A permanent that starts on the battlefield (scenario setups).
#[derive(Debug, Clone)]
pub struct PresetPermanent {
    /// The card.
    pub card: Arc<CardDefinition>,
    /// Starts tapped.
    pub tapped: bool,
    /// Starts summoning sick.
    pub summoning_sick: bool,
    /// Damage already marked.
    pub damage: u32,
    /// Counters already on it, by kind; a kind listed twice adds up.
    pub counters: Vec<(CounterKind, u32)>,
    /// The permanent it starts attached to, as (seat, place in that seat's `battlefield`).
    pub attached_to: Option<(usize, usize)>,
    /// The seat of the player it starts attached to (an Aura that enchants a player).
    pub attached_player: Option<usize>,
}

impl PresetPermanent {
    /// An untapped, ready permanent.
    pub fn new(card: Arc<CardDefinition>) -> Self {
        PresetPermanent {
            card,
            tapped: false,
            summoning_sick: false,
            damage: 0,
            counters: Vec::new(),
            attached_to: None,
            attached_player: None,
        }
    }
}

impl From<Arc<CardDefinition>> for PresetPermanent {
    fn from(card: Arc<CardDefinition>) -> Self {
        PresetPermanent::new(card)
    }
}

/// One seat's starting contents.
#[derive(Debug, Clone, Default)]
pub struct PlayerSetup {
    /// Library, top card first.
    pub library: Vec<Arc<CardDefinition>>,
    /// Starting hand (scenario setups; normal games deal from the library).
    pub hand: Vec<Arc<CardDefinition>>,
    /// Permanents already in play.
    pub battlefield: Vec<PresetPermanent>,
    /// Graveyard, oldest card first.
    pub graveyard: Vec<Arc<CardDefinition>>,
    /// Starting life; the format's default when `None`.
    pub life: Option<i32>,
    /// Keywords the player has from the start, such as hexproof (CR 702.11d).
    pub keywords: Vec<Keyword>,
    /// Counters on the player from the start, by kind; a kind listed twice adds up.
    pub counters: Vec<(CounterKind, u32)>,
    /// The commanders the seat starts with in the command zone (CR 903.6), one or a pair of partners; the cards
    /// of the deck are the other ones.
    pub commanders: Vec<Arc<CardDefinition>>,
    /// The companion the seat names (CR 702.139): a card outside its deck that the game keeps outside the game until the
    /// seat pays to put it into its hand.
    pub companion: Option<Arc<CardDefinition>>,
}

/// Where the game starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// Pick a first player, shuffle, deal opening hands and mulligan.
    Mulligan,
    /// Skip the opening: the game begins at the start of this step (its turn-based actions still run).
    At {
        /// Turn number, from 1.
        turn: u32,
        /// The active player.
        active: PlayerId,
        /// The step.
        step: Step,
    },
}

/// Everything needed to reproduce a game: replaying events over this setup yields the state.
#[derive(Debug, Clone)]
pub struct GameSetup {
    /// The format's rules.
    pub format: Arc<dyn FormatRules>,
    /// What the table's creator set in place of the format's values.
    pub house_rules: HouseRules,
    /// Seed for shuffles and the first-player roll.
    pub seed: u64,
    /// Seats in turn order.
    pub players: Vec<PlayerSetup>,
    /// Where the game starts.
    pub start: Start,
    /// Whether decks are checked against the format (off for scenario setups).
    pub validate_decks: bool,
}

impl GameSetup {
    /// A normal game start with one deck per seat.
    pub fn new(
        format: Arc<dyn FormatRules>,
        seed: u64,
        decks: Vec<Vec<Arc<CardDefinition>>>,
    ) -> Self {
        GameSetup {
            format,
            house_rules: HouseRules::default(),
            seed,
            players: decks
                .into_iter()
                .map(|library| PlayerSetup {
                    library,
                    ..PlayerSetup::default()
                })
                .collect(),
            start: Start::Mulligan,
            validate_decks: true,
        }
    }

    /// The setup with these commanders, one list for each seat in table order, starting in the command zone.
    #[must_use]
    pub fn with_commanders(mut self, commanders: Vec<Vec<Arc<CardDefinition>>>) -> Self {
        for (player, cards) in self.players.iter_mut().zip(commanders) {
            player.commanders = cards;
        }
        self
    }

    /// The setup with these companions, one entry for each seat in table order.
    #[must_use]
    pub fn with_companions(mut self, companions: Vec<Option<Arc<CardDefinition>>>) -> Self {
        for (player, card) in self.players.iter_mut().zip(companions) {
            player.companion = card;
        }
        self
    }

    /// The setup with `house_rules` laid over the format's values.
    #[must_use]
    pub fn with_house_rules(mut self, house_rules: HouseRules) -> Self {
        self.house_rules = house_rules;
        self
    }

    /// The rules the game plays by: the format's values with the house rules over them.
    pub fn ruleset(&self) -> RuleSet<'_> {
        RuleSet::new(&*self.format, &self.house_rules)
    }

    /// Checks seat count, decks and the starting point.
    pub fn validate(&self) -> Result<(), SetupError> {
        self.house_rules.validate_for(self.players.len())?;
        if self.players.len() < 2 {
            return Err(SetupError::TooFewPlayers(self.players.len()));
        }
        let has_command_zone = self.format.extra_zones().contains(&ExtraZone::Command);
        if let Some(seat) = self
            .players
            .iter()
            .position(|player| !player.commanders.is_empty())
            .filter(|_| !has_command_zone)
        {
            return Err(SetupError::NoCommandZone { seat });
        }
        if self.validate_decks {
            for (seat, player) in self.players.iter().enumerate() {
                let deck: Vec<_> = player
                    .library
                    .iter()
                    .chain(&player.hand)
                    .chain(player.battlefield.iter().map(|p| &p.card))
                    .chain(&player.graveyard)
                    .cloned()
                    .collect();
                self.format
                    .validate_seat(&deck, &player.commanders, &self.ruleset())
                    .map_err(|source| SetupError::InvalidDeck {
                        player: PlayerId::from_index(seat),
                        source,
                    })?;
                if let Some(companion) = &player.companion {
                    self.format
                        .validate_companion(companion, &deck, &player.commanders, &self.ruleset())
                        .map_err(|source| SetupError::InvalidCompanion {
                            player: PlayerId::from_index(seat),
                            source,
                        })?;
                }
            }
        }
        for (seat, player) in self.players.iter().enumerate() {
            for (index, preset) in player.battlefield.iter().enumerate() {
                let host = preset.attached_to;
                if host.is_some_and(|(host_seat, place)| {
                    (host_seat, place) == (seat, index)
                        || self
                            .players
                            .get(host_seat)
                            .is_none_or(|p| place >= p.battlefield.len())
                }) {
                    return Err(SetupError::BadAttachment { seat, index });
                }
                if preset
                    .attached_player
                    .is_some_and(|enchanted| enchanted >= self.players.len())
                {
                    return Err(SetupError::BadAttachment { seat, index });
                }
            }
        }
        if let Start::At { turn, active, step } = self.start {
            if turn == 0 {
                return Err(SetupError::InvalidStart("turns start at 1"));
            }
            if matches!(
                step,
                Step::DeclareBlockers | Step::FirstStrikeDamage | Step::CombatDamage
            ) {
                return Err(SetupError::InvalidStart(
                    "combat can only be entered at declare attackers",
                ));
            }
            if active.index() >= self.players.len() {
                return Err(SetupError::InvalidStart("active player does not exist"));
            }
        }
        Ok(())
    }
}

/// The permanent state a starting permanent begins with; `host` is the object it starts attached to.
fn preset_state(
    preset: &PresetPermanent,
    host: Option<ObjectId>,
    enchanted: Option<PlayerId>,
    owner: PlayerId,
) -> PermanentState {
    let mut counters: BTreeMap<CounterKind, u32> = BTreeMap::new();
    for (kind, count) in &preset.counters {
        if *count > 0 {
            *counters.entry(kind.clone()).or_default() += count;
        }
    }
    // A planeswalker starts with its printed loyalty unless the setup says how many loyalty counters it holds (CR 306.5b).
    if preset.card.type_set().contains(CardType::Planeswalker)
        && !counters.contains_key(&CounterKind::Loyalty)
        && let Some(loyalty) = preset.card.loyalty.filter(|loyalty| *loyalty > 0)
    {
        counters.insert(CounterKind::Loyalty, loyalty);
    }
    PermanentState {
        tapped: preset.tapped,
        designations: Designations::default(),
        damage: preset.damage,
        deathtouched: false,
        summoning_sick: preset.summoning_sick,
        new_to: None,
        counters,
        // Every object of a fresh game is on its first incarnation (counter 0).
        attached_to: host.map(|id| ObjectRef { id, counter: 0 }),
        attached_player: enchanted,
        // The permanents of a starting position share one time; a game begins with them in play.
        timestamp: 0,
        observed: (!preset.summoning_sick).then_some(owner),
        chosen: None,
        paid: Vec::new(),
        way: None,
        x: None,
        face: None,
        spent: Vec::new(),
        copy: None,
        merged: None,
        phased: crate::object::PhasedState::default(),
    }
}

/// The seat `owner` as its setup describes it: its life (the format's unless it names one), keywords and counters.
fn starting_player(
    owner: PlayerId,
    setup: &PlayerSetup,
    life: i32,
    status: MulliganStatus,
) -> Player {
    let mut player = Player::new(
        owner,
        setup.life.unwrap_or(life),
        status,
        setup.keywords.clone(),
    );
    for (kind, count) in &setup.counters {
        if *count > 0 {
            *player.counters.entry(kind.clone()).or_default() += count;
        }
    }
    player
}

impl GameState {
    /// Builds the initial state; objects get sequential ids per seat: library, hand, battlefield, graveyard.
    // One field for each part of the state, so the list is as long as the state.
    #[allow(clippy::too_many_lines)]
    pub fn from_setup(setup: &GameSetup) -> Result<GameState, SetupError> {
        setup.validate()?;
        let seats = setup.players.len();
        let mut objects = BTreeMap::new();
        let mut zones = Zones::new(seats);
        let mut players = Vec::with_capacity(seats);
        let mut next_id = 0u64;
        let mut create = |owner: PlayerId, card: &Arc<CardDefinition>, zone: Zone| {
            let id = ObjectId(next_id);
            next_id += 1;
            zones.insert(zone, id, crate::zones::Placement::Top);
            objects.insert(id, GameObject::new(id, owner, Arc::clone(card), zone));
            id
        };
        let mut presets = Vec::new();
        let mut commander_ids = Vec::new();
        let mut placed: BTreeMap<(usize, usize), ObjectId> = BTreeMap::new();
        for (seat, setup_player) in setup.players.iter().enumerate() {
            let owner = PlayerId::from_index(seat);
            for card in setup_player.library.iter().rev() {
                create(owner, card, Zone::Library(owner));
            }
            for card in &setup_player.hand {
                create(owner, card, Zone::Hand(owner));
            }
            for (index, preset) in setup_player.battlefield.iter().enumerate() {
                let id = create(owner, &preset.card, Zone::Battlefield);
                placed.insert((seat, index), id);
                presets.push((id, preset.clone()));
            }
            for card in &setup_player.graveyard {
                create(owner, card, Zone::Graveyard(owner));
            }
            for card in &setup_player.commanders {
                commander_ids.push(create(owner, card, Zone::Command(owner)));
            }
            let status = match setup.start {
                Start::Mulligan => MulliganStatus::NeedsHand,
                Start::At { .. } => MulliganStatus::Kept,
            };
            let life = setup.ruleset().starting_life(owner);
            players.push(starting_player(owner, setup_player, life, status));
        }
        // A companion is no object yet: it keeps the id it will have in the hand. It is revealed with the opening
        // procedure, or at once in a game that skips it.
        let mut companions = crate::companions::Companions::default();
        let stage = match setup.start {
            Start::Mulligan => CompanionStage::Hidden,
            Start::At { .. } => CompanionStage::Revealed,
        };
        for (seat, setup_player) in setup.players.iter().enumerate() {
            if let Some(card) = &setup_player.companion {
                companions.name(
                    PlayerId::from_index(seat),
                    ObjectId(next_id),
                    Arc::clone(card),
                    stage,
                );
                next_id += 1;
            }
        }
        for (id, preset) in presets {
            if let Some(object) = objects.get_mut(&id) {
                let host = preset.attached_to.and_then(|place| placed.get(&place));
                let owner = object.owner();
                let enchanted = preset.attached_player.map(PlayerId::from_index);
                object.permanent = Some(preset_state(&preset, host.copied(), enchanted, owner));
            }
        }
        let (stage, turn, first_player) = match setup.start {
            Start::Mulligan => (Stage::PreGame, (0, PlayerId(0), Step::Untap), PlayerId(0)),
            Start::At { turn, active, step } => {
                let back = usize::try_from(turn - 1).unwrap_or(0) % seats;
                let first = PlayerId::from_index((active.index() + seats - back) % seats);
                (Stage::Turns, (turn, active, step), first)
            }
        };
        let mut game = GameState {
            format: Arc::clone(&setup.format),
            house_rules: setup.house_rules.clone(),
            seed: setup.seed,
            rng_index: 0,
            stage,
            first_player,
            players,
            objects,
            ceased: BTreeMap::new(),
            zones,
            extra_turns: Vec::new(),
            turn: TurnState {
                number: turn.0,
                active: turn.1,
                step: turn.2,
                lands_played: 0,
                round: if turn.0 == 0 {
                    0
                } else {
                    (turn.0 - 1) / u32::try_from(seats).unwrap_or(u32::MAX).max(1) + 1
                },
                regular: turn.1,
                extra: false,
                combats: 0,
                ahead: Vec::new(),
            },
            priority: None,
            combat: CombatState::default(),
            effects: Vec::new(),
            rule_effects: Vec::new(),
            next_effect: 0,
            // The permanents of a starting position have timestamp 0; what enters or begins later is after them.
            next_timestamp: 1,
            next_object: next_id,
            resolving: None,
            pending_triggers: Vec::new(),
            trigger_round: 0,
            delayed: Vec::new(),
            next_delayed: 0,
            limits_spent: BTreeSet::new(),
            loyalty_used: BTreeSet::new(),
            tallies: Tallies::default(),
            triggers_possible: false,
            statics_possible: false,
            layers: LayersCache::default(),
            rules_possible: false,
            rules_cache: RulesCache::default(),
            possible: Possible::default(),
            result: None,
            day_night: None,
            monarch: None,
            citys_blessing: std::collections::BTreeSet::new(),
            commanders: Commanders::of(commander_ids),
            companions,
        };
        game.note_what_cards_can_do();
        // A commander is moved by a replacement effect and costs a tax, so both are looked for in its game.
        if game.commanders.any() {
            game.possible.replacements = true;
            game.possible.costs = true;
        }
        Ok(game)
    }

    /// Whether any card of the game can trigger something or has a static ability, which the state then watches for.
    fn note_what_cards_can_do(&mut self) {
        let triggers = self.objects().any(|object| {
            object
                .def
                .each_face()
                .any(crate::ability_reader::definition_triggers)
        });
        let statics = self.objects().any(|object| {
            object
                .def
                .each_face()
                .any(crate::ability_reader::definition_has_static)
        });
        self.triggers_possible = triggers;
        self.statics_possible = statics;
        let rules = self.objects().any(|object| {
            object
                .def
                .each_face()
                .any(crate::ability_reader::definition_has_rule)
        });
        self.rules_possible = rules;
        let elsewhere = self.objects().any(|object| {
            object
                .def
                .each_face()
                .any(crate::ability_reader::definition_works_elsewhere)
        });
        self.possible.elsewhere = elsewhere;
        let states = self.objects().any(|object| {
            object
                .def
                .each_face()
                .any(crate::ability_reader::definition_has_state_trigger)
        });
        self.possible.states = states;
        let counters = self.objects().any(|object| {
            object.permanent().is_some_and(|permanent| {
                permanent
                    .counters()
                    .any(|(kind, _)| crate::counters::changes_characteristics(kind))
            })
        });
        self.possible.counters = counters;
        let attachments = self.objects().any(|object| {
            object
                .def
                .each_face()
                .any(crate::attach::definition_attaches)
                || object
                    .permanent()
                    .is_some_and(|permanent| permanent.attached_to().is_some())
        });
        self.possible.attachments = attachments;
        let legends = self.objects().any(|object| {
            object
                .def
                .each_face()
                .any(|face| face.supertypes.contains(&Supertype::Legendary))
        });
        self.possible.legends = legends;
        let costs = self.objects().any(|object| {
            object
                .def
                .each_face()
                .any(crate::ability_reader::definition_has_cost_modifier)
        });
        self.possible.costs = costs;
        let replacements = self.objects().any(|object| {
            object
                .def
                .each_face()
                .any(crate::ability_reader::definition_has_replacement)
        });
        self.possible.replacements = replacements;
        // Copies of a card share a name and a definition, so each name is read once.
        let mut read = std::collections::BTreeSet::new();
        let protection = self.objects().any(|object| {
            read.insert(object.def.name.as_str())
                && object
                    .def
                    .each_face()
                    .any(crate::ability_reader::definition_has_protection)
        }) || self.players.iter().any(|player| {
            player
                .keywords()
                .iter()
                .any(|keyword| matches!(keyword, mtg_cards::keyword::Keyword::Protection(_)))
        });
        self.possible.protection = protection;
        self.possible.day_night = self.any_face(crate::ability_reader::definition_has_day_night);
        self.possible.ascend = self.any_face(crate::ability_reader::definition_has_ascend);
    }

    /// Whether some face of a card of the game satisfies `test`.
    fn any_face(&self, test: fn(&mtg_cards::CardDefinition) -> bool) -> bool {
        self.objects()
            .any(|object| object.def.each_face().any(test))
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a starting permanent can hold counters and start attached to a permanent of any seat, a bad attachment is refused by name, and the state names a permanent's host and attachments only while the host is the very object it was attached to; a snapshot carries both.
    use mtg_core::card::CardType;
    use rstest::rstest;

    use super::*;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
    use crate::snapshot::snapshot;

    fn aura() -> Arc<CardDefinition> {
        Arc::new(CardDefinition::new("Halo").with_type(CardType::Enchantment))
    }

    fn halo_on(host: (usize, usize)) -> PresetPermanent {
        PresetPermanent {
            attached_to: Some(host),
            ..PresetPermanent::new(aura())
        }
    }

    /// Seat 0: a Bear with counters, a Halo on the Bear and a Halo on seat 1's Ogre; seat 1: the Ogre.
    fn game() -> GameState {
        let mut setup = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
        setup.players[0].battlefield = vec![
            PresetPermanent {
                counters: vec![
                    (CounterKind::Charge, 1),
                    (CounterKind::PlusOnePlusOne, 1),
                    (CounterKind::PlusOnePlusOne, 1),
                    (CounterKind::Poison, 0),
                ],
                ..PresetPermanent::new(creature("Bear", 2, 2))
            },
            halo_on((0, 0)),
            halo_on((1, 0)),
        ];
        setup.players[1].battlefield = vec![PresetPermanent::new(creature("Ogre", 4, 4))];
        GameState::from_setup(&setup).unwrap()
    }

    /// The Bear, the Halo on it, the Halo on the Ogre and the Ogre, in battlefield order.
    fn cast(game: &GameState) -> [ObjectId; 4] {
        [0, 1, 2, 3].map(|at| game.zone(Zone::Battlefield)[at])
    }

    #[test]
    fn counters_add_up_and_attachments_find_their_hosts_across_seats() {
        let game = game();
        let [bear, on_bear, on_ogre, ogre] = cast(&game);
        let counters: Vec<_> = game
            .object(bear)
            .unwrap()
            .permanent()
            .unwrap()
            .counters()
            .map(|(kind, count)| (kind.clone(), count))
            .collect();
        // Equal kinds add up, a count of zero is no counter, and the order is the kind's.
        assert_eq!(
            counters,
            [(CounterKind::PlusOnePlusOne, 2), (CounterKind::Charge, 1)]
        );
        assert_eq!(game.host_of(on_bear), Some(bear));
        assert_eq!(game.host_of(on_ogre), Some(ogre));
        assert_eq!(game.host_of(bear), None);
        assert_eq!(game.attachments_of(bear), [on_bear]);
        assert_eq!(game.attachments_of(ogre), [on_ogre]);
        assert_eq!(game.attachment_index().len(), 2);
        let shot = snapshot(&game, bear).unwrap();
        assert_eq!(shot.counters, counters);
        assert_eq!(shot.attachments, [on_bear]);
    }

    #[test]
    fn a_host_that_left_or_returned_as_a_new_object_holds_nothing() {
        let mut game = game();
        let [bear, on_bear, on_ogre, ogre] = cast(&game);
        let moved = |object, from, to| Event::ZoneChanged {
            object,
            from,
            to,
            cause: ZoneCause::Moved,
        };
        let home = Zone::Graveyard(PlayerId(1));
        apply_event(&mut game, &moved(ogre, Zone::Battlefield, home)).unwrap();
        assert_eq!(game.attachments_of(ogre), []);
        assert_eq!(game.host_of(on_ogre), None);
        // It returns as a new object: the Halo was attached to the old one.
        apply_event(&mut game, &moved(ogre, home, Zone::Battlefield)).unwrap();
        assert_eq!(game.attachments_of(ogre), []);
        assert_eq!(game.host_of(on_ogre), None);
        // The other host is untouched.
        assert_eq!(game.host_of(on_bear), Some(bear));
    }

    #[rstest]
    #[case::to_itself((0, 0), 0, 0)]
    #[case::to_a_seat_that_is_not_there((7, 0), 0, 0)]
    #[case::to_a_place_that_is_not_there((1, 5), 0, 0)]
    fn a_bad_attachment_is_refused_by_name(
        #[case] host: (usize, usize),
        #[case] seat: usize,
        #[case] index: usize,
    ) {
        let mut setup = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
        setup.players[0].battlefield = vec![halo_on(host)];
        setup.players[1].battlefield = vec![PresetPermanent::new(creature("Ogre", 4, 4))];
        assert_eq!(
            GameState::from_setup(&setup).unwrap_err(),
            SetupError::BadAttachment { seat, index }
        );
    }
}
