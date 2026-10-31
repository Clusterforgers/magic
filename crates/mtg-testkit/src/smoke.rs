//! The smoke gate of the read tier: plays one card in bot games at two and four seats under every invariant and the
//! watchdog, counts how the games used it, and names the first game that failed or that never met the card.

use std::collections::BTreeSet;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::Duration;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::{Color, ColorSet};
use mtg_core::ids::ObjectId;
use mtg_engine::format::Normal;
use mtg_engine::{Engine, Event, GameSetup};
use thiserror::Error;

use crate::cards::{cost, land};
use crate::harness::{FailureKind, PlayOptions, bots_for, play_game_with_bots};

/// The table sizes a card is played at.
pub const SEATS: [usize; 2] = [2, 4];

/// The seeds the gate plays when the caller has no others: one game at each table size for each.
pub const DEFAULT_SEEDS: [u64; 2] = [1, 2];

/// Cards in a library of the gate: copies of the card, lands and creatures.
const LIBRARY: usize = 40;

/// Copies of the card in a library: as many as a deck of the format may hold, so that four seats do not put on the
/// table more of a card than a real game could.
const COPIES: usize = 4;

/// Lands in a library.
const LANDS: usize = 16;

/// Answers after which a game counts as stuck: a little above the longest game of the suites (about 39,000), so a
/// game that merely drags on ends and a card that keeps the game from ending does not. A loop within one turn ends
/// sooner, at the watchdog's cap on a turn.
const MAX_ANSWERS: usize = 50_000;

/// Wall-clock time after which a game counts as stuck: a game of the suites takes well under a second, so a card whose
/// every answer is slow (a mana ability that searches the whole table) fails the gate instead of holding it for minutes.
const TIME_LIMIT: Duration = Duration::from_secs(20);

/// Permanents of no use but as targets in a library: artifacts and enchantments, two of each.
const TARGETS: usize = 4;

/// Colours a library produces mana of at most.
const MOST_COLORS: usize = 3;

/// How the games used the card, counted over the events of every game.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Uses {
    /// Times it was cast as a spell.
    pub cast: usize,
    /// Times a spell of it, or an ability of it that went on the stack, finished resolving.
    pub resolved: usize,
    /// Times one of its abilities was activated (a mana ability once for the mana it adds in a row).
    pub activated: usize,
    /// Times one of its abilities triggered.
    pub triggered: usize,
    /// Times it was played as a land.
    pub played: usize,
}

impl Uses {
    /// Every use counted: a card that is cast, played, activated or triggered was seen in a game.
    pub const fn total(&self) -> usize {
        self.cast + self.activated + self.triggered + self.played
    }

    fn add(&mut self, other: Uses) {
        self.cast += other.cast;
        self.resolved += other.resolved;
        self.activated += other.activated;
        self.triggered += other.triggered;
        self.played += other.played;
    }
}

/// The game a failure happened in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameAt {
    /// Seats at the table.
    pub seats: usize,
    /// The seed of the game and its bots.
    pub seed: u64,
}

impl fmt::Display for GameAt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} seats, seed {}", self.seats, self.seed)
    }
}

/// Why a card fails the gate.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SmokeFailure {
    /// An invariant of the engine broke.
    #[error("{at}: invariant violated: {detail}")]
    Invariant {
        /// The game.
        at: GameAt,
        /// What broke.
        detail: String,
    },
    /// The engine refused an answer built from its own options, or failed internally (a rules file whose effect errors).
    #[error("{at}: engine error: {detail}")]
    Engine {
        /// The game.
        at: GameAt,
        /// The error.
        detail: String,
    },
    /// The game did not end: it passed the answer limit, the watchdog's cap or the time limit.
    #[error("{at}: stuck: {detail}")]
    Stuck {
        /// The game.
        at: GameAt,
        /// Which limit.
        detail: String,
    },
    /// The game ended in an inconsistent way.
    #[error("{at}: bad ending: {detail}")]
    BadEnding {
        /// The game.
        at: GameAt,
        /// What was inconsistent.
        detail: String,
    },
    /// The engine panicked.
    #[error("{at}: panicked: {detail}")]
    Panicked {
        /// The game.
        at: GameAt,
        /// The panic message.
        detail: String,
    },
    /// The games ended well but the card was never cast, played, activated or triggered.
    #[error("never seen in {games} games")]
    NeverSeen {
        /// Games played.
        games: usize,
    },
}

/// What the gate found out about one card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmokeReport {
    /// The card's name.
    pub card: String,
    /// Games played, the failed one included.
    pub games: usize,
    /// How the games used the card.
    pub uses: Uses,
    /// The first failure; none when the card passes.
    pub failure: Option<SmokeFailure>,
}

impl SmokeReport {
    /// Whether every game ended well and the card was used.
    pub const fn passed(&self) -> bool {
        self.failure.is_none()
    }
}

/// Plays `card` in a bot game at each table size of [`SEATS`] for each of `seeds`: every library holds copies of the
/// card beside lands of the colours it needs and creatures, and the bots prefer to play the card, so its spells,
/// abilities and triggers run under every invariant and the watchdog. The first failed game ends the run; otherwise the
/// card fails when no game used it.
pub fn smoke_card(card: &CardDefinition, seeds: &[u64]) -> SmokeReport {
    let mut card = card.clone();
    // A card on a player's read leaves a failing ability to the table; the gate is there to see the error.
    card.read = false;
    card.faces.iter_mut().for_each(|face| face.read = false);
    let card = Arc::new(card);
    let mut report = SmokeReport {
        card: card.name.clone(),
        games: 0,
        uses: Uses::default(),
        failure: None,
    };
    'games: for &seed in seeds {
        for seats in SEATS {
            report.games += 1;
            match play_one(&card, GameAt { seats, seed }) {
                Ok(uses) => report.uses.add(uses),
                Err(failure) => {
                    report.failure = Some(failure);
                    break 'games;
                }
            }
        }
    }
    if report.failure.is_none() && report.uses.total() == 0 {
        report.failure = Some(SmokeFailure::NeverSeen {
            games: report.games,
        });
    }
    report
}

/// The game of `seats` seats and `seed` the gate plays: every library is the same forty cards, shuffled by the seed.
pub fn smoke_setup(card: &Arc<CardDefinition>, seats: usize, seed: u64) -> GameSetup {
    let library = smoke_library(card);
    let mut setup = GameSetup::new(Arc::new(Normal), seed, vec![library; seats]);
    // The libraries are shorter than a deck of the format and hold more copies than it allows.
    setup.validate_decks = false;
    setup
}

/// The colours `card` needs mana of, most needed first and at most [`MOST_COLORS`]; green and red for a card that needs none.
fn colors_needed(card: &CardDefinition) -> Vec<Color> {
    let mut weight = [0usize; 5];
    for face in std::iter::once(card).chain(card.each_face()) {
        for color in face.colors().iter() {
            weight[color.index()] += 3;
        }
    }
    let json = serde_json::to_string(card).unwrap_or_default();
    for color in Color::ALL {
        weight[color.index()] += json.matches(&format!("\"{color:?}\"")).count();
    }
    for symbol in json.split('{').skip(1) {
        // A cost written as text, such as `{1}{R}`; the braces of the JSON itself are followed by a quote.
        let Some((inside, _)) = symbol.split_once('}') else {
            continue;
        };
        if inside
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/')
        {
            for letter in inside.chars() {
                if let Some(color) = Color::from_symbol(letter) {
                    weight[color.index()] += 1;
                }
            }
        }
    }
    let mut colors: Vec<Color> = Color::ALL
        .into_iter()
        .filter(|color| weight[color.index()] > 0)
        .collect();
    colors.sort_by_key(|color| std::cmp::Reverse(weight[color.index()]));
    colors.truncate(MOST_COLORS);
    if colors.is_empty() {
        colors = vec![Color::Green, Color::Red];
    }
    colors
}

/// A creature of the colour `color` whose cost is `generic` mana, so any land pays for it and the colours of the card
/// never decide what it can target.
fn filler_creature(slot: usize, color: Color) -> Arc<CardDefinition> {
    let size = 1 + i32::try_from(slot % 3).unwrap_or(0);
    let card = CardDefinition::new(format!("Smoke Creature {slot}"))
        .with_type(CardType::Creature)
        .with_mana_cost(cost(u32::try_from(slot % 3 + 1).unwrap_or(1), &[]))
        .with_color_indicator(ColorSet::EMPTY.with(color))
        .with_power_toughness(size, size + 1);
    Arc::new(card)
}

/// An artifact or enchantment for one mana that does nothing.
fn target_permanent(kind: CardType, slot: usize) -> Arc<CardDefinition> {
    let name = format!("Smoke {kind:?} {slot}");
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_mana_cost(cost(1, &[])),
    )
}

/// One library: [`COPIES`] of the card, [`LANDS`] lands that make its colours in turn, a few artifacts and enchantments,
/// and creatures of every colour to fill it, so that there is something of each kind to target, block and attack with.
fn smoke_library(card: &Arc<CardDefinition>) -> Vec<Arc<CardDefinition>> {
    let colors = colors_needed(card);
    let mut library: Vec<Arc<CardDefinition>> = vec![Arc::clone(card); COPIES];
    library.extend((0..LANDS).map(|at| land(colors[at % colors.len()])));
    library.extend((0..TARGETS).map(|slot| {
        let kind = if slot % 2 == 0 {
            CardType::Artifact
        } else {
            CardType::Enchantment
        };
        target_permanent(kind, slot / 2)
    }));
    let creatures = LIBRARY - COPIES - LANDS - TARGETS;
    library.extend((0..creatures).map(|slot| filler_creature(slot, Color::ALL[slot % 5])));
    library
}

/// The objects of the card in the opening state of `setup`.
fn card_objects(
    setup: &GameSetup,
    name: &str,
) -> Result<BTreeSet<ObjectId>, mtg_engine::EngineError> {
    let engine = Engine::new(setup.clone())?;
    Ok(engine
        .state()
        .objects()
        .filter(|object| object.card().name == name)
        .map(mtg_engine::object::GameObject::id)
        .collect())
}

/// Plays the game at `at`, and counts the uses of the card in it.
fn play_one(card: &Arc<CardDefinition>, at: GameAt) -> Result<Uses, SmokeFailure> {
    let setup = smoke_setup(card, at.seats, at.seed);
    let detail = |error: &dyn fmt::Display| error.to_string();
    let objects = card_objects(&setup, &card.name).map_err(|error| SmokeFailure::Engine {
        at,
        detail: detail(&error),
    })?;
    // Odd seeds also pay in part and choose how to pay hybrid and Phyrexian symbols.
    let odd = at.seed % 2 == 1;
    let options = PlayOptions {
        max_answers: MAX_ANSWERS,
        partial_payments: odd,
        symbol_choices: odd,
        runaway_fails: true,
        time_limit: Some(TIME_LIMIT),
        ..PlayOptions::default()
    };
    let bots = bots_for(at.seats, at.seed, options)
        .into_iter()
        .map(|bot| bot.favouring(card.name.clone()))
        .collect();
    let played = catch_unwind(AssertUnwindSafe(|| {
        play_game_with_bots(setup, at.seed, options, bots)
    }));
    match played {
        Ok(Ok(outcome)) => Ok(count_uses(&outcome.events, &objects)),
        Ok(Err(failure)) => Err(match failure.kind {
            FailureKind::Invariant(violation) => SmokeFailure::Invariant {
                at,
                detail: detail(&violation),
            },
            FailureKind::Engine(error) => SmokeFailure::Engine {
                at,
                detail: detail(&error),
            },
            FailureKind::StepLimit(answers) => SmokeFailure::Stuck {
                at,
                detail: format!("no result after {answers} answers"),
            },
            FailureKind::Runaway(report) => SmokeFailure::Stuck {
                at,
                detail: first_line(&report),
            },
            kind @ (FailureKind::BadEnding(_) | FailureKind::Unanswered(_)) => {
                SmokeFailure::BadEnding {
                    at,
                    detail: detail(&kind),
                }
            }
        }),
        Err(panic) => Err(panic_failure(at, panic.as_ref())),
    }
}

/// The first line of `text`, which names the failure the rest of a report explains.
fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_owned()
}

/// The failure of a game that panicked.
fn panic_failure(at: GameAt, panic: &(dyn std::any::Any + Send)) -> SmokeFailure {
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("no message");
    SmokeFailure::Panicked {
        at,
        detail: first_line(message),
    }
}

/// The uses of the objects `card` in the log of a game.
fn count_uses(events: &[Event], card: &BTreeSet<ObjectId>) -> Uses {
    let mut uses = Uses::default();
    // The abilities of the card that went on the stack: their resolutions count.
    let mut on_stack: BTreeSet<ObjectId> = BTreeSet::new();
    // The source of the mana added by the event before: one activation of a mana ability adds its mana in a row.
    let mut adding: Option<ObjectId> = None;
    for event in events {
        let before = adding.take();
        match event {
            Event::SpellCast { object, .. } if card.contains(object) => uses.cast += 1,
            Event::SpellResolved { object } if card.contains(object) => uses.resolved += 1,
            Event::LandPlayed { object, .. } if card.contains(object) => uses.played += 1,
            Event::AbilityActivated { source, .. } if card.contains(&source.id) => {
                uses.activated += 1;
            }
            Event::AbilityTriggered(triggered) if card.contains(&triggered.source.id) => {
                uses.triggered += 1;
            }
            Event::AbilityPutOnStack(put) if card.contains(&put.source.id) => {
                on_stack.insert(put.object);
            }
            Event::ResolutionEnded {
                object,
                outcome: mtg_engine::events::ResolutionOutcome::Resolved,
            } if on_stack.contains(object) => uses.resolved += 1,
            Event::ManaAdded {
                source: Some(source),
                ..
            } if card.contains(source) => {
                adding = Some(*source);
                uses.activated += usize::from(before != adding);
            }
            _ => {}
        }
    }
    uses
}

#[cfg(test)]
mod tests {
    //! Goal: the gate reads the colours a card needs from its cost and abilities, counts only the events of the card's own objects, and tells a watchdog report from any other panic.
    use super::*;
    use crate::cards::creature;
    use crate::trigger_cards::artifact_with;
    use mtg_cards::ability::{Ability, Timing};
    use mtg_cards::cost::{Cost, CostItem};
    use mtg_cards::mana::ManaCostX;
    use mtg_core::ids::PlayerId;
    use mtg_engine::choose::Choices;
    use mtg_engine::events::ResolutionOutcome;
    use rstest::rstest;

    fn paying_artifact(text: &str) -> CardDefinition {
        let mana: ManaCostX = text.parse().unwrap();
        let ability = Ability::Activated {
            cost: Cost(vec![CostItem::Mana(mana)]),
            timing: Timing::default(),
            condition: None,
            limit: None,
            from: mtg_cards::refs::ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: Vec::new(),
        };
        (*artifact_with("Paying", vec![ability])).clone()
    }

    fn plain(card: &Arc<CardDefinition>) -> CardDefinition {
        (**card).clone()
    }

    #[rstest]
    #[case::vanilla(plain(&creature("a", 2, &[], 1, 1)), vec![Color::Green, Color::Red])]
    #[case::two_colors(plain(&creature("b", 1, &[Color::Red, Color::Red, Color::Blue], 1, 1)), vec![Color::Red, Color::Blue])]
    #[case::land(plain(&land(Color::Black)), vec![Color::Black])]
    #[case::ability_cost(paying_artifact("{1}{W}{W}"), vec![Color::White])]
    #[case::hybrid_ability_cost(paying_artifact("{G/U}"), vec![Color::Blue, Color::Green])]
    fn the_library_makes_the_colors_a_card_needs(
        #[case] card: CardDefinition,
        #[case] expected: Vec<Color>,
    ) {
        let mut found = colors_needed(&card);
        found.sort();
        let mut expected = expected;
        expected.sort();
        assert_eq!(found, expected);
    }

    #[test]
    fn a_library_is_forty_cards_with_copies_of_the_card_and_lands_of_its_colors() {
        let card = creature("Smoke Subject", 1, &[Color::White, Color::Black], 2, 2);
        let library = smoke_library(&card);
        assert_eq!(library.len(), LIBRARY);
        assert_eq!(
            library.iter().filter(|c| c.name == "Smoke Subject").count(),
            COPIES
        );
        let lands: Vec<&Arc<CardDefinition>> = library
            .iter()
            .filter(|c| c.is_type(mtg_core::card::CardType::Land))
            .collect();
        assert_eq!(lands.len(), LANDS);
        let names: BTreeSet<&str> = lands.iter().map(|land| land.name.as_str()).collect();
        assert_eq!(names, BTreeSet::from(["Plains", "Swamp"]));
    }

    #[test]
    fn uses_count_only_the_objects_of_the_card() {
        let ours = ObjectId(1);
        let other = ObjectId(2);
        let stack = ObjectId(3);
        let card = BTreeSet::from([ours]);
        let player = PlayerId(0);
        let cast = |object| Event::SpellCast {
            player,
            object,
            choices: Choices::default(),
        };
        let events = vec![
            cast(ours),
            cast(other),
            Event::SpellResolved { object: ours },
            Event::SpellResolved { object: other },
            Event::LandPlayed {
                player,
                object: ours,
                face: None,
            },
            Event::ResolutionEnded {
                object: stack,
                outcome: ResolutionOutcome::Resolved,
            },
        ];
        let uses = count_uses(&events, &card);
        let expected = Uses {
            cast: 1,
            resolved: 1,
            played: 1,
            ..Uses::default()
        };
        assert_eq!(uses, expected);
        assert_eq!(uses.total(), 2);
    }

    #[test]
    fn the_library_holds_targets_of_every_kind_for_the_card() {
        let card = creature("Smoke Subject", 1, &[Color::Red], 2, 2);
        let library = smoke_library(&card);
        let of_type = |ty: CardType| library.iter().filter(|c| c.is_type(ty)).count();
        assert_eq!(of_type(CardType::Artifact), TARGETS / 2);
        assert_eq!(of_type(CardType::Enchantment), TARGETS / 2);
        assert_eq!(
            of_type(CardType::Creature),
            LIBRARY - LANDS - TARGETS,
            "the card and the fillers"
        );
        let colors: BTreeSet<Color> = library
            .iter()
            .filter(|c| c.name.starts_with("Smoke Creature"))
            .flat_map(|c| c.colors().iter())
            .collect();
        assert_eq!(
            colors.len(),
            5,
            "every colour is there to be named a target"
        );
    }

    #[rstest]
    #[case::string(Box::new("index out of bounds: the len is 2\nbut the index is 5".to_owned()) as Box<dyn std::any::Any + Send>)]
    #[case::str(Box::new("explicit panic") as Box<dyn std::any::Any + Send>)]
    #[case::other(Box::new(7_u8) as Box<dyn std::any::Any + Send>)]
    fn a_panic_is_reported_by_its_first_line(#[case] panic: Box<dyn std::any::Any + Send>) {
        let at = GameAt { seats: 2, seed: 3 };
        let failure = panic_failure(at, panic.as_ref());
        assert!(matches!(failure, SmokeFailure::Panicked { .. }));
        assert!(
            failure
                .to_string()
                .starts_with("2 seats, seed 3: panicked: ")
        );
        assert!(!failure.to_string().contains('\n'));
    }

    #[test]
    fn a_gate_with_no_seeds_has_seen_nothing() {
        let card = creature("Nothing", 0, &[], 1, 1);
        let report = smoke_card(&card, &[]);
        assert_eq!(report.failure, Some(SmokeFailure::NeverSeen { games: 0 }));
        assert!(!report.passed());
    }
}
