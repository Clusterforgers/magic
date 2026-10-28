//! Goal: boards of combat with replacement and prevention effects in play, and the real damage they lead to (this is not a test module itself).
#![allow(clippy::expect_used)]

use mtg_cards::refs::Selector;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{OptionChoice, ReplacementChoice, Target};
use mtg_engine::decision::{Answer, DamageOptions, DecisionKind, PriorityAction};
use mtg_engine::effects::Duration;
use mtg_engine::forecast::{CombatForecast, pending_combat_forecast};
use mtg_engine::rules::{DamageShield, Rule};
use mtg_engine::turn::Step;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::creature;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};

use super::combat::to_blockers;
use super::forecast::damage_was_dealt;
use super::pid;

/// One permanent of a board.
#[derive(Debug, Clone, Copy)]
pub enum Piece {
    /// A creature that costs nothing: name, power, toughness and colour.
    Body(&'static str, i32, i32, Option<Color>),
    /// A card of the corpus, by name.
    Card(&'static str),
}

/// A shield that is in force when the blockers have been declared.
#[derive(Debug, Clone, Copy)]
pub enum Shield {
    /// Prevents the next damage, this much in all, that would be dealt to player 1.
    ToDefender(u32),
    /// Prevents all combat damage (Fog).
    Fog,
    /// Regenerates the named creature once.
    Regenerate(&'static str),
}

/// A two-player combat: player 0 attacks player 1 with the named creatures, who blocks as told.
#[derive(Debug, Clone)]
pub struct Board {
    pub mine: Vec<Piece>,
    pub theirs: Vec<Piece>,
    pub attack: Vec<&'static str>,
    pub blocks: Vec<(&'static str, &'static str)>,
    pub shields: Vec<Shield>,
    pub life: i32,
}

impl Board {
    /// `attack` of `mine` attacks and `blocks` (blocker, attacker) are declared; player 1 has 20 life.
    pub fn new(
        mine: &[Piece],
        theirs: &[Piece],
        attack: &[&'static str],
        blocks: &[(&'static str, &'static str)],
    ) -> Board {
        Board {
            mine: mine.to_vec(),
            theirs: theirs.to_vec(),
            attack: attack.to_vec(),
            blocks: blocks.to_vec(),
            shields: Vec::new(),
            life: 20,
        }
    }

    /// The board with a shield in force.
    pub fn shielded(mut self, shield: Shield) -> Board {
        self.shields.push(shield);
        self
    }

    /// The board with player 1 at this life.
    pub fn at_life(mut self, life: i32) -> Board {
        self.life = life;
        self
    }

    /// The scenario: attackers declared next, nothing else done.
    pub fn scenario(&self) -> Scenario {
        let permanents = |pieces: &[Piece]| -> Vec<PresetPermanent> {
            pieces
                .iter()
                .map(|piece| match *piece {
                    Piece::Body(name, power, toughness, color) => {
                        ready(creature(name, 0, color.as_slice(), power, toughness))
                    }
                    Piece::Card(name) => ready(card(name).expect("a card of the corpus")),
                })
                .collect()
        };
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, permanents(&self.mine))
            .battlefield(1, permanents(&self.theirs))
            .life(1, self.life)
    }

    /// The game with the attack declared and the shields in force, at the decision of the blockers when anyone can block.
    pub fn driver_facing_blocks(&self) -> Result<Driver, ScriptError> {
        let mut driver = Driver::from_scenario(self.scenario())?;
        let attacks: Vec<(&str, usize)> = self.attack.iter().map(|name| (*name, 1)).collect();
        driver.attack(&attacks)?;
        to_blockers(&mut driver, 1)?;
        for shield in &self.shields {
            let source = driver.id_of(self.attack[0])?;
            let rule = match *shield {
                Shield::ToDefender(left) => Rule::PreventDamage(Box::new(DamageShield {
                    to: Some(vec![Target::Player(pid(1))]),
                    source: Selector::default(),
                    from: None,
                    combat: None,
                    left: Some(left),
                    then: Vec::new(),
                })),
                Shield::Fog => Rule::PreventDamage(Box::new(DamageShield {
                    to: None,
                    source: Selector::default(),
                    from: None,
                    combat: Some(true),
                    left: None,
                    then: Vec::new(),
                })),
                Shield::Regenerate(name) => {
                    let id = driver.id_of(name)?;
                    let object = driver.state().object(id).expect("the creature").reference();
                    Rule::Regenerate(object)
                }
            };
            driver.add_rule_from(source, rule, Duration::UntilEndOfTurn)?;
        }
        Ok(driver)
    }

    /// The game with the blockers declared too: damage is ahead.
    pub fn driver_before_damage(&self) -> Result<Driver, ScriptError> {
        let mut driver = self.driver_facing_blocks()?;
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            driver.block(&self.blocks)?;
        }
        Ok(driver)
    }
}

/// The forecast the game shows right now.
pub fn forecast_now(driver: &Driver) -> CombatForecast {
    pending_combat_forecast(driver.state()).expect("damage is ahead")
}

/// The default division of damage, written out as the rule: lethal damage to each blocker in order, the rest to the
/// player attacked with trample and else on the last blocker. A division is made before the replacement effects.
pub fn lethal_first(options: &DamageOptions) -> Vec<(ObjectId, u32)> {
    let mut left = options.power;
    let mut shares = Vec::new();
    for (at, blocker) in options.blockers.iter().enumerate() {
        let last = at + 1 == options.blockers.len();
        let share = if last && options.excess.is_none() {
            left
        } else {
            left.min(options.lethal[at])
        };
        left -= share;
        shares.push((*blocker, share));
    }
    shares
}

/// Plays the damage steps for real, answering each question of the replacement step with the option at `pick` (the
/// last one when there are fewer) and each division with the default one; returns every event from the first answer
/// to the end of the regular damage step and the state-based actions after it.
pub fn play_out(driver: &mut Driver, pick: usize) -> Result<Vec<Event>, ScriptError> {
    let mut all = Vec::new();
    let mut dealt = false;
    for _ in 0..80 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(all);
        };
        let answer = match &decision.kind {
            DecisionKind::ChooseReplacement(options) => Answer::ChooseReplacement(
                ReplacementChoice(pick.min(options.candidates.len().saturating_sub(1))),
            ),
            DecisionKind::ChooseOption(options) => Answer::ChooseOption(OptionChoice(
                pick.min(options.options.len().saturating_sub(1)),
            )),
            DecisionKind::AssignDamage(options) => Answer::AssignDamage(lethal_first(options)),
            _ => Answer::Priority(PriorityAction::Pass),
        };
        let step = driver.state().turn().step();
        let events = driver.answer(answer)?;
        dealt |= damage_was_dealt(step, &events);
        all.extend(events);
        // The state-based actions after the damage may ask about the order of effects too: wait for their end.
        if dealt && !driver.engine().replacing() {
            return Ok(all);
        }
    }
    Err(ScriptError::Stuck("the combat damage".into()))
}

/// What the game looks like once the damage is done: both life totals and the creatures left, by name.
pub fn outcome(driver: &Driver) -> (i32, i32, Vec<String>) {
    let life = |seat: usize| {
        driver
            .state()
            .player(pid(seat))
            .map_or(0, mtg_engine::player::Player::life)
    };
    let mut left = super::combat::survivors(driver);
    left.sort();
    (life(0), life(1), left)
}
