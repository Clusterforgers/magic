//! A scripted-answer driver: feed answers by card name, then assert on state and events.

use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{
    CardsChoice, CostsChoice, DivideChoice, ModeChoice, NumberChoice, OptionChoice, OrderChoice,
    ReplacementChoice, Target, TargetChoice, TriggersChoice, TriggersOptions, YesNoChoice,
};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{
    Answer, DecisionKind, MulliganChoice, PayChoice, PayOptions, PriorityAction, PriorityOptions,
};
use mtg_engine::effects::{Duration, Modification, Selector};
use mtg_engine::help::Convoked;
use mtg_engine::mana::{ManaSource, Payment};
use mtg_engine::manual::ManualAction;
use mtg_engine::rules::Rule;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine, EngineError, Event, GameState};

use crate::invariants::InvariantChecker;
use crate::scenario::Scenario;
use crate::script_error::ScriptError;

/// Safety valve for loops that wait for a step to arrive.
const MAX_WAIT_ANSWERS: usize = 20_000;

/// Drives an [`Engine`] with scripted answers and checks invariants after every answer.
#[derive(Debug)]
pub struct Driver {
    pub(crate) engine: Engine,
    checker: InvariantChecker,
}

impl Driver {
    /// Wraps an engine that has just been created.
    pub fn new(engine: Engine) -> Result<Self, ScriptError> {
        let mut checker = InvariantChecker::new(engine.setup())?;
        checker.observe_engine(&engine)?;
        Ok(Driver { engine, checker })
    }

    /// Builds a scenario and wraps the resulting engine.
    pub fn from_scenario(scenario: Scenario) -> Result<Self, ScriptError> {
        Driver::new(scenario.build()?)
    }

    /// The engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// The current state.
    pub fn state(&self) -> &GameState {
        self.engine.state()
    }

    /// The pending decision.
    pub fn decision(&self) -> Result<&Decision, ScriptError> {
        self.engine
            .decision()
            .ok_or(ScriptError::Engine(EngineError::NoDecision))
    }

    /// Applies a raw answer and checks invariants; returns the events it produced.
    pub fn answer(&mut self, answer: Answer) -> Result<Vec<Event>, ScriptError> {
        let events = self.engine.apply(answer)?;
        self.checker.observe_engine(&self.engine)?;
        Ok(events)
    }

    /// A player concedes and the invariants are checked; returns the events it produced.
    pub fn concede(&mut self, player: PlayerId) -> Result<Vec<Event>, ScriptError> {
        let events = self.engine.concede(player)?;
        self.checker.observe_engine(&self.engine)?;
        Ok(events)
    }

    /// Applies several raw answers in order.
    pub fn answers(
        &mut self,
        answers: impl IntoIterator<Item = Answer>,
    ) -> Result<(), ScriptError> {
        for answer in answers {
            self.answer(answer)?;
        }
        Ok(())
    }

    pub(crate) fn name_of(&self, id: ObjectId) -> &str {
        self.state()
            .object(id)
            .map_or("?", |o| o.card().name.as_str())
    }

    fn priority(&self) -> Result<(PlayerId, &PriorityOptions), ScriptError> {
        let decision = self.decision()?;
        match &decision.kind {
            DecisionKind::Priority(options) => Ok((decision.player, options)),
            other => Err(ScriptError::UnexpectedDecision {
                wanted: "priority",
                got: other.name().to_owned(),
            }),
        }
    }

    fn find(
        &self,
        ids: impl IntoIterator<Item = ObjectId>,
        name: &str,
        place: &'static str,
    ) -> Result<ObjectId, ScriptError> {
        ids.into_iter()
            .find(|id| self.name_of(*id) == name)
            .ok_or_else(|| ScriptError::NoSuchCard {
                name: name.to_owned(),
                place,
            })
    }

    /// The id of the first battlefield permanent with this name.
    pub fn id_of(&self, name: &str) -> Result<ObjectId, ScriptError> {
        self.find(
            self.state().zone(Zone::Battlefield).iter().copied(),
            name,
            "the battlefield",
        )
    }

    /// Registers a continuous effect through the engine's hook and checks invariants.
    pub fn add_effect(
        &mut self,
        selector: Selector,
        modification: Modification,
        duration: Duration,
    ) -> Result<Vec<Event>, ScriptError> {
        let events = self
            .engine
            .add_continuous_effect(selector, modification, duration)?;
        self.checker.observe_engine(&self.engine)?;
        Ok(events)
    }

    /// Registers a continuous effect as if a spell or ability of `source` had created it, and checks invariants.
    pub fn add_effect_from(
        &mut self,
        source: ObjectId,
        selector: Selector,
        modification: Modification,
        duration: Duration,
    ) -> Result<Vec<Event>, ScriptError> {
        let events =
            self.engine
                .add_continuous_effect_from(source, selector, modification, duration)?;
        self.checker.observe_engine(&self.engine)?;
        Ok(events)
    }

    /// Registers a change to a rule of the game through the engine's hook and checks invariants.
    pub fn add_rule(&mut self, rule: Rule, duration: Duration) -> Result<Vec<Event>, ScriptError> {
        let events = self.engine.add_rule(rule, duration)?;
        self.checker.observe_engine(&self.engine)?;
        Ok(events)
    }

    /// Registers a change to a rule of the game as if a spell or ability of `source` had made it, and checks invariants.
    pub fn add_rule_from(
        &mut self,
        source: ObjectId,
        rule: Rule,
        duration: Duration,
    ) -> Result<Vec<Event>, ScriptError> {
        let events = self.engine.add_rule_from(source, rule, duration)?;
        self.checker.observe_engine(&self.engine)?;
        Ok(events)
    }

    /// The selector that names the object `id` as it is now: a new object it becomes later is not named.
    pub fn on(&self, id: ObjectId) -> Selector {
        Selector::objects(self.state(), [id])
    }

    /// The player holding priority passes.
    pub fn pass(&mut self) -> Result<Vec<Event>, ScriptError> {
        self.priority()?;
        self.answer(Answer::Priority(PriorityAction::Pass))
    }

    /// The player holding priority takes a manual action by hand.
    pub fn manual(&mut self, action: ManualAction) -> Result<Vec<Event>, ScriptError> {
        self.priority()?;
        self.answer(Answer::Priority(PriorityAction::Manual(action)))
    }

    /// The player holding priority plays the named land from hand.
    pub fn play_land(&mut self, name: &str) -> Result<Vec<Event>, ScriptError> {
        let (_, options) = self.priority()?;
        let id = self.find(options.playable_lands.clone(), name, "playable lands")?;
        self.answer(Answer::Priority(PriorityAction::PlayLand(id)))
    }

    /// The player holding priority taps the named permanent for mana.
    pub fn tap_for_mana(&mut self, name: &str) -> Result<Vec<Event>, ScriptError> {
        let (_, options) = self.priority()?;
        let source = options
            .mana_abilities
            .iter()
            .find(|m| self.name_of(m.source.object) == name)
            .map(|m| m.source)
            .ok_or_else(|| ScriptError::NoSuchCard {
                name: name.to_owned(),
                place: "mana abilities",
            })?;
        self.answer(Answer::Priority(PriorityAction::ActivateMana(source)))
    }

    /// The player holding priority activates the mana ability of the named permanent that adds `mana`: a Treasure
    /// offers one source for each colour.
    pub fn tap_for_mana_of(
        &mut self,
        name: &str,
        mana: mtg_core::mana::ManaType,
    ) -> Result<Vec<Event>, ScriptError> {
        let (_, options) = self.priority()?;
        let source = options
            .mana_abilities
            .iter()
            .find(|m| m.produces == mana && self.name_of(m.source.object) == name)
            .map(|m| m.source)
            .ok_or_else(|| ScriptError::NoSuchCard {
                name: name.to_owned(),
                place: "mana abilities",
            })?;
        self.answer(Answer::Priority(PriorityAction::ActivateMana(source)))
    }

    /// Starts casting the named spell and pays automatically.
    pub fn cast(&mut self, name: &str) -> Result<Vec<Event>, ScriptError> {
        self.begin_cast(name)?;
        self.answer(Answer::PayMana(PayChoice::Auto))
    }

    /// Starts casting the named spell and pays by tapping the named lands.
    pub fn cast_tapping(&mut self, name: &str, lands: &[&str]) -> Result<Vec<Event>, ScriptError> {
        self.cast_paying(name, lands, 0)
    }

    /// Starts casting the named spell and pays by tapping the named lands and paying `life`.
    pub fn cast_paying(
        &mut self,
        name: &str,
        lands: &[&str],
        life: u32,
    ) -> Result<Vec<Event>, ScriptError> {
        self.begin_cast(name)?;
        self.pay_tapping(lands, life)
    }

    /// Answers the pending payment by tapping the named lands and paying `life`.
    pub fn pay_tapping(&mut self, lands: &[&str], life: u32) -> Result<Vec<Event>, ScriptError> {
        let payment = self.payment_tapping(lands, life)?;
        self.answer(Answer::PayMana(PayChoice::Manual(payment)))
    }

    /// Answers the pending payment by tapping the named lands, tapping the named creatures for convoke (each pays {1},
    /// or one mana of the colour given), exiling the named graveyard cards for delve and paying `life`.
    pub fn pay_helped(
        &mut self,
        lands: &[&str],
        convoke: &[(&str, Option<Color>)],
        delve: &[&str],
        life: u32,
    ) -> Result<Vec<Event>, ScriptError> {
        self.pay_helped_by(lands, convoke, &[], delve, life)
    }

    /// Answers the pending payment as `pay_helped` does, and taps the named artifacts for improvise (each pays {1}).
    pub fn pay_helped_by(
        &mut self,
        lands: &[&str],
        convoke: &[(&str, Option<Color>)],
        improvise: &[&str],
        delve: &[&str],
        life: u32,
    ) -> Result<Vec<Event>, ScriptError> {
        let mut payment = self.payment_tapping(lands, life)?;
        let DecisionKind::PayMana(options) = &self.decision()?.kind else {
            return Err(self.unexpected("pay-mana"));
        };
        for (name, pays) in convoke {
            let free = options
                .help
                .convoke
                .iter()
                .map(|o| o.creature)
                .filter(|id| !payment.convoke.iter().any(|c| c.creature == *id));
            let creature = self.find(free, name, "creatures that may convoke")?;
            payment.convoke.push(Convoked {
                creature,
                pays: *pays,
            });
        }
        for name in improvise {
            let free = options
                .help
                .improvise
                .iter()
                .copied()
                .filter(|id| !payment.improvise.contains(id));
            payment
                .improvise
                .push(self.find(free, name, "artifacts that may improvise")?);
        }
        for name in delve {
            let free = options
                .help
                .delve
                .iter()
                .copied()
                .filter(|id| !payment.delve.contains(id));
            payment
                .delve
                .push(self.find(free, name, "cards that may delve")?);
        }
        self.answer(Answer::PayMana(PayChoice::Manual(payment)))
    }

    /// The payment that taps the named lands and pays `life`, for the pending payment decision.
    fn payment_tapping(&self, lands: &[&str], life: u32) -> Result<Payment, ScriptError> {
        let DecisionKind::PayMana(options) = &self.decision()?.kind else {
            return Err(self.unexpected("pay-mana"));
        };
        let mut taps: Vec<ManaSource> = Vec::new();
        for land in lands {
            let free = options
                .sources
                .iter()
                .map(|o| o.source)
                .filter(|s| !taps.contains(s))
                .map(|s| s.object);
            let object = self.find(free, land, "payment sources")?;
            let source = options
                .sources
                .iter()
                .find(|o| o.source.object == object)
                .map(|o| o.source)
                .ok_or_else(|| ScriptError::NoSuchCard {
                    name: (*land).to_owned(),
                    place: "payment sources",
                })?;
            taps.push(source);
        }
        Ok(Payment {
            taps,
            life,
            ..Payment::default()
        })
    }

    /// The pending payment decision's options.
    pub fn pay_options(&self) -> Result<&PayOptions, ScriptError> {
        match &self.decision()?.kind {
            DecisionKind::PayMana(options) => Ok(options),
            _ => Err(self.unexpected("pay-mana")),
        }
    }

    /// Chooses the spell to cast; the payment decision is then pending.
    pub fn begin_cast(&mut self, name: &str) -> Result<(), ScriptError> {
        let (_, options) = self.priority()?;
        let id = self.find(options.castable.clone(), name, "castable spells")?;
        self.answer(Answer::Priority(PriorityAction::CastSpell(id)))?;
        Ok(())
    }

    /// A player as a target.
    pub fn target_player(&self, seat: usize) -> Target {
        Target::Player(PlayerId::from_index(seat))
    }

    /// The battlefield permanent with this name as a target, as it is now.
    pub fn target_named(&self, name: &str) -> Result<Target, ScriptError> {
        let id = self.id_of(name)?;
        self.state()
            .object(id)
            .map(|object| Target::Object(object.reference()))
            .ok_or_else(|| ScriptError::NoSuchCard {
                name: name.to_owned(),
                place: "the battlefield",
            })
    }

    /// Answers the pending target decision with these targets.
    pub fn choose_targets(&mut self, targets: &[Target]) -> Result<Vec<Event>, ScriptError> {
        if !matches!(self.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            return Err(self.unexpected("choose-targets"));
        }
        self.answer(Answer::ChooseTargets(TargetChoice::Targets(
            targets.to_vec(),
        )))
    }

    /// The pending trigger-order decision's options.
    pub fn trigger_options(&self) -> Result<&TriggersOptions, ScriptError> {
        match &self.decision()?.kind {
            DecisionKind::OrderTriggers(options) => Ok(options),
            _ => Err(self.unexpected("order-triggers")),
        }
    }

    /// Answers the pending trigger-order decision: the triggers by position, in the order they go on the stack.
    pub fn order_triggers(&mut self, order: &[usize]) -> Result<Vec<Event>, ScriptError> {
        self.trigger_options()?;
        self.answer(Answer::OrderTriggers(TriggersChoice(order.to_vec())))
    }

    /// Chooses no target for the triggered ability at the pending target decision, which removes it (CR 603.3d).
    pub fn remove_trigger(&mut self) -> Result<Vec<Event>, ScriptError> {
        if !matches!(self.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            return Err(self.unexpected("choose-targets"));
        }
        self.answer(Answer::ChooseTargets(TargetChoice::Remove))
    }

    /// Backs out of the cast or activation at its pending target decision.
    pub fn cancel_targets(&mut self) -> Result<Vec<Event>, ScriptError> {
        if !matches!(self.decision()?.kind, DecisionKind::ChooseTargets(_)) {
            return Err(self.unexpected("choose-targets"));
        }
        self.answer(Answer::ChooseTargets(TargetChoice::Cancel))
    }

    /// Casts the named spell at these targets (one specification) and pays automatically.
    pub fn cast_at(&mut self, name: &str, targets: &[Target]) -> Result<Vec<Event>, ScriptError> {
        self.begin_cast(name)?;
        self.choose_targets(targets)?;
        self.answer(Answer::PayMana(PayChoice::Auto))
    }

    /// Starts activating the first ability of the named permanent that is not a mana ability.
    pub fn begin_activation(&mut self, name: &str) -> Result<Vec<Event>, ScriptError> {
        let (_, options) = self.priority()?;
        let ability = options
            .abilities
            .iter()
            .find(|a| self.name_of(a.object) == name)
            .copied()
            .ok_or_else(|| ScriptError::NoSuchCard {
                name: name.to_owned(),
                place: "activatable abilities",
            })?;
        self.answer(Answer::Priority(PriorityAction::Activate(ability)))
    }

    /// Starts activating the printed ability with this index of the named permanent: the loyalty ability of a planeswalker
    /// among its abilities, in the order of the card.
    pub fn begin_ability(&mut self, name: &str, ability: usize) -> Result<Vec<Event>, ScriptError> {
        let (_, options) = self.priority()?;
        let found = options
            .abilities
            .iter()
            .find(|a| a.ability == ability && self.name_of(a.object) == name)
            .copied()
            .ok_or_else(|| ScriptError::NoSuchCard {
                name: format!("{name} ability {ability}"),
                place: "activatable abilities",
            })?;
        self.answer(Answer::Priority(PriorityAction::Activate(found)))
    }

    /// The indices of the printed abilities of the named permanent that the deciding player may activate now.
    pub fn offered_abilities(&self, name: &str) -> Result<Vec<usize>, ScriptError> {
        let (_, options) = self.priority()?;
        Ok(options
            .abilities
            .iter()
            .filter(|a| self.name_of(a.object) == name)
            .map(|a| a.ability)
            .collect())
    }

    /// The deciding player attacks planeswalkers: each pair is (attacker name, planeswalker name).
    pub fn attack_planeswalkers(
        &mut self,
        attacks: &[(&str, &str)],
    ) -> Result<Vec<Event>, ScriptError> {
        let DecisionKind::DeclareAttackers(options) = &self.decision()?.kind else {
            return Err(self.unexpected("declare-attackers"));
        };
        let mut chosen: Vec<(ObjectId, AttackTarget)> = Vec::new();
        for (name, walker) in attacks {
            let free = options
                .attackers
                .iter()
                .map(|a| a.object)
                .filter(|o| !chosen.iter().any(|(c, _)| c == o));
            let object = self.find(free, name, "legal attackers")?;
            let target = options
                .attackers
                .iter()
                .find(|a| a.object == object)
                .and_then(|a| {
                    a.defenders.iter().copied().find(|defender| {
                        matches!(defender, AttackTarget::Planeswalker(id) if self.name_of(*id) == *walker)
                    })
                })
                .ok_or_else(|| ScriptError::NoSuchCard {
                    name: (*walker).to_owned(),
                    place: "planeswalkers the attacker may attack",
                })?;
            chosen.push((object, target));
        }
        self.answer(Answer::DeclareAttackers(chosen))
    }

    fn unexpected(&self, wanted: &'static str) -> ScriptError {
        ScriptError::UnexpectedDecision {
            wanted,
            got: self
                .engine
                .decision()
                .map_or_else(|| "nothing".to_owned(), |d| d.kind.name().to_owned()),
        }
    }

    /// The deciding player attacks: each pair is (attacker name, defending player index).
    pub fn attack(&mut self, attacks: &[(&str, usize)]) -> Result<Vec<Event>, ScriptError> {
        let DecisionKind::DeclareAttackers(options) = &self.decision()?.kind else {
            return Err(self.unexpected("declare-attackers"));
        };
        let mut chosen: Vec<(ObjectId, AttackTarget)> = Vec::new();
        for (name, defender) in attacks {
            let free = options
                .attackers
                .iter()
                .map(|a| a.object)
                .filter(|o| !chosen.iter().any(|(c, _)| c == o));
            let object = self.find(free, name, "legal attackers")?;
            chosen.push((
                object,
                AttackTarget::Player(PlayerId::from_index(*defender)),
            ));
        }
        self.answer(Answer::DeclareAttackers(chosen))
    }

    /// The deciding player blocks: each pair is (blocker name, attacker name).
    pub fn block(&mut self, blocks: &[(&str, &str)]) -> Result<Vec<Event>, ScriptError> {
        let DecisionKind::DeclareBlockers(options) = &self.decision()?.kind else {
            return Err(self.unexpected("declare-blockers"));
        };
        let mut chosen: Vec<(ObjectId, ObjectId)> = Vec::new();
        for (blocker, attacker) in blocks {
            let free = options
                .blockers
                .iter()
                .map(|b| b.object)
                .filter(|o| !chosen.iter().any(|(c, _)| c == o));
            let blocker_id = self.find(free, blocker, "legal blockers")?;
            let can_block = options
                .blockers
                .iter()
                .find(|b| b.object == blocker_id)
                .map(|b| b.can_block.clone())
                .unwrap_or_default();
            let attacker_id = self.find(can_block, attacker, "attackers the blocker can block")?;
            chosen.push((blocker_id, attacker_id));
        }
        self.answer(Answer::DeclareBlockers(chosen))
    }

    /// The attacker's controller divides damage: each pair is (blocker name, damage).
    pub fn assign_damage(&mut self, shares: &[(&str, u32)]) -> Result<Vec<Event>, ScriptError> {
        let DecisionKind::AssignDamage(options) = &self.decision()?.kind else {
            return Err(self.unexpected("assign-damage"));
        };
        let mut assignment = Vec::new();
        for (name, amount) in shares {
            let free = options
                .blockers
                .iter()
                .copied()
                .filter(|b| !assignment.iter().any(|(c, _)| c == b));
            assignment.push((self.find(free, name, "blockers")?, *amount));
        }
        self.answer(Answer::AssignDamage(assignment))
    }

    /// A passive answer to whatever is pending: pass, keep, attack with nothing, block nothing.
    pub fn passive_answer(decision: &Decision) -> Answer {
        match &decision.kind {
            DecisionKind::Mulligan(_) => Answer::Mulligan(MulliganChoice::Keep),
            DecisionKind::BottomCards(o) => {
                Answer::BottomCards(o.hand.iter().copied().take(o.count).collect())
            }
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Cancel),
            DecisionKind::DeclareAttackers(o) => Answer::DeclareAttackers(o.required_attack()),
            DecisionKind::DeclareBlockers(o) => Answer::DeclareBlockers(o.required_blocks()),
            DecisionKind::AssignDamage(o) => Answer::AssignDamage(
                o.blockers
                    .iter()
                    .enumerate()
                    .map(|(i, b)| (*b, if i == 0 { o.power } else { 0 }))
                    .collect(),
            ),
            DecisionKind::Discard(o) => {
                Answer::Discard(o.hand.iter().copied().take(o.count).collect())
            }
            // Backing out of a cast is the passive way through an announcement; a trigger has no way out.
            DecisionKind::ChooseTargets(o) if o.can_cancel => {
                Answer::ChooseTargets(TargetChoice::Cancel)
            }
            DecisionKind::ChooseTargets(o) => Answer::ChooseTargets(TargetChoice::Targets(
                o.legal.iter().copied().take(o.min as usize).collect(),
            )),
            DecisionKind::ChooseMode(o) if o.can_cancel => Answer::ChooseMode(ModeChoice::Cancel),
            DecisionKind::ChooseMode(o) => Answer::ChooseMode(ModeChoice::Modes(
                o.modes
                    .iter()
                    .take(o.min as usize)
                    .map(|m| m.index)
                    .collect(),
            )),
            // Backing out of a cast is the passive way through an announcement.
            DecisionKind::ChooseNumber(o) if o.can_cancel => {
                Answer::ChooseNumber(NumberChoice::Cancel)
            }
            DecisionKind::ChooseNumber(o) => Answer::ChooseNumber(NumberChoice::Number(o.min)),
            DecisionKind::DivideAmount(o) if o.can_cancel => {
                Answer::DivideAmount(DivideChoice::Cancel)
            }
            // One point each and the rest to the first target.
            DecisionKind::DivideAmount(o) => {
                let others = u32::try_from(o.among.len().saturating_sub(1)).unwrap_or(0);
                Answer::DivideAmount(DivideChoice::Division(
                    o.among
                        .iter()
                        .enumerate()
                        .map(|(at, t)| (*t, if at == 0 { o.total - others } else { 1 }))
                        .collect(),
                ))
            }
            DecisionKind::ChooseCards(o) => Answer::ChooseCards(CardsChoice(
                o.from.iter().copied().take(o.min as usize).collect(),
            )),
            DecisionKind::OrderCards(o) => Answer::OrderCards(OrderChoice(o.cards.clone())),
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(false)),
            DecisionKind::ChooseOption(_) => Answer::ChooseOption(OptionChoice(0)),
            DecisionKind::ChooseCosts(o) if o.can_cancel => {
                Answer::ChooseCosts(CostsChoice::Cancel)
            }
            DecisionKind::ChooseCosts(_) => Answer::ChooseCosts(CostsChoice::Costs(Vec::new())),
            DecisionKind::OrderTriggers(o) => {
                Answer::OrderTriggers(TriggersChoice((0..o.triggers.len()).collect()))
            }
            DecisionKind::ChooseReplacement(_) => Answer::ChooseReplacement(ReplacementChoice(0)),
        }
    }

    /// Answers the pending decision passively (pass, keep, no attackers, no blockers).
    pub fn answer_passively(&mut self) -> Result<Vec<Event>, ScriptError> {
        let decision = self.decision()?.clone();
        self.answer(Driver::passive_answer(&decision))
    }

    /// Answers passively until the game reaches `step` of turn `turn` (a decision is pending there).
    pub fn idle_until(&mut self, turn: u32, step: Step) -> Result<(), ScriptError> {
        for _ in 0..MAX_WAIT_ANSWERS {
            let state = self.state();
            if state.turn().number() == turn
                && state.turn().step() == step
                && self.engine.decision().is_some()
            {
                return Ok(());
            }
            let decision = self.decision()?.clone();
            self.answer(Driver::passive_answer(&decision))?;
        }
        Err(ScriptError::Stuck(format!("turn {turn} {step:?}")))
    }

    /// Passes priority (only) until the current turn reaches `step`; any other decision is an error.
    pub fn pass_until(&mut self, step: Step) -> Result<(), ScriptError> {
        for _ in 0..MAX_WAIT_ANSWERS {
            if self.state().turn().step() == step && self.engine.decision().is_some() {
                return Ok(());
            }
            self.pass()?;
        }
        Err(ScriptError::Stuck(format!("{step:?}")))
    }
}
