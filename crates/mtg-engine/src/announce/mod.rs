//! The announcement of a spell or ability that is put on the stack (CR 601.2): the one procedure every cast and
//! activation goes through, and a triggered ability as it goes on the stack (CR 603.3).
//!
//! What a player has begun lives outside game state until it is paid for, so backing out is free and leaves no trace
//! in the log. [`announce`] walks the [`Phase`]s in the order of the rules; each is a module that asks its question,
//! passes or rewinds the whole announcement, and the ones that wait for a later task are stubs that pass
//! (`docs/engine.md`, Casting).

mod alternative;
mod division;
mod legality;
mod modes;
mod modifiers;
mod optional;
mod payment;
mod targets;
mod total;
mod x;

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::abilities::{AbilityRef, Cause};
use crate::ability_reader;
use crate::choose::{Choices, Target};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::resolve::spell_ability;
use crate::state::GameState;
use crate::triggers::PendingTrigger;

pub(crate) use alternative::Alternative;
pub(crate) use optional::Optional;
pub(crate) use x::XCost;

/// What is being put on the stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum What {
    /// A spell cast from a hand.
    Spell(ObjectId),
    /// An activated ability of a permanent.
    Ability(AbilityRef),
    /// A triggered ability that waits in the queue; its targets are chosen as it goes on the stack (CR 603.3d).
    Trigger(Box<PendingTrigger>),
}

/// One step of the announcement, named by the rule of CR 601.2 it carries out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    /// The modes of a modal spell or ability (CR 601.2b, 700.2).
    Modes,
    /// The alternative cost the spell is cast for (CR 601.2b, 118.9).
    Alternative,
    /// The optional costs paid in addition (CR 601.2b, 118.8, 702.33).
    Optional,
    /// The value of X (CR 601.2b, 107.3).
    X,
    /// The targets of each specification (CR 601.2c, 115).
    Targets,
    /// How an amount is divided among the targets (CR 601.2d).
    Division,
    /// Whether the proposed spell or ability can be put on the stack (CR 601.2e).
    Legality,
    /// The total cost: the cost, its alternative and additional parts, X and the cost modifiers (CR 601.2f).
    Total,
    /// Mana abilities and payment, then the spell or ability goes on the stack (CR 601.2g, 601.2h, 601.2i).
    Payment,
}

impl Phase {
    /// The steps in the order of the rules; the announcement asks them one after the other and a step that has
    /// nothing to ask passes to the next.
    pub(crate) const ORDER: [Phase; 9] = [
        Phase::Modes,
        Phase::Alternative,
        Phase::Optional,
        Phase::X,
        Phase::Targets,
        Phase::Division,
        Phase::Legality,
        Phase::Total,
        Phase::Payment,
    ];

    /// The letter of the rule of CR 601.2 the step carries out: `b` for 601.2b.
    #[cfg(test)]
    pub(crate) const fn rule(self) -> char {
        match self {
            Phase::Modes | Phase::Alternative | Phase::Optional | Phase::X => 'b',
            Phase::Targets => 'c',
            Phase::Division => 'd',
            Phase::Legality => 'e',
            Phase::Total => 'f',
            Phase::Payment => 'g',
        }
    }

    /// The step's question, the events that complete the announcement, or `None` to go on to the next step.
    fn run(self, ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
        match self {
            Phase::Modes => modes::run(ctx, work),
            Phase::Alternative => alternative::run(ctx, work),
            Phase::Optional => optional::run(ctx, work),
            Phase::X => x::run(ctx, work),
            Phase::Targets => targets::run(ctx, work),
            Phase::Division => division::run(ctx, work),
            Phase::Legality => legality::run(ctx, work),
            Phase::Total => total::run(ctx, work),
            Phase::Payment => Some(payment::run(ctx, work)),
        }
    }
}

/// What the rules of the spell or ability ask its player to announce beyond modes and targets, read from the rules
/// when the announcement begins.
///
/// Nothing reads these from a card yet: (g2) fills `x`, (g3) `optional` and (g4) `alternative`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Needs {
    /// The cost the spell is cast for instead of its mana cost.
    pub(crate) alternative: Option<Alternative>,
    /// The costs the player may add, by index.
    pub(crate) optional: Vec<Optional>,
    /// An `{X}` in the cost.
    pub(crate) x: Option<XCost>,
}

impl Needs {
    /// What `what` asks for; its rules are `rules`.
    const fn of(_state: &GameState, _what: &What, _rules: Option<&Ability>) -> Self {
        Needs {
            alternative: None,
            optional: Vec::new(),
            x: None,
        }
    }
}

/// The steps of the announcement that are done and the decision it waits for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Done {
    /// The modes are settled: chosen, or there are none to choose. Choosing none is a choice too.
    pub(crate) modes: bool,
    /// The optional costs are settled.
    pub(crate) optional: bool,
    /// X is settled.
    pub(crate) x: bool,
    /// How many target specifications have their division settled.
    pub(crate) divided: usize,
    /// The step whose decision the player has been asked.
    asked: Option<Phase>,
}

/// A cast or activation that has begun and is not paid for yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pending {
    pub(crate) player: PlayerId,
    pub(crate) what: What,
    /// What the player has announced so far.
    pub(crate) choices: Choices,
    /// Which steps are done.
    pub(crate) done: Done,
    needs: Option<Needs>,
}

impl Pending {
    pub(crate) fn new(player: PlayerId, what: What) -> Self {
        Pending {
            player,
            what,
            choices: Choices::default(),
            done: Done::default(),
            needs: None,
        }
    }

    /// The announcement of a triggered ability that waits in the queue.
    pub(crate) fn trigger(player: PlayerId, waiting: PendingTrigger) -> Self {
        Pending::new(player, What::Trigger(Box::new(waiting)))
    }

    /// An announcement whose needs are given instead of read from the rules, to try the steps a card cannot ask for yet.
    #[cfg(test)]
    pub(crate) fn needing(player: PlayerId, what: What, needs: Needs) -> Self {
        Pending {
            needs: Some(needs),
            ..Pending::new(player, what)
        }
    }

    /// Takes the answer to the decision the announcement waits for.
    pub(crate) fn accept(&mut self, answered: Answered) -> Result<(), EngineError> {
        let wanted = match answered {
            Answered::Modes(_) => Phase::Modes,
            Answered::Targets(_) => Phase::Targets,
            Answered::Number(_) => Phase::X,
            Answered::Costs(_) => Phase::Optional,
            Answered::Division(_) => Phase::Division,
        };
        if self.done.asked != Some(wanted) {
            return Err(EngineError::InvalidEvent(
                "an answer for a step the announcement did not ask".into(),
            ));
        }
        self.done.asked = None;
        match answered {
            Answered::Modes(modes) => {
                self.choices.modes = modes;
                self.done.modes = true;
            }
            Answered::Targets(targets) => self.choices.targets.push(targets),
            Answered::Number(number) => {
                self.choices.x = Some(number);
                self.done.x = true;
            }
            Answered::Costs(costs) => {
                self.choices.optional_costs = costs;
                self.done.optional = true;
            }
            Answered::Division(shares) => {
                self.choices.division.extend(shares);
                self.done.divided += 1;
            }
        }
        Ok(())
    }
}

/// What a decision of the announcement was answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Answered {
    /// The modes, in the order they are printed.
    Modes(Vec<usize>),
    /// The targets of the next specification.
    Targets(Vec<Target>),
    /// The value of X.
    Number(u32),
    /// The optional costs paid, by index.
    Costs(Vec<usize>),
    /// The shares of the next amount to divide.
    Division(Vec<(Target, u32)>),
}

/// What the cast or activation needs next.
pub(crate) enum Next {
    /// A decision from the player.
    Ask(Decision),
    /// Nothing more to ask: these events pay for it and put it on the stack.
    Commit(Vec<Event>),
    /// It cannot go on, so nothing happens: the announcement is rewound (CR 601.2).
    Drop,
}

/// What a step sees of the announcement, which it does not change.
pub(super) struct Ctx<'a> {
    pub(super) state: &'a GameState,
    pub(super) player: PlayerId,
    pub(super) what: &'a What,
    /// The object whose abilities and targets are being announced.
    pub(super) source: ObjectId,
    /// The ability being announced: the ability of a spell, an activated or a triggered ability.
    pub(super) ability: Option<&'a Ability>,
    pub(super) triggered: Option<&'a PendingTrigger>,
    pub(super) needs: &'a Needs,
}

impl Ctx<'_> {
    /// Whether the player may still back out: a triggered ability that is put on the stack cannot be (CR 603.3c).
    pub(super) const fn can_cancel(&self) -> bool {
        self.triggered.is_none()
    }

    /// What a triggered ability triggered on.
    pub(super) fn cause(&self) -> Option<&Cause> {
        self.triggered.map(|waiting| &waiting.trigger.cause)
    }

    /// The decision that asks `kind` of the player.
    pub(super) const fn ask(&self, kind: DecisionKind) -> Next {
        Next::Ask(Decision {
            player: self.player,
            kind,
        })
    }

    /// What rewinding the announcement does: a spell or activation is dropped without a trace, a triggered ability
    /// that cannot be given what it needs is removed (CR 603.3d).
    fn rewind(&self) -> Next {
        self.triggered.map_or(Next::Drop, |waiting| {
            Next::Commit(vec![crate::triggers::removing(waiting)])
        })
    }
}

/// What a step changes of the announcement.
pub(super) struct Work<'a> {
    pub(super) choices: &'a mut Choices,
    pub(super) done: &'a mut Done,
    /// The total cost, once the step for it has worked it out.
    pub(super) total: Option<total::Total>,
}

/// The object whose abilities and targets are being announced.
const fn source_of(what: &What) -> ObjectId {
    match what {
        What::Spell(spell) => *spell,
        What::Ability(ability) => ability.object,
        What::Trigger(waiting) => waiting.trigger.source.id,
    }
}

/// The rules of the spell or ability being announced.
fn rules_of(state: &GameState, what: &What) -> Option<Arc<Ability>> {
    let object = state.object(source_of(what));
    match what {
        What::Spell(spell) => object
            .and_then(|_| spell_ability(state, *spell))
            .map(|(_, rules)| rules),
        What::Ability(address) => object.and_then(|_| ability_reader::ability(state, *address)),
        What::Trigger(waiting) => Some(Arc::clone(&waiting.rules)),
    }
}

/// The next decision of a pending cast or activation, or the events that complete it.
///
/// The steps of [`Phase::ORDER`] run in turn, each passing over what it has no question for. A step that cannot be
/// met (a mode or target specification with nothing to offer, a cost that cannot be paid) rewinds the announcement:
/// a cast or activation is dropped with no event, and a triggered ability that cannot be given what it needs is
/// removed (CR 603.3d).
pub(crate) fn announce(state: &GameState, pending: &mut Pending) -> Next {
    let Pending {
        player,
        what,
        choices,
        done,
        needs,
    } = pending;
    let what: &What = what;
    let held = rules_of(state, what);
    let needs: &Needs = needs.get_or_insert_with(|| Needs::of(state, what, held.as_deref()));
    let ctx = Ctx {
        state,
        player: *player,
        what,
        source: source_of(what),
        ability: held.as_deref(),
        triggered: match what {
            What::Trigger(waiting) => Some(waiting),
            _ => None,
        },
        needs,
    };
    let mut work = Work {
        choices,
        done,
        total: None,
    };
    for phase in Phase::ORDER {
        match phase.run(&ctx, &mut work) {
            None => {}
            Some(Next::Drop) => return ctx.rewind(),
            Some(Next::Ask(decision)) => {
                work.done.asked = Some(phase);
                return Next::Ask(decision);
            }
            Some(commit) => return commit,
        }
    }
    ctx.rewind()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod procedure;

#[cfg(test)]
mod through_engine;
