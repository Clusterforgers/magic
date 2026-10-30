//! The announcement of a spell or ability that is put on the stack (CR 601.2): the one procedure every cast and
//! activation goes through, and a triggered ability as it goes on the stack (CR 603.3).
//!
//! What a player has begun lives outside game state until it is paid for, so backing out is free and leaves no trace
//! in the log. [`announce`] walks the [`Phase`]s in the order of the rules; each is a module that asks its question,
//! passes or rewinds the whole announcement, and the ones that wait for a later task are stubs that pass
//! (`docs/engine.md`, Casting).

mod again;
mod alternative;
mod anchor;
mod c9_emerge_reduce;
pub(crate) mod companion;
mod dk1c5_unlock;
mod division;
mod face;
pub(crate) mod foretell;
mod legality;
mod modes;
mod modifiers;
mod optional;
mod parts;
mod payment;
pub(crate) mod required;
pub(crate) mod suspend;
mod targets;
mod total;
pub(crate) mod turn_up;
mod w2f_counter_kinds;
mod x;

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::value::CastWay;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::abilities::{AbilityRef, Cause};
use crate::ability_reader;
use crate::choose::{Choices, Target};
use crate::combat::AttackTarget;
use crate::decision::{Decision, DecisionKind, Unplayable};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::faces;
use crate::parts::Part;
use crate::price::Price;
use crate::resolve::spell_ability;
use crate::state::GameState;
use crate::triggers::PendingTrigger;
use crate::ways::Way;
use anchor::Anchor;

pub(crate) use modifiers::{c11a_reduced, cheapest};
pub(crate) use optional::{Optional, kicked_with, payments};
pub(crate) use x::{XCost, payable, some_x_has_targets};

/// What is being put on the stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum What {
    /// A spell cast from a hand.
    Spell(ObjectId),
    /// An activated ability of a permanent.
    Ability(AbilityRef),
    /// A triggered ability that waits in the queue; its targets are chosen as it goes on the stack (CR 603.3d).
    Trigger(Box<PendingTrigger>),
    /// An attack declared whose cost is not paid yet (CR 508.1h); only its payment is asked (`attack_tax`).
    Attack(Vec<(ObjectId, AttackTarget)>, Vec<ObjectId>),
}

/// One step of the announcement, named by the rule of CR 601.2 it carries out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    /// The face of a card with several faces the spell is cast as (CR 601.2a, 712.2, 715.3, 709.3).
    Face,
    /// The modes of a modal spell or ability (CR 601.2b, 700.2).
    Modes,
    /// The alternative cost the spell is cast for (CR 601.2b, 118.9).
    Alternative,
    /// The optional costs paid in addition (CR 601.2b, 118.8, 702.33).
    Optional,
    /// The permanents and cards that pay the parts of an ability's cost that take some (CR 601.2b, 601.2h).
    Parts,
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
    pub(crate) const ORDER: [Phase; 10] = [
        Phase::Modes,
        Phase::Alternative,
        Phase::Optional,
        Phase::Parts,
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
            Phase::Face
            | Phase::Modes
            | Phase::Alternative
            | Phase::Optional
            | Phase::Parts
            | Phase::X => 'b',
            Phase::Targets => 'c',
            Phase::Division => 'd',
            Phase::Legality => 'e',
            Phase::Total => 'f',
            Phase::Payment => 'g',
        }
    }

    /// Why an announcement that cannot get past this step is taken back, in the words of the priority decision.
    const fn hindrance(self) -> Unplayable {
        match self {
            Phase::Face | Phase::Modes | Phase::Targets | Phase::Division | Phase::Legality => {
                Unplayable::Targets
            }
            Phase::Parts => Unplayable::Cost,
            Phase::Alternative | Phase::Optional | Phase::X | Phase::Total | Phase::Payment => {
                Unplayable::Mana
            }
        }
    }

    /// The step's question, the events that complete the announcement, or `None` to go on to the next step.
    fn run(self, ctx: &Ctx<'_>, work: &mut Work<'_>) -> Option<Next> {
        match self {
            // The face is settled before the steps that read the card (`announce`).
            Phase::Face => None,
            Phase::Modes => modes::run(ctx, work),
            Phase::Alternative => alternative::run(ctx, work),
            Phase::Optional => optional::run(ctx, work),
            Phase::Parts => parts::run(ctx, work),
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
/// The keywords of a spell fill `optional` (kicker, multikicker), the cost and rules of a card fill `x`, the keywords
/// and the cast permissions fill `ways`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Needs {
    /// The ways the card can be cast from where it is; empty for the mana cost alone.
    pub(crate) ways: Vec<Way>,
    /// The costs the player may add, by index.
    pub(crate) optional: Vec<Optional>,
    /// An `{X}` in the cost.
    pub(crate) x: Option<XCost>,
    /// The price of an activated ability whose cost has parts the player picks objects for.
    pub(crate) price: Option<Box<Price>>,
}

/// What a resolution lets a player cast (CR 608.2g): the spell goes on the stack as part of the resolution, with no
/// timing and no way back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Granted {
    /// The mana cost is not paid (CR 118.9).
    pub(crate) free: bool,
    /// The alternative cost it is cast for: its madness cost, or nothing for suspend.
    pub(crate) way: Option<CastWay>,
}

impl Needs {
    /// What `what` asks for; its rules are `rules`.
    fn of(
        state: &GameState,
        player: PlayerId,
        what: &What,
        rules: Option<&Ability>,
        granted: Option<Granted>,
    ) -> Self {
        Needs {
            ways: alternative::read(state, player, what, granted),
            optional: optional::read(state, what),
            x: XCost::of(state, what, rules),
            price: parts::read(what, rules),
        }
    }

    /// The indices of the optional costs that may be paid any number of times.
    fn repeatable(&self) -> impl Iterator<Item = usize> {
        self.optional
            .iter()
            .enumerate()
            .filter(|(_, cost)| cost.offerable && cost.repeatable)
            .map(|(index, _)| index)
    }

    /// The parts a spell pays besides its mana for what `choices` announced: those of the way it is cast and of the
    /// optional costs paid, in that order.
    pub(crate) fn cost_parts(&self, choices: &Choices) -> Vec<Part> {
        let mut parts = self
            .ways
            .iter()
            .find(|way| way.is(choices))
            .map(|way| way.parts.clone())
            .unwrap_or_default();
        parts.extend(optional::parts_paid(&self.optional, choices));
        parts
    }
}

/// The steps of the announcement that are done and the decision it waits for.
// One flag for each step that is settled, named after it.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Done {
    /// The face of the card is settled: chosen, or the card has one.
    pub(crate) face: bool,
    /// The face the player picked among those offered, by position in the options.
    pub(crate) face_picked: Option<usize>,
    /// The modes are settled: chosen, or there are none to choose. Choosing none is a choice too.
    pub(crate) modes: bool,
    /// The way the spell is cast is settled.
    pub(crate) alternative: bool,
    /// The way the player picked among those offered, by position in the options.
    pub(crate) way_picked: Option<usize>,
    /// The optional costs are settled.
    pub(crate) optional: bool,
    /// Once the optional costs that may be paid once are settled: how many of those that may be paid any number of
    /// times are settled too.
    pub(crate) repeated: Option<usize>,
    /// X is settled.
    pub(crate) x: bool,
    /// How many target specifications have their division settled.
    pub(crate) divided: usize,
    /// The kind of counter the player picked for the next permanent of a cost that removes a counter of any kind, by
    /// position in the options.
    pub(crate) w2f_kind_picked: Option<u8>,
    /// The step whose decision the player has been asked.
    asked: Option<Phase>,
    /// Why the announcement was taken back: the step that could not be met.
    pub(crate) rewound: Option<Unplayable>,
}

/// A special action that pays a cost and does not use the stack (CR 116.2): the cost is paid through the normal payment
/// decision and the card moves to exile with the payment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Special {
    /// Suspend (CR 702.62a).
    Suspend,
    /// Foretell (CR 702.143a).
    Foretell,
    /// Turning a face-down permanent face up (CR 702.37e).
    TurnFaceUp,
    /// Putting a companion from outside the game into the hand (CR 702.139): the card is the id it will have there.
    Companion,
    /// Unlocking this door of a Room (CR 709.5e).
    Unlock(u8),
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
    /// Set when a resolution casts the spell.
    pub(crate) granted: Option<Granted>,
    /// Set when the card is the subject of a special action that pays a cost and does not use the stack.
    pub(crate) special: Option<Special>,
    needs: Option<Needs>,
    /// The objects it is about when it began, which a manual action may take away (`anchor.rs`).
    anchor: Option<Anchor>,
    /// A manual action came while it was under way: what it would put on the stack is checked against the state before it
    /// is (`Engine::advance`).
    pub(crate) touched: bool,
    /// The answers it took, in order, to give again once a manual action unsettles it (`again.rs`).
    answers: Vec<crate::decision::Answer>,
    /// A manual action came since its last question: it is asked again before its next one (`again.rs`).
    stale: bool,
}

impl Pending {
    pub(crate) fn new(player: PlayerId, what: What) -> Self {
        Pending {
            player,
            what,
            choices: Choices::default(),
            done: Done::default(),
            granted: None,
            needs: None,
            anchor: None,
            touched: false,
            answers: Vec::new(),
            stale: false,
            special: None,
        }
    }

    /// The cast of `spell` that a resolution lets `player` make, with or without paying its mana cost.
    pub(crate) fn during_resolution(
        player: PlayerId,
        spell: ObjectId,
        free: bool,
        way: Option<CastWay>,
    ) -> Self {
        Pending {
            granted: Some(Granted { free, way }),
            ..Pending::new(player, What::Spell(spell))
        }
    }

    /// The special action `special` on `card` from the hand by `player`: only the payment of its cost is asked.
    pub(crate) fn special(player: PlayerId, card: ObjectId, special: Special) -> Self {
        Pending {
            special: Some(special),
            ..Pending::new(player, What::Spell(card))
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

    /// The parts a spell pays besides its mana for what has been announced (`Needs::cost_parts`), a counted one for the
    /// X announced.
    pub(crate) fn cost_parts(&self) -> Vec<Part> {
        let x = self.choices.x.unwrap_or(0);
        self.needs
            .as_ref()
            .map(|needs| needs.cost_parts(&self.choices))
            .unwrap_or_default()
            .iter()
            .map(|part| part.at(x))
            .collect()
    }

    /// Whether the cards picked for the next part of the cost are too weak for it: the part taps creatures for their
    /// total power and `cards` have less, so the answer is refused (CR 702.122a).
    pub(crate) fn check_picked(
        &self,
        state: &GameState,
        cards: &[ObjectId],
    ) -> Result<(), IllegalAnswer> {
        let weak = self
            .needs
            .as_ref()
            .and_then(|needs| parts::weak(&self.what, needs, &self.choices, state, cards));
        match weak {
            Some((needed, got)) => Err(IllegalAnswer::TooLittlePower { needed, got }),
            None => Ok(()),
        }
    }

    /// Takes the answer to the decision the announcement waits for.
    pub(crate) fn accept(&mut self, answered: Answered) -> Result<(), EngineError> {
        let wanted = match answered {
            Answered::Modes(_) => Phase::Modes,
            Answered::Way(_) if self.done.asked == Some(Phase::Face) => Phase::Face,
            // The X of a counted part is asked while the parts are, before the picks that depend on it, and a choice of
            // option among the parts is the kind of counter a picked permanent loses.
            Answered::Way(_) | Answered::Number(_) if self.done.asked == Some(Phase::Parts) => {
                Phase::Parts
            }
            Answered::Cards(_) => Phase::Parts,
            Answered::Way(_) => Phase::Alternative,
            Answered::Targets(_) => Phase::Targets,
            // A number is asked for X, and for the times a repeatable optional cost is paid, which comes once the
            // costs that are paid once are settled.
            Answered::Number(_)
                if self.done.repeated.is_some() && self.done.asked == Some(Phase::Optional) =>
            {
                Phase::Optional
            }
            Answered::Number(_) => Phase::X,
            Answered::Costs(_) => Phase::Optional,
            Answered::Division(_) => Phase::Division,
        };
        let costs_again = matches!(answered, Answered::Costs(_)) && self.done.repeated.is_some();
        if self.done.asked != Some(wanted) || costs_again {
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
            Answered::Way(pick) if wanted == Phase::Face => self.done.face_picked = Some(pick),
            Answered::Way(pick) if wanted == Phase::Parts => {
                self.done.w2f_kind_picked = u8::try_from(pick).ok();
            }
            Answered::Way(pick) => self.done.way_picked = Some(pick),
            Answered::Cards(cards) => self.choices.pick_for_cost(cards),
            Answered::Targets(targets) => self.choices.targets.push(targets),
            Answered::Number(times) if wanted == Phase::Optional => {
                let paid = self
                    .needs
                    .as_ref()
                    .and_then(|needs| needs.repeatable().nth(self.done.repeated.unwrap_or(0)));
                if let Some(index) = paid {
                    let times = usize::try_from(times).unwrap_or(0);
                    self.choices
                        .optional_costs
                        .extend(std::iter::repeat_n(index, times));
                }
                self.done.repeated = Some(self.done.repeated.unwrap_or(0) + 1);
            }
            Answered::Number(number) => {
                self.choices.x = Some(number);
                self.done.x = true;
            }
            Answered::Costs(costs) => {
                self.choices.optional_costs = costs;
                self.done.repeated = Some(0);
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
    /// The way to cast the spell, by position among those offered.
    Way(usize),
    /// The targets of the next specification.
    Targets(Vec<Target>),
    /// The value of X.
    Number(u32),
    /// The optional costs paid, by index.
    Costs(Vec<usize>),
    /// The permanents and cards picked for the next part of the cost.
    Cards(Vec<ObjectId>),
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
    pub(super) granted: Option<Granted>,
    pub(super) needs: &'a Needs,
}

impl Ctx<'_> {
    /// Whether the player may still back out: a triggered ability that is put on the stack cannot be (CR 603.3c), nor
    /// can a spell the resolution of another is casting, which the player chose to cast already.
    pub(super) const fn can_cancel(&self) -> bool {
        self.triggered.is_none() && self.granted.is_none()
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
fn source_of(what: &What) -> ObjectId {
    match what {
        What::Spell(spell) => *spell,
        What::Ability(ability) => ability.object,
        What::Trigger(waiting) => waiting.trigger.source.id,
        What::Attack(attackers, _) => attackers.first().map_or(ObjectId(0), |(object, _)| *object),
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
        What::Attack(..) => None,
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
        granted,
        special,
        needs,
        anchor,
        touched: _,
        answers: _,
        stale: _,
    } = pending;
    // What a manual action took out from under the announcement cannot be put on the stack.
    let now = Anchor::of(state, what);
    if anchor.get_or_insert_with(|| now.clone()) != &now {
        return Next::Drop;
    }
    if let What::Attack(attackers, _) = what {
        return crate::attack_tax::next(state, *player, attackers);
    }
    if let Some(special) = special
        && let What::Spell(card) = what
    {
        return match special {
            Special::Suspend => suspend::next(state, *player, *card),
            Special::Foretell => foretell::next(state, *player, *card),
            Special::TurnFaceUp => turn_up::next(state, *player, *card),
            Special::Companion => companion::next(state, *player, *card),
            Special::Unlock(door) => dk1c5_unlock::next(state, *player, *card, *door),
        };
    }
    let what: &What = what;
    // The card is the face it is cast as from the first question on.
    let faced;
    let state = match what {
        What::Spell(card) if faces::is_multi_face(state, *card) => {
            if let Some(next) = face::run(state, *player, *card, *granted, choices, done) {
                return next;
            }
            faced = state.with_face(*card, choices.face);
            &*faced
        }
        _ => state,
    };
    let held = rules_of(state, what);
    let needs: &Needs =
        needs.get_or_insert_with(|| Needs::of(state, *player, what, held.as_deref(), *granted));
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
        granted: *granted,
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
            Some(Next::Drop) => {
                work.done.rewound = Some(phase.hindrance());
                return ctx.rewind();
            }
            Some(Next::Ask(decision)) => {
                // Entwine is paid before the modes are chosen: the costs it asks are answered as the optional costs.
                let asks_costs = matches!(
                    decision.kind,
                    DecisionKind::ChooseCosts(_) | DecisionKind::ChooseNumber(_)
                );
                work.done.asked = Some(if phase == Phase::Modes && asks_costs {
                    Phase::Optional
                } else {
                    phase
                });
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

#[cfg(test)]
mod kicks;

#[cfg(test)]
mod x_after;
