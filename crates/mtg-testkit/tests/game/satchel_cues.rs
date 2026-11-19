//! Goal: a boss's bot casts each Satchel card at its cue and holds it otherwise, for every cue kind; a reusable card's
//! tax is weighed against its mana value and the hand; a once card is cast once at most.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::collections::BTreeMap;
use std::sync::{Arc, Once};

use common::pid;
use mtg_core::cue::SatchelCue;
use mtg_core::house::HouseRules;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::PresetPermanent;
use mtg_engine::decision::{Answer, PriorityAction};
use mtg_engine::events::Event;
use mtg_engine::phases::{BossPhase, PhaseStart};
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_testkit::brain::{self, Level, satchel};
use mtg_testkit::cards::{forest, free_creature, island, mountain, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const HERO: usize = 0;
const BOSS: usize = 1;

/// The cue of each card these tests put in a Satchel, one card per cue kind.
fn cues() -> BTreeMap<String, SatchelCue> {
    use SatchelCue as C;
    [
        ("Lightning Bolt", C::Kill { toughness: 3 }),
        ("Swords to Plowshares", C::Eyed { creature: true }),
        ("Opt", C::HandLow { cards: 1 }),
        ("Brainstorm", C::Mana { mana: 6 }),
        ("Consider", C::EarlyTurns { turns: 1 }),
        ("Peek", C::Graveyard { creatures: 1 }),
        ("Doom Blade", C::BehindOnBoard { power: 4 }),
        ("Village Rites", C::Board { creatures: 3 }),
        ("Healing Salve", C::HeroesEnd { mana: 0 }),
        ("Counterspell", C::Counter { mana_value: 4 }),
        ("Memory Lapse", C::Protect),
        ("Giant Growth", C::Combat),
        ("Rampant Growth", C::Lethal { reach: 0 }),
    ]
    .into_iter()
    .map(|(name, cue)| (name.to_owned(), cue))
    .collect()
}

/// Installs [`cues`] for the bots of this test binary.
fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        satchel::install(cues());
    });
}

/// What a fight starts with besides its lands and Satchel.
#[derive(Default)]
struct Board {
    hero: Vec<PresetPermanent>,
    boss: Vec<PresetPermanent>,
    /// Of each basic land, for each seat; five when 0.
    lands: usize,
}

/// A fight of one hero (seat 0, a Spirit emblem) and a boss (seat 1, a phase) with `board`; the boss holds `satchel`
/// in its Satchel, reusable or once.
fn fight(satchel: &str, reusable: bool, board: Board) -> Scenario {
    install();
    let house = HouseRules {
        teams: Some(Teams {
            mode: TeamMode::Teams,
            seats: vec![0, 1],
        }),
        ..HouseRules::default()
    };
    let phase = BossPhase {
        begins: PhaseStart::AtLife(20),
        emblem: Arc::new(emblem_definition("Wrath", &[])),
    };
    let each = if board.lands == 0 { 5 } else { board.lands };
    let with_lands = |mut permanents: Vec<PresetPermanent>| {
        for land in [forest, island, mountain, plains, swamp] {
            permanents.extend((0..each).map(|_| ready(land())));
        }
        permanents
    };
    let card = vec![card(satchel).unwrap()];
    let scenario = Scenario::new(2)
        .house_rules(house)
        .spirit(HERO, Arc::new(emblem_definition("Haunt", &[])))
        .phases(BOSS, [phase])
        .battlefield(HERO, with_lands(board.hero))
        .battlefield(BOSS, with_lands(board.boss));
    if reusable {
        scenario.satchel_reusable(BOSS, card)
    } else {
        scenario.satchel(BOSS, card)
    }
}

/// `count` ready copies of the corpus card `name`.
fn copies(name: &str, count: usize) -> Vec<PresetPermanent> {
    (0..count).map(|_| ready(card(name).unwrap())).collect()
}

/// Passes for everyone until the boss has priority in `step` of `turn`.
fn boss_priority(driver: &mut Driver, turn: u32, step: Step) -> Result<(), ScriptError> {
    driver.idle_until(turn, step)?;
    while driver.decision()?.player != pid(BOSS) {
        driver.answer_passively()?;
    }
    Ok(())
}

/// What the boss's Normal bot answers now.
fn boss_answer(driver: &Driver) -> Result<Answer, ScriptError> {
    let mut bot = brain::brain(Level::Normal, 7, pid(BOSS), false);
    Ok(bot.choose(driver.decision()?, Some(driver.engine())))
}

/// Whether the boss's bot casts its Satchel card `name` now.
fn casts(driver: &Driver, name: &str) -> Result<bool, ScriptError> {
    let id = driver.state().satchel(pid(BOSS))[0];
    assert_eq!(driver.state().object(id).unwrap().card().name, name);
    Ok(boss_answer(driver)? == Answer::Priority(PriorityAction::CastSpell(id)))
}

/// The moment of a cue case: the scenario around the boss, then where the boss is asked.
#[derive(Debug, Clone, Copy)]
enum Case {
    /// The hero holds a 2/2 (cast) or only a 5/5 (hold); the boss's first main phase.
    Kill(bool),
    /// The hero holds Grizzly Bears (cast) or no creature (hold); the boss's first main phase.
    Eyed(bool),
    /// The boss holds no card (cast) or four (hold); its first main phase.
    HandLow(bool),
    /// The boss has five of each basic land (cast) or one (hold); its first main phase.
    Mana(bool),
    /// The boss's first turn (cast) or its third (hold).
    EarlyTurns(bool),
    /// Grizzly Bears in the boss's graveyard (cast) or none (hold).
    Graveyard(bool),
    /// The hero has three Grizzly Bears against the boss's none (cast) or two (hold).
    BehindOnBoard(bool),
    /// The boss has three Grizzly Bears (cast) or one (hold).
    Board(bool),
    /// The hero's end step (cast) or the boss's (hold).
    HeroesEnd(bool),
    /// The hero casts Serra Angel (cast) or Grizzly Bears (hold).
    Counter(bool),
    /// The hero bolts the boss's Grizzly Bears (cast) or the boss (hold).
    Protect(bool),
    /// The hero attacks with Grizzly Bears, in the declare blockers step (cast); the boss's main phase (hold).
    Combat(bool),
    /// The hero at 4 life against the boss's two Grizzly Bears (cast) or at 20 (hold).
    Lethal(bool),
}

impl Case {
    fn card(self) -> &'static str {
        match self {
            Case::Kill(_) => "Lightning Bolt",
            Case::Eyed(_) => "Swords to Plowshares",
            Case::HandLow(_) => "Opt",
            Case::Mana(_) => "Brainstorm",
            Case::EarlyTurns(_) => "Consider",
            Case::Graveyard(_) => "Peek",
            Case::BehindOnBoard(_) => "Doom Blade",
            Case::Board(_) => "Village Rites",
            Case::HeroesEnd(_) => "Healing Salve",
            Case::Counter(_) => "Counterspell",
            Case::Protect(_) => "Memory Lapse",
            Case::Combat(_) => "Giant Growth",
            Case::Lethal(_) => "Rampant Growth",
        }
    }

    fn fires(self) -> bool {
        match self {
            Case::Kill(f)
            | Case::Eyed(f)
            | Case::HandLow(f)
            | Case::Mana(f)
            | Case::EarlyTurns(f)
            | Case::Graveyard(f)
            | Case::BehindOnBoard(f)
            | Case::Board(f)
            | Case::HeroesEnd(f)
            | Case::Counter(f)
            | Case::Protect(f)
            | Case::Combat(f)
            | Case::Lethal(f) => f,
        }
    }

    /// What the case starts with besides its lands and Satchel.
    fn board(self) -> Board {
        let f = self.fires();
        let bears = |count: usize| copies("Grizzly Bears", count);
        let cub = |toughness: i32| vec![ready(free_creature("Cub", 2, toughness))];
        match self {
            Case::Kill(_) => Board {
                hero: cub(if f { 2 } else { 5 }),
                ..Board::default()
            },
            Case::Eyed(_) | Case::Combat(_) => Board {
                hero: bears(usize::from(f)),
                ..Board::default()
            },
            Case::Mana(_) => Board {
                lands: if f { 5 } else { 1 },
                ..Board::default()
            },
            Case::BehindOnBoard(_) => Board {
                hero: bears(3),
                boss: bears(if f { 0 } else { 2 }),
                ..Board::default()
            },
            Case::Board(_) => Board {
                boss: bears(if f { 3 } else { 1 }),
                ..Board::default()
            },
            Case::Counter(_) | Case::Protect(_) => Board {
                boss: bears(1),
                ..Board::default()
            },
            Case::Lethal(_) => Board {
                boss: bears(2),
                ..Board::default()
            },
            _ => Board::default(),
        }
    }

    fn scenario(self) -> Scenario {
        let f = self.fires();
        let base = fight(self.card(), true, self.board());
        match self {
            Case::HandLow(_) if !f => {
                base.hand(BOSS, (0..4).map(|_| card("Grizzly Bears").unwrap()))
            }
            Case::Graveyard(_) if f => base.graveyard(BOSS, [card("Grizzly Bears").unwrap()]),
            Case::Counter(_) | Case::Protect(_) => base.hand(
                HERO,
                ["Serra Angel", "Grizzly Bears", "Lightning Bolt"].map(|name| card(name).unwrap()),
            ),
            Case::Lethal(_) => base.life(HERO, if f { 4 } else { 20 }),
            _ => base,
        }
    }

    /// Brings the game to the moment the boss is asked.
    fn reach(self, driver: &mut Driver) -> Result<(), ScriptError> {
        let f = self.fires();
        match self {
            Case::EarlyTurns(_) => boss_priority(driver, if f { 2 } else { 6 }, Step::Main1),
            Case::HeroesEnd(_) => boss_priority(driver, if f { 1 } else { 2 }, Step::End),
            Case::Counter(_) => {
                driver.idle_until(1, Step::Main1)?;
                driver.cast(if f { "Serra Angel" } else { "Grizzly Bears" })?;
                driver.pass()?;
                Ok(())
            }
            Case::Protect(_) => {
                driver.idle_until(1, Step::Main1)?;
                let at = if f {
                    driver.target_named("Grizzly Bears")?
                } else {
                    driver.target_player(BOSS)
                };
                driver.cast_at("Lightning Bolt", &[at])?;
                driver.pass()?;
                Ok(())
            }
            Case::Combat(_) if f => {
                driver.idle_until(1, Step::DeclareAttackers)?;
                driver.attack(&[("Grizzly Bears", BOSS)])?;
                boss_priority(driver, 1, Step::DeclareBlockers)
            }
            _ => boss_priority(driver, 2, Step::Main1),
        }
    }
}

#[rstest]
fn the_boss_casts_at_the_cue_and_holds_otherwise(
    #[values(
        Case::Kill(true),
        Case::Kill(false),
        Case::Eyed(true),
        Case::Eyed(false),
        Case::HandLow(true),
        Case::HandLow(false),
        Case::Mana(true),
        Case::Mana(false),
        Case::EarlyTurns(true),
        Case::EarlyTurns(false),
        Case::Graveyard(true),
        Case::Graveyard(false),
        Case::BehindOnBoard(true),
        Case::BehindOnBoard(false),
        Case::Board(true),
        Case::Board(false),
        Case::HeroesEnd(true),
        Case::HeroesEnd(false),
        Case::Counter(true),
        Case::Counter(false),
        Case::Protect(true),
        Case::Protect(false),
        Case::Combat(true),
        Case::Combat(false),
        Case::Lethal(true),
        Case::Lethal(false)
    )]
    case: Case,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(case.scenario())?;
    case.reach(&mut driver)?;
    assert_eq!(casts(&driver, case.card())?, case.fires(), "{case:?}");
    Ok(())
}

#[rstest]
#[case::first_cast_is_free(0, 1, SatchelCue::Mana { mana: 0 }, None, false)]
#[case::tax_up_to_twice_the_cost(2, 1, SatchelCue::Mana { mana: 0 }, None, false)]
#[case::tax_past_twice_the_cost(4, 1, SatchelCue::Mana { mana: 0 }, None, true)]
#[case::urgent_pays_two_more(4, 1, SatchelCue::Protect, None, false)]
#[case::urgent_has_a_limit_too(6, 1, SatchelCue::Protect, None, true)]
#[case::hand_spell_comes_first(2, 3, SatchelCue::Mana { mana: 0 }, Some(3), true)]
#[case::small_hand_spell_does_not(2, 3, SatchelCue::Mana { mana: 0 }, Some(2), false)]
#[case::hand_never_stops_an_answer(2, 3, SatchelCue::Counter { mana_value: 1 }, Some(5), false)]
#[case::hand_does_not_matter_untaxed(0, 3, SatchelCue::Mana { mana: 0 }, Some(5), false)]
fn the_tax_is_weighed(
    #[case] tax: u32,
    #[case] mana_value: u32,
    #[case] cue: SatchelCue,
    #[case] hand_best: Option<u32>,
    #[case] held: bool,
) {
    assert_eq!(satchel::taxed_out(tax, mana_value, cue, hand_best), held);
}

/// Plays the boss with its Normal bot and the hero passively through turn `last`; the times the Satchel card was cast.
fn play_through(driver: &mut Driver, last: u32) -> Result<usize, ScriptError> {
    let id = driver.state().satchel(pid(BOSS))[0];
    let mut bot = brain::brain(Level::Normal, 7, pid(BOSS), false);
    let mut cast = 0;
    while driver.state().turn().number() <= last && driver.state().result().is_none() {
        let decision = driver.decision()?.clone();
        let answer = if decision.player == pid(BOSS) {
            bot.choose(&decision, Some(driver.engine()))
        } else {
            Driver::passive_answer(&decision)
        };
        for event in driver.answer(answer)? {
            if matches!(event, Event::SpellCast { object, .. } if object == id) {
                cast += 1;
            }
        }
    }
    Ok(cast)
}

#[rstest]
fn a_reusable_card_is_recast_while_its_tax_is_worth_it_and_a_once_card_is_cast_once(
    #[values(true, false)] reusable: bool,
) -> Result<(), ScriptError> {
    let board = Board {
        hero: (0..6)
            .map(|at| ready(free_creature(&format!("Cub {at}"), 2, 2)))
            .collect(),
        ..Board::default()
    };
    let mut driver = Driver::from_scenario(fight("Lightning Bolt", reusable, board))?;
    let casts = play_through(&mut driver, 6)?;
    // Bolt costs 1: the first two casts (tax 0 and 2) are worth it, the third (tax 4) is not.
    assert_eq!(casts, if reusable { 2 } else { 1 });
    Ok(())
}
