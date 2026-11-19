//! The Learn course's lessons as data: each one's table, the stops its learner holds, the bot's script and the
//! learner's script (`docs/tutorial.md`, Scripted table and States).

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::setup::{GameSetup, PlayerSetup, PresetPermanent, Start};
use mtg_engine::turn::Step;
use mtg_protocol::StopSide;

use super::{BOT_SEAT, PLAYER_SEAT, TutorialError, card};
use crate::recipe::FormatKind;

/// What the learner does at one decision, as the web's coach lets it through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Keeps the opening hand.
    Keep,
    /// Plays the land of this name.
    Land(&'static str),
    /// Casts the spell of this name, from the hand or the command zone.
    Cast(&'static str),
    /// Passes priority.
    Pass,
    /// Lets the game pay (Pay for me).
    PayAuto,
    /// Taps these sources by hand, one per name, then confirms.
    PayTap(&'static [&'static str]),
    /// Attacks the bot with these creatures.
    Attack(&'static [&'static str]),
    /// Blocks the attacker (second name) with the blocker (first name).
    Block(&'static str, &'static str),
    /// Targets the learner's permanent of this name.
    Target(&'static str),
    /// Answers a yes-or-no question.
    Choose(bool),
    /// Sets a lasting stop; the decision stays pending.
    SetStop(StopSide, Step),
    /// End turn: passes until the end of the turn.
    EndTurn,
}

/// The kind of a decision the learner is sent, named as the web's `decisionName` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Keep or mulligan.
    Mulligan,
    /// Priority.
    Priority,
    /// A mana payment.
    PayMana,
    /// Declaring attackers.
    DeclareAttackers,
    /// Declaring blockers.
    DeclareBlockers,
    /// Choosing targets.
    ChooseTargets,
    /// A yes-or-no question.
    YesNo,
}

/// One line of the learner's script: the turn, the step, the decision kind and what the learner does.
pub type Line = (u32, Step, Kind, Act);

/// What the bot does at one moment of its script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotAct {
    /// Keeps the opening hand.
    Keep,
    /// Plays the land of this name.
    Land(&'static str),
    /// Casts the spell of this name, aimed at the learner's permanent of the second name when it targets.
    Cast(&'static str, Option<&'static str>),
    /// Attacks the learner with these creatures.
    Attack(&'static [&'static str]),
    /// Blocks the attacker (second name) with the blocker (first name).
    Block(&'static str, &'static str),
}

/// One line of the bot's script: the turn, the step and the act.
pub type BotLine = (u32, Step, BotAct);

/// One lesson of the course.
#[derive(Debug, Clone, Copy)]
pub struct Lesson {
    /// The lesson's id, shared with the web.
    pub id: &'static str,
    /// Builds the lesson's table.
    pub setup: fn() -> Result<GameSetup, TutorialError>,
    /// The stops the learner holds from the start, each kept until cleared.
    pub stops: &'static [(StopSide, Step)],
    /// The bot's script.
    pub bot: &'static [BotLine],
    /// The learner's script: every decision the learner is sent, in order.
    pub script: &'static [Line],
}

/// The course, in its order.
pub const LESSONS: [Lesson; 7] = [SEAT, MANA, COMBAT, STACK, TURN, COMMANDER_LESSON, HELP];

/// The lesson called `id`.
pub fn lesson(id: &str) -> Option<&'static Lesson> {
    LESSONS.iter().find(|lesson| lesson.id == id)
}

/// The seed; a lesson's table depends on no shuffle, so it only fixes what a card's own randomness does.
const SEED: u64 = 1;

/// `count` copies of the card called `name`.
fn copies(name: &'static str, count: usize) -> Result<Vec<Arc<CardDefinition>>, TutorialError> {
    Ok(vec![card(name)?; count])
}

/// The cards called `names`, in order.
fn cards(names: &[&'static str]) -> Result<Vec<Arc<CardDefinition>>, TutorialError> {
    names.iter().map(|name| card(name)).collect()
}

/// Untapped, ready permanents of the cards called `names`.
fn ready(names: &[&'static str]) -> Result<Vec<PresetPermanent>, TutorialError> {
    Ok(cards(names)?
        .into_iter()
        .map(PresetPermanent::new)
        .collect())
}

/// A permanent that entered this turn.
fn sick(name: &'static str) -> Result<PresetPermanent, TutorialError> {
    Ok(PresetPermanent {
        summoning_sick: true,
        ..PresetPermanent::new(card(name)?)
    })
}

/// A two-seat Commander table of `learner` and `bot` that starts at `start`.
fn table(learner: PlayerSetup, bot: PlayerSetup, start: Start) -> GameSetup {
    let mut setup = GameSetup::new(FormatKind::Commander.rules(), SEED, Vec::new());
    setup.players = vec![learner, bot];
    setup.start = start;
    setup.validate_decks = false;
    setup
}

/// The start of `step` on `turn`: the learner's turns are the odd ones.
fn at(turn: u32, step: Step) -> Start {
    let active = if turn % 2 == 1 { PLAYER_SEAT } else { BOT_SEAT };
    Start::At { turn, active, step }
}

/// The bot's seat: its library of Swamps, its hand and its permanents.
fn bot(
    hand: &[&'static str],
    battlefield: &[&'static str],
    life: Option<i32>,
) -> Result<PlayerSetup, TutorialError> {
    Ok(PlayerSetup {
        library: copies("Swamp", 20)?,
        hand: cards(hand)?,
        battlefield: ready(battlefield)?,
        life,
        ..PlayerSetup::default()
    })
}

const SEAT: Lesson = Lesson {
    id: "seat",
    setup: seat_setup,
    stops: &[],
    bot: &[
        (0, Step::Untap, BotAct::Keep),
        (2, Step::Main1, BotAct::Land("Swamp")),
    ],
    script: &[
        (0, Step::Untap, Kind::Mulligan, Act::Keep),
        (1, Step::Main1, Kind::Priority, Act::Land("Forest")),
        (1, Step::Main1, Kind::Priority, Act::Pass),
        (1, Step::Main2, Kind::Priority, Act::Pass),
    ],
};

/// The opening: a hand of Forest ×3, Plains, Grizzly Bears, Llanowar Elves and Giant Growth over Forests.
fn seat_setup() -> Result<GameSetup, TutorialError> {
    let mut library = cards(&[
        "Forest",
        "Forest",
        "Forest",
        "Plains",
        "Grizzly Bears",
        "Llanowar Elves",
        "Giant Growth",
    ])?;
    library.extend(copies("Forest", 20)?);
    let learner = PlayerSetup {
        library,
        ..PlayerSetup::default()
    };
    let bot = PlayerSetup {
        library: copies("Swamp", 27)?,
        ..PlayerSetup::default()
    };
    let mut setup = table(learner, bot, Start::Mulligan);
    setup.stacked = true;
    Ok(setup)
}

const MANA: Lesson = Lesson {
    id: "mana",
    setup: mana_setup,
    stops: &[(StopSide::Own, Step::Main2)],
    bot: &[
        (4, Step::Main1, BotAct::Land("Swamp")),
        (4, Step::Main1, BotAct::Cast("Dregscape Zombie", None)),
    ],
    script: &[
        (3, Step::Main1, Kind::Priority, Act::Land("Forest")),
        (3, Step::Main1, Kind::Priority, Act::Cast("Grizzly Bears")),
        (
            3,
            Step::Main1,
            Kind::PayMana,
            Act::PayTap(&["Forest", "Forest"]),
        ),
        (3, Step::Main1, Kind::Priority, Act::Cast("Llanowar Elves")),
        (3, Step::Main1, Kind::PayMana, Act::PayAuto),
        (3, Step::Main2, Kind::Priority, Act::Pass),
    ],
};

/// Turn 3: two Forests in play, Forest, Grizzly Bears and Llanowar Elves in hand.
fn mana_setup() -> Result<GameSetup, TutorialError> {
    let learner = PlayerSetup {
        library: copies("Forest", 20)?,
        hand: cards(&["Forest", "Grizzly Bears", "Llanowar Elves"])?,
        battlefield: ready(&["Forest", "Forest"])?,
        ..PlayerSetup::default()
    };
    let bot = bot(&["Swamp", "Dregscape Zombie"], &["Swamp"], None)?;
    Ok(table(learner, bot, at(3, Step::Main1)))
}

/// The bot's life in the combat lesson: two attacks of the learner win.
pub const COMBAT_BOT_LIFE: i32 = 8;

const COMBAT: Lesson = Lesson {
    id: "combat",
    setup: combat_setup,
    stops: &[
        (StopSide::Own, Step::Main1),
        (StopSide::Own, Step::DeclareAttackers),
        (StopSide::Own, Step::Main2),
    ],
    bot: &[(
        6,
        Step::DeclareAttackers,
        BotAct::Attack(&["Dregscape Zombie", "Dregscape Zombie"]),
    )],
    script: &[
        (5, Step::Main1, Kind::Priority, Act::Pass),
        (
            5,
            Step::DeclareAttackers,
            Kind::DeclareAttackers,
            Act::Attack(&["Grizzly Bears"]),
        ),
        (5, Step::DeclareAttackers, Kind::Priority, Act::Pass),
        (5, Step::Main2, Kind::Priority, Act::Pass),
        (
            6,
            Step::DeclareBlockers,
            Kind::DeclareBlockers,
            Act::Block("Serra Angel", "Dregscape Zombie"),
        ),
        (7, Step::Main1, Kind::Priority, Act::Pass),
        (
            7,
            Step::DeclareAttackers,
            Kind::DeclareAttackers,
            Act::Attack(&["Grizzly Bears", "Serra Angel"]),
        ),
        (7, Step::DeclareAttackers, Kind::Priority, Act::Pass),
    ],
};

/// Turn 5: four lands, Grizzly Bears and a Serra Angel that arrived this turn, against two Zombies and 8 life.
fn combat_setup() -> Result<GameSetup, TutorialError> {
    let mut battlefield = ready(&["Forest", "Forest", "Forest", "Plains", "Grizzly Bears"])?;
    battlefield.push(sick("Serra Angel")?);
    let learner = PlayerSetup {
        library: copies("Forest", 20)?,
        battlefield,
        ..PlayerSetup::default()
    };
    let bot = bot(
        &[],
        &["Swamp", "Swamp", "Dregscape Zombie", "Dregscape Zombie"],
        Some(COMBAT_BOT_LIFE),
    )?;
    Ok(table(learner, bot, at(5, Step::Main1)))
}

const STACK: Lesson = Lesson {
    id: "stack",
    setup: stack_setup,
    stops: &[],
    bot: &[(
        6,
        Step::DeclareAttackers,
        BotAct::Attack(&["Dregscape Zombie"]),
    )],
    script: &[
        (
            5,
            Step::Main2,
            Kind::Priority,
            Act::Cast("Hero in Training"),
        ),
        (5, Step::Main2, Kind::PayMana, Act::PayAuto),
        (5, Step::Main2, Kind::Priority, Act::Pass),
        (5, Step::Main2, Kind::Priority, Act::Pass),
        (5, Step::Main2, Kind::Priority, Act::Pass),
        (6, Step::DeclareAttackers, Kind::Priority, Act::Pass),
        (
            6,
            Step::DeclareBlockers,
            Kind::DeclareBlockers,
            Act::Block("Grizzly Bears", "Dregscape Zombie"),
        ),
        (
            6,
            Step::DeclareBlockers,
            Kind::Priority,
            Act::Cast("Giant Growth"),
        ),
        (
            6,
            Step::DeclareBlockers,
            Kind::ChooseTargets,
            Act::Target("Grizzly Bears"),
        ),
        (6, Step::DeclareBlockers, Kind::PayMana, Act::PayAuto),
    ],
};

/// Turn 5, second main phase: Forest ×3, Plains and Grizzly Bears; Hero in Training ({2}{W}) and Giant Growth in
/// hand, so one Forest stays open.
fn stack_setup() -> Result<GameSetup, TutorialError> {
    let learner = PlayerSetup {
        library: copies("Forest", 20)?,
        hand: cards(&["Hero in Training", "Giant Growth"])?,
        battlefield: ready(&["Forest", "Forest", "Forest", "Plains", "Grizzly Bears"])?,
        ..PlayerSetup::default()
    };
    let bot = bot(&[], &["Swamp", "Swamp", "Dregscape Zombie"], None)?;
    Ok(table(learner, bot, at(5, Step::Main2)))
}

const TURN: Lesson = Lesson {
    id: "turn",
    setup: turn_setup,
    stops: &[],
    bot: &[(6, Step::Main1, BotAct::Land("Swamp"))],
    script: &[
        (
            5,
            Step::Main1,
            Kind::Priority,
            Act::SetStop(StopSide::Opponent, Step::End),
        ),
        (5, Step::Main1, Kind::Priority, Act::EndTurn),
        (6, Step::End, Kind::Priority, Act::Pass),
    ],
};

/// Turn 5: four lands and a Grizzly Bears that arrived this turn; Giant Growth in hand.
fn turn_setup() -> Result<GameSetup, TutorialError> {
    let mut battlefield = ready(&["Forest", "Forest", "Forest", "Plains"])?;
    battlefield.push(sick("Grizzly Bears")?);
    let learner = PlayerSetup {
        library: copies("Forest", 20)?,
        hand: cards(&["Giant Growth"])?,
        battlefield,
        ..PlayerSetup::default()
    };
    let bot = bot(&["Swamp"], &["Swamp", "Swamp"], None)?;
    Ok(table(learner, bot, at(5, Step::Main1)))
}

/// The learner's commander.
pub const COMMANDER: &str = "Giada, Font of Hope";

const COMMANDER_LESSON: Lesson = Lesson {
    id: "commander",
    setup: commander_setup,
    stops: &[(StopSide::Own, Step::Main2)],
    bot: &[(6, Step::Main1, BotAct::Cast("Doom Blade", Some(COMMANDER)))],
    script: &[
        (5, Step::Main1, Kind::Priority, Act::Cast(COMMANDER)),
        (5, Step::Main1, Kind::PayMana, Act::PayAuto),
        (5, Step::Main2, Kind::Priority, Act::Pass),
        (6, Step::Main1, Kind::YesNo, Act::Choose(true)),
        (7, Step::Main1, Kind::Priority, Act::Cast(COMMANDER)),
        (7, Step::Main1, Kind::PayMana, Act::PayAuto),
    ],
};

/// Turn 5: Plains ×3 and a Forest, Giada in the command zone, a library of Serra Angels the lands cannot cast.
fn commander_setup() -> Result<GameSetup, TutorialError> {
    let learner = PlayerSetup {
        library: copies("Serra Angel", 20)?,
        battlefield: ready(&["Plains", "Plains", "Plains", "Forest"])?,
        commanders: vec![card(COMMANDER)?],
        ..PlayerSetup::default()
    };
    let bot = bot(&["Doom Blade"], &["Swamp", "Swamp", "Swamp"], None)?;
    Ok(table(learner, bot, at(5, Step::Main1)))
}

const HELP: Lesson = Lesson {
    id: "help",
    setup: help_setup,
    stops: &[],
    bot: &[
        (4, Step::Main1, BotAct::Land("Swamp")),
        (4, Step::Main1, BotAct::Cast("Dregscape Zombie", None)),
    ],
    script: &[],
};

/// The end step of turn 3, so the log holds the bot's turn 4 and the learner's turn 5: four lands, Serra Angel and
/// Grizzly Bears; the turn-5 draw is a Forest.
fn help_setup() -> Result<GameSetup, TutorialError> {
    let learner = PlayerSetup {
        library: copies("Forest", 20)?,
        battlefield: ready(&[
            "Forest",
            "Forest",
            "Forest",
            "Plains",
            "Serra Angel",
            "Grizzly Bears",
        ])?,
        ..PlayerSetup::default()
    };
    let bot = bot(&["Swamp", "Dregscape Zombie"], &["Swamp"], None)?;
    Ok(table(learner, bot, at(3, Step::End)))
}
