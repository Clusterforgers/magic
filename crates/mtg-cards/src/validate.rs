//! Validation of rules data: the structure of a file by itself, and a file against the imported card.
//!
//! Every problem carries the path of the value it is about, in the form serde uses for parse errors
//! (`faces[0].abilities[1].Triggered.effects[2].If.then[0]`).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::ability::{Ability, LoyaltyCost, Timing};
use crate::assemble::{MANA_WAYS, PLAIN_MANA_ABILITY, mana_shape, plain_mana_ability};
use crate::card::{Card, Face, ImplStatus, TokenSpec};
use crate::cost::{Cost, CostItem};
use crate::effect::{Destination, DieRow, Effect, Mode, Order, Place, Produce, TokenRef, VoteFor};
use crate::keyword::Keyword;
use crate::modify::{Duration, GameRule, Modification};
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use crate::spend::SpendOn;
use crate::support::{PrintedCard, PrintedFace, oracle_fingerprint};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::CastWay;
use crate::value::{Amount, Condition, TurnTally};

/// One problem found in a file: where, and what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The path inside the file; empty for the file as a whole.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

/// How deep the abilities of a token may make tokens whose abilities are counted.
const MOST_NESTED: u32 = 4;

/// An exchange of control names its two permanents with one reference or two.
const W6E_EXCHANGE_PAIR: &str =
    "an exchange of control names its two permanents with one reference or two";

/// Only the rule that a permanent doesn't untap lasts through the next untap step of its controller.
const C7_UNTAP_ONLY: &str =
    "only a permanent that doesn't untap waits for its controller's next untap step";

/// The name of an enum value as serde writes it (`DealDamage`), for naming primitives.
fn tag_of<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        Ok(serde_json::Value::Object(map)) if map.len() == 1 => {
            map.keys().next().cloned().unwrap_or_default()
        }
        _ => String::from("?"),
    }
}

/// Whether the part of a cost that counts holds a fixed number or the X announced: the engine pays no other amount
/// (`Cost.PayLife.Counted`).
fn is_fixed(item: &CostItem) -> bool {
    let fixed = |amount: &Amount| {
        matches!(amount, Amount::Fixed(number) if *number >= 0) || *amount == Amount::X
    };
    match item {
        CostItem::PayLife(amount) | CostItem::PayEnergy(amount) | CostItem::Mill(amount) => {
            fixed(amount)
        }
        CostItem::RemoveCounters { count, .. }
        | CostItem::Sacrifice { count, .. }
        | CostItem::Discard { count, .. }
        | CostItem::TapPermanents { count, .. }
        | CostItem::ExileFromGraveyard { count, .. }
        | CostItem::ExileFromHand { count, .. }
        | CostItem::RevealFromHand { count, .. }
        | CostItem::ReturnToHand { count, .. } => fixed(count),
        _ => true,
    }
}

/// Whether the amount of a part of a cost is the X announced.
fn amount_is_x(item: &CostItem) -> bool {
    match item {
        CostItem::PayLife(amount) | CostItem::PayEnergy(amount) | CostItem::Mill(amount) => {
            *amount == Amount::X
        }
        CostItem::RemoveCounters { count, .. }
        | CostItem::Sacrifice { count, .. }
        | CostItem::Discard { count, .. }
        | CostItem::TapPermanents { count, .. }
        | CostItem::ExileFromGraveyard { count, .. }
        | CostItem::ExileFromHand { count, .. }
        | CostItem::RevealFromHand { count, .. }
        | CostItem::ReturnToHand { count, .. } => *count == Amount::X,
        _ => false,
    }
}

/// Whether text is a UUID in the lowercase 8-4-4-4-12 form Scryfall uses.
fn is_uuid(text: &str) -> bool {
    let groups: Vec<&str> = text.split('-').collect();
    groups.len() == 5
        && groups.iter().zip([8, 4, 4, 4, 12]).all(|(group, len)| {
            group.len() == len && group.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
        })
}

fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// What a reference can be or must be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Player,
    Object,
    Either,
}

impl Kind {
    const fn word(self) -> &'static str {
        match self {
            Kind::Player => "a player",
            Kind::Object => "an object",
            Kind::Either => "a player or an object",
        }
    }

    fn fits(self, want: Kind) -> bool {
        self == Kind::Either || want == Kind::Either || self == want
    }
}

fn kind_of_targetable(what: &Targetable) -> Kind {
    match what {
        Targetable::Player | Targetable::Opponent => Kind::Player,
        Targetable::Object(_) | Targetable::PerPlayer { .. } => Kind::Object,
        Targetable::AnyTarget | Targetable::Either(_) => Kind::Either,
    }
}

/// Whether a step asks its controller or another player something as it runs.
fn w5d_asks(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::May { .. }
            | Effect::Modal { .. }
            | Effect::Choose { .. }
            | Effect::Pay { .. }
            | Effect::Search { .. }
            | Effect::Sacrifice { .. }
            | Effect::Discard { .. }
            | Effect::TapOrUntap { .. }
    )
}

/// The target specifications and bound names an ability body can see.
#[derive(Default)]
struct Scope {
    targets: Vec<(Kind, bool, bool)>,
    used: BTreeSet<usize>,
    in_trigger: bool,
    /// `Dealer` names something: a triggered ability, or a replacement of damage.
    dealer: bool,
    names: Vec<BTreeMap<String, Kind>>,
}

impl Scope {
    fn new(targets: &[TargetSpec], in_trigger: bool) -> Scope {
        Scope {
            targets: targets
                .iter()
                .map(|spec| {
                    (
                        kind_of_targetable(&spec.what),
                        spec.divide.is_some(),
                        !spec.bounds.is_one(),
                    )
                })
                .collect(),
            used: BTreeSet::new(),
            in_trigger,
            dealer: in_trigger,
            names: vec![BTreeMap::new()],
        }
    }

    fn lookup(&self, name: &str) -> Option<Kind> {
        self.names
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }
}

/// Where a walk stands in an effect list, which says whether "if you do" has an effect to ask about.
#[derive(Clone, Copy, PartialEq, Eq)]
enum After {
    /// The first effect of a list, or no effect list.
    Nothing,
    /// An effect that has another before it.
    Effect,
    /// The condition of an `If` that has an effect before it.
    Condition,
}

/// What the amount being walked is the amount of.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Amounting {
    /// Anything but damage or counters.
    #[default]
    Other,
    /// Damage or counters, whose recipients each read their share of a division.
    Shared,
}

impl Amounting {
    fn of(shared: bool) -> Self {
        if shared { Self::Shared } else { Self::Other }
    }
}

// One flag for each fact about the card the walk checks a node against.
#[allow(clippy::struct_excessive_bools)]
struct Walker<'a> {
    tokens: &'a BTreeSet<String>,
    problems: Vec<Problem>,
    uses: BTreeSet<String>,
    path: Vec<String>,
    scope: Scope,
    has_kicker: bool,
    /// The card has a Devour keyword: what `Amount::Devoured` reads.
    w7e_has_devour: bool,
    /// The costs of the card's Kicker keywords: what `KickedWith` may name.
    kickers: Vec<Cost>,
    /// Whether the card prints an optional additional cost ("As an additional cost to cast this spell, you may ...").
    has_additional_cost: bool,
    /// Whether the cost being walked takes objects the effects can name (`Ref::Paid`): a part that picks, or a required additional cost that does.
    pays_objects: bool,
    /// Whether a required additional cost of the card counts the X the caster announces, which the effects read.
    counts_x: bool,
    x_free: bool,
    /// Whether an `X` read here is the one announced for the ability being walked, which its effects read as it resolves.
    x_now: bool,
    /// Whether a choice between costs is walked as the required additional cost of a spell, the one place the engine asks it.
    choosing: bool,
    /// What the amount being walked is the amount of, which says whether a divided amount has recipients to read it.
    amounting: Amounting,
    /// How many abilities deep the walk stands inside abilities that other abilities grant or tokens carry.
    nested: u32,
    /// Where the walk stands in an effect list, for "if you do".
    after: After,
    /// The effect before the one being walked is a `May`, whose acceptance "if you do" asks about.
    after_may: bool,
    face: usize,
    /// The face has an `{X}` in a cycling cost or in the cost of a monstrosity, which its trigger on that action reads.
    w6c_cost_x: bool,
    spell_targets: Vec<TargetSpec>,
    x_uses: Vec<(usize, String)>,
    names: Vec<(usize, String, String)>,
    /// The keywords the abilities grant: `AddKeyword`, the abilities of a token, the abilities a backup gives.
    granted: Vec<Keyword>,
}

impl<'a> Walker<'a> {
    fn new(tokens: &'a BTreeSet<String>) -> Self {
        Walker {
            tokens,
            problems: Vec::new(),
            uses: BTreeSet::new(),
            path: Vec::new(),
            scope: Scope::default(),
            has_kicker: false,
            w7e_has_devour: false,
            kickers: Vec::new(),
            has_additional_cost: false,
            pays_objects: false,
            counts_x: false,
            x_free: false,
            x_now: false,
            choosing: false,
            amounting: Amounting::Other,
            nested: 0,
            after: After::Nothing,
            after_may: false,
            face: 0,
            w6c_cost_x: false,
            spell_targets: Vec::new(),
            x_uses: Vec::new(),
            names: Vec::new(),
            granted: Vec::new(),
        }
    }

    fn at<T>(&mut self, segment: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        self.path.push(segment.into());
        let result = f(self);
        self.path.pop();
        result
    }

    /// The path as serde writes it: names joined by dots, indices attached (`effects[2].If.then[0]`).
    fn current_path(&self) -> String {
        let mut path = String::new();
        for segment in &self.path {
            if !path.is_empty() && !segment.starts_with('[') {
                path.push('.');
            }
            path.push_str(segment);
        }
        path
    }

    fn problem(&mut self, message: impl Into<String>) {
        self.problems.push(Problem {
            path: self.current_path(),
            message: message.into(),
        });
    }

    fn uses(&mut self, group: &str, tag: &str) {
        self.uses.insert(format!("{group}.{tag}"));
    }

    fn expect(&mut self, found: Kind, want: Kind) {
        if !found.fits(want) {
            self.problem(format!(
                "expected {} but this is {}",
                want.word(),
                found.word()
            ));
        }
    }

    fn bind(&mut self, name: &str, kind: Kind) {
        if !is_name(name) {
            self.problem(format!("`{name}` is not a name (lowercase letters, digits and underscores, starting with a letter)"));
        } else if self.scope.lookup(name).is_some() {
            self.problem(format!("`{name}` is already bound"));
        } else if let Some(scope) = self.scope.names.last_mut() {
            scope.insert(name.to_owned(), kind);
        }
    }

    fn reference(&mut self, r: &Ref, want: Kind) {
        match r {
            Ref::You
            | Ref::Opponents
            | Ref::EachPlayer
            | Ref::AttachedPlayer
            | Ref::Monarch
            | Ref::Chosen
            | Ref::Promised
            | Ref::Attacked
            | Ref::MostLife
            | Ref::Defending => {
                self.expect(Kind::Player, want);
            }
            Ref::This | Ref::Attached => self.expect(Kind::Object, want),
            Ref::Paid => {
                if !self.pays_objects {
                    self.problem("`Paid` needs a cost that takes objects: a sacrifice, a discard, an exile or a tap");
                }
                self.expect(Kind::Object, want);
            }
            Ref::Target(index) => {
                let index = usize::from(*index);
                match self.scope.targets.get(index).map(|t| t.0) {
                    None => {
                        let count = self.scope.targets.len();
                        self.problem(format!(
                            "target {index} does not exist: this ability has {count} target specification(s)"
                        ));
                    }
                    Some(kind) => {
                        self.scope.used.insert(index);
                        self.expect(kind, want);
                    }
                }
            }
            Ref::Triggering => {
                if !self.scope.in_trigger {
                    self.problem("`Triggering` is only valid in a triggered ability");
                }
            }
            Ref::Dealer => {
                if !self.scope.in_trigger {
                    self.problem("`Dealer` is only valid in a triggered ability");
                } else if !self.scope.dealer {
                    self.problem(
                        "`Dealer` is the source of damage: only a `Damage` replacement has one",
                    );
                }
                self.expect(Kind::Object, want);
            }
            Ref::Bound(name) => match self.scope.lookup(name) {
                None => self.problem(format!("`{name}` is not bound by an earlier step")),
                Some(kind) => self.expect(kind, want),
            },
            Ref::ControllerOf(inner) | Ref::OwnerOf(inner) => {
                self.expect(Kind::Player, want);
                self.at("of", |w| w.reference(inner, Kind::Object));
            }
            Ref::Each(selector) => {
                self.expect(Kind::Object, want);
                self.selector(selector);
            }
        }
    }

    fn selector(&mut self, selector: &Selector) {
        for (i, pred) in selector.0.iter().enumerate() {
            self.at(format!("[{i}]"), |w| w.pred(pred));
        }
    }

    fn pred(&mut self, pred: &Pred) {
        match pred {
            Pred::ControlledBy(r) | Pred::OwnedBy(r) | Pred::AttackingPlayer(r) => {
                self.reference(r, Kind::Player);
            }
            Pred::Is(r)
            | Pred::SharesSubtype(r)
            | Pred::SharesName(r)
            | Pred::SharesType(r)
            | Pred::SharesColor(r)
            | Pred::ExiledWith(r)
            | Pred::EncodedWith(r)
            | Pred::Crewed(r)
            | Pred::Saddled(r)
            | Pred::DamagedBy(r)
            | Pred::SameCard(r) => {
                self.reference(r, Kind::Object);
            }
            Pred::HasKeyword(keyword) => self.keyword(keyword),
            Pred::Named(name) => self
                .names
                .push((self.face, self.current_path(), name.clone())),
            Pred::Compare { to, .. } => {
                self.at("to", |w| w.amount(to));
            }
            Pred::CastFrom(zone) => {
                if !matches!(zone, ZoneKind::Hand | ZoneKind::Graveyard) {
                    self.problem("`CastFrom` names a hand or a graveyard");
                }
            }
            Pred::Targeting(inner) | Pred::HasAttached(inner) => self.selector(inner),
            Pred::Not(inner) => self.pred(inner),
            Pred::AnyOf(preds) => {
                if preds.is_empty() {
                    self.problem("`AnyOf` needs at least one predicate");
                }
                for (i, inner) in preds.iter().enumerate() {
                    self.at(format!("[{i}]"), |w| w.pred(inner));
                }
            }
            Pred::This
            | Pred::Other
            | Pred::Type(_)
            | Pred::Supertype(_)
            | Pred::Subtype(_)
            | Pred::Color(_)
            | Pred::Colorless
            | Pred::Token
            | Pred::Commander
            | Pred::Spell
            | Pred::Ability(_)
            | Pred::Tapped
            | Pred::Untapped
            | Pred::Monstrous
            | Pred::Renowned
            | Pred::EnteredThisTurn
            | Pred::DamagedThisTurn
            | Pred::AttackedThisTurn
            | Pred::Attacking
            | Pred::Blocking
            | Pred::Unblocked
            | Pred::HasCounter(_)
            | Pred::HasAnyCounter
            | Pred::ChosenColor
            | Pred::XInCost
            | Pred::Goaded
            | Pred::ChosenSubtype
            | Pred::In(_) => {}
        }
    }

    /// The amounts that read the optional costs paid: the kicks and the gift promised.
    fn paid_amount(&mut self, amount: &Amount) {
        if *amount == Amount::Devoured {
            self.uses("Amount", "Devoured");
            if !self.w7e_has_devour {
                self.problem("`Devoured` needs a Devour keyword on the card");
            }
        } else if *amount == Amount::Kicks {
            self.uses("Amount", "Kicks");
            if !self.has_kicker {
                self.problem("`Kicks` needs a Kicker or Multikicker keyword on the card");
            }
        } else {
            self.uses("Amount", "Promised");
            if !self.has_additional_cost {
                self.problem("`Promised` needs a Gift keyword on the card");
            }
        }
    }

    /// An amount that counts something of the players a reference names.
    fn player_amount(&mut self, name: &str, who: &Ref) {
        self.uses("Amount", name);
        self.at(name, |w| w.reference(who, Kind::Player));
    }

    /// The fewest cards in a hand of the players a reference names, and the life lost or the dice rolled by the resolving
    /// ability.
    fn hand_or_life_lost(&mut self, amount: &Amount) {
        match amount {
            Amount::LowestHand(who) => self.player_amount("LowestHand", who),
            Amount::DieResult(_) => self.uses("Amount", "DieResult"),
            _ => self.uses("Amount", "LifeLostThisWay"),
        }
    }

    /// A count of the turn: of players for the tallies of players, of one object for the times counters were put on it.
    fn tallied(&mut self, who: &Ref, what: TurnTally) {
        if what == TurnTally::CountersPut {
            self.uses("Amount", "Tallied");
            self.at("Tallied", |w| w.reference(who, Kind::Object));
        } else {
            self.player_amount("Tallied", who);
        }
    }

    fn amount(&mut self, amount: &Amount) {
        // Only the amount of damage or counters, or a sum or even share made of it, can be a recipient's share.
        let sharing = std::mem::take(&mut self.amounting) == Amounting::Shared;
        match amount {
            Amount::Fixed(_) => {}
            Amount::X => {
                if !self.x_free && !self.counts_x {
                    self.x_uses.push((self.face, self.current_path()));
                }
                // The effects and targets of the spell or ability that has the X read it, and so do the delayed triggers
                // it sets up and, for a permanent, its enters trigger and the counters it enters with (CR 107.3m);
                // what else it creates or carries does not remember it.
                if !self.x_now {
                    self.uses("Amount", "X.Elsewhere");
                }
            }
            Amount::Count(selector) => self.selector(selector),
            Amount::Stat { of, .. } => self.at("of", |w| w.reference(of, Kind::Object)),
            Amount::Life(who) => self.reference(who, Kind::Player),
            Amount::CommanderCasts(who) => self.player_amount("CommanderCasts", who),
            Amount::Counters { on, .. } | Amount::AnyCounters(on) => {
                self.at("on", |w| w.reference(on, Kind::Either));
            }
            Amount::Sum(parts) => {
                for (i, part) in parts.iter().enumerate() {
                    self.amounting = Amounting::of(sharing);
                    self.at(format!("[{i}]"), |w| w.amount(part));
                }
            }
            Amount::Times { of, .. } | Amount::Half { of, .. } => {
                self.amounting = Amounting::of(sharing);
                self.at("of", |w| w.amount(of));
            }
            Amount::Divided => {
                if !sharing {
                    self.uses("Amount", "Divided.Elsewhere");
                }
                if !self.scope.targets.iter().any(|t| t.1) {
                    self.problem("`Divided` needs a target specification with `divide`");
                }
            }
            Amount::EvenShare(total) => {
                if !sharing {
                    self.uses("Amount", "EvenShare.Elsewhere");
                }
                if !self.scope.targets.iter().any(|t| t.2) {
                    self.problem(
                        "`EvenShare` needs a target specification with more than one target",
                    );
                }
                self.amounting = Amounting::of(sharing);
                self.amount(total);
            }
            Amount::TargetCount(index) => {
                let index = usize::from(*index);
                if index < self.scope.targets.len() {
                    self.scope.used.insert(index);
                } else {
                    self.problem(format!("target {index} does not exist"));
                }
            }
            Amount::Devotion(colors) => {
                self.uses("Amount", "Devotion");
                if colors.is_empty() {
                    self.problem("devotion is to at least one color");
                }
            }
            Amount::Greatest { of, .. } => {
                self.uses("Amount", "Greatest");
                self.at("of", |w| w.selector(of));
            }
            Amount::Total { of, .. } => {
                self.uses("Amount", "Total");
                self.at("of", |w| w.selector(of));
            }
            Amount::Distinct { of, .. } => self.w2c_selector_amount("Distinct", of),
            Amount::CardTypes(of) => self.w2c_selector_amount("CardTypes", of),
            Amount::Destroyed(of) => self.w2c_selector_amount("Destroyed", of),
            Amount::Players(who) => self.player_amount("Players", who),
            Amount::HighestLife(who) => self.player_amount("HighestLife", who),
            Amount::LowestLife(who) => self.player_amount("LowestLife", who),
            Amount::LowestHand(_) | Amount::LifeLostThisWay | Amount::DieResult(_) => {
                self.hand_or_life_lost(amount);
            }
            Amount::Kicks | Amount::Promised | Amount::Devoured => self.paid_amount(amount),
            Amount::Mutations(of) => {
                self.uses("Amount", "Mutations");
                self.at("of", |w| w.reference(of, Kind::Either));
            }
            Amount::CastWith(_) => self.uses("Amount", "CastWith"),
            Amount::ColorsSpent => self.uses("Amount", "ColorsSpent"),
            Amount::PermanentsGraved => self.uses("Amount", "PermanentsGraved"),
            Amount::Tallied { who, what } => self.tallied(who, *what),
            Amount::Largest { who, of } => {
                self.player_amount("Largest", who);
                self.at("of", |w| w.selector(of));
            }
            Amount::StartingLife(who) => self.player_amount("StartingLife", who),
            Amount::Party(who) => self.player_amount("Party", who),
            Amount::Triggering => self.c11e_triggering(),
        }
    }

    /// An amount that counts over the objects a selector names.
    fn w2c_selector_amount(&mut self, tag: &str, of: &Selector) {
        self.uses("Amount", tag);
        self.selector(of);
    }

    /// The amount of the event, which only a triggered ability has.
    fn c11e_triggering(&mut self) {
        self.uses("Amount", "Triggering");
        if !self.scope.in_trigger {
            self.problem("`Triggering` is only valid in a triggered ability");
        }
    }

    fn bounds(&mut self, bounds: &Bounds) {
        if let (Some(min), Some(Amount::Fixed(max))) = (bounds.fixed_min(), &bounds.max)
            && i64::from(min) > i64::from(*max)
        {
            self.problem(format!("the minimum {min} is above the maximum {max}"));
        }
        self.at("min", |w| w.amount(&bounds.min));
        if let Some(max) = &bounds.max {
            self.at("max", |w| w.amount(max));
        }
    }

    fn condition(&mut self, condition: &Condition) {
        match condition {
            Condition::Not(inner) => self.at("Not", |w| w.condition(inner)),
            Condition::All(list) | Condition::Any(list) => {
                if list.is_empty() {
                    self.problem("needs at least one condition");
                }
                for (i, inner) in list.iter().enumerate() {
                    self.at(format!("[{i}]"), |w| w.condition(inner));
                }
            }
            Condition::Compare { left, right, .. } => {
                self.at("left", |w| w.amount(left));
                self.at("right", |w| w.amount(right));
            }
            Condition::Matches { subject, selector } => {
                self.at("subject", |w| w.reference(subject, Kind::Object));
                self.at("selector", |w| w.selector(selector));
            }
            Condition::Kicked => {
                self.uses("Condition", "Kicked");
                if !self.has_kicker {
                    self.problem("`Kicked` needs a Kicker or Multikicker keyword on the card");
                }
            }
            Condition::KickedWith(cost) => {
                self.uses("Condition", "KickedWith");
                if !self.kickers.contains(cost) {
                    self.problem("`KickedWith` needs a Kicker keyword with that cost on the card");
                }
            }
            Condition::AdditionalCostPaid => {
                self.uses("Condition", "AdditionalCostPaid");
                if !self.has_additional_cost {
                    self.problem(
                        "`AdditionalCostPaid` needs an `AdditionalCost` ability on the card",
                    );
                }
            }
            Condition::CastWith(_) => self.uses("Condition", "CastWith"),
            Condition::ManaSpent(_) => self.uses("Condition", "ManaSpent"),
            Condition::YourTurn => {}
            Condition::Performed => {
                self.uses("Condition", "Performed");
                // The engine knows whether a `May` was accepted, whether a `CastFromZone` cast or played the card and whether a `Sacrifice` or a `RemoveCounters` took something; any other effect before it waits.
                if !self.after_may {
                    self.uses("Condition", "Performed.Other");
                }
                if self.after != After::Condition {
                    self.problem("`Performed` is the condition of an `If` that follows the effect it asks about");
                }
            }
            Condition::Monarch(who) => {
                self.uses("Condition", "Monarch");
                self.at("Monarch", |w| w.reference(who, Kind::Player));
            }
            Condition::MonarchAtTurnStart(who) => {
                self.uses("Condition", "MonarchAtTurnStart");
                self.at("MonarchAtTurnStart", |w| w.reference(who, Kind::Player));
            }
            Condition::CitysBlessing(who) => {
                self.uses("Condition", "CitysBlessing");
                self.at("CitysBlessing", |w| w.reference(who, Kind::Player));
            }
            Condition::IsDay => self.uses("Condition", "IsDay"),
            Condition::IsNight => self.uses("Condition", "IsNight"),
            Condition::Step(_) => self.uses("Condition", "Step"),
            Condition::BeforeAttackers => self.uses("Condition", "BeforeAttackers"),
            Condition::InCombat => self.uses("Condition", "InCombat"),
            Condition::FirstCombat => self.uses("Condition", "FirstCombat"),
            Condition::NeitherDayNorNight => self.uses("Condition", "NeitherDayNorNight"),
            Condition::NewlyControlled(what) => {
                self.uses("Condition", "NewlyControlled");
                self.at("NewlyControlled", |w| w.reference(what, Kind::Object));
            }
            Condition::Level { of, .. } => {
                self.uses("Condition", "Level");
                self.at("Level", |w| w.reference(of, Kind::Object));
            }
        }
    }

    /// The effects of a payment: an `{X}` its mana announces is the X they read ("you may pay {X}{R}. If you do, it
    /// deals X damage"), whatever the mana cost of the card.
    fn c11f_paid_x(&mut self, cost: &Cost, paid: &[Effect]) {
        let (saved, saved_now) = (self.x_free, self.x_now);
        self.x_free |= has_x_mana(cost);
        // The resolution reads the X the payer announced (`Effect.Pay.X`), wherever the ability stands.
        self.x_now |= has_x_mana(cost);
        self.effect_list("paid", paid, false);
        (self.x_free, self.x_now) = (saved, saved_now);
    }

    /// The cost of an `Effect::Pay`: life of a counted amount ("pay that much life") is counted as the effect
    /// resolves (`Effect.Pay.Counted`); every other part is named as in any cost.
    fn c6_eg_paid_cost(&mut self, cost: &Cost) {
        let counted: Vec<(usize, &Amount)> = cost
            .0
            .iter()
            .enumerate()
            .filter_map(|(i, item)| match item {
                CostItem::PayLife(amount) if !is_fixed(item) => Some((i, amount)),
                _ => None,
            })
            .collect();
        if counted.is_empty() {
            return self.cost(cost);
        }
        let mut fixed = cost.clone();
        for (i, _) in &counted {
            fixed.0[*i] = CostItem::PayLife(Amount::Fixed(0));
        }
        self.cost(&fixed);
        self.uses("Effect", "Pay.Counted");
        for (i, amount) in counted {
            self.at(format!("[{i}]"), |w| w.amount(amount));
        }
    }

    fn cost(&mut self, cost: &Cost) {
        for (i, item) in cost.0.iter().enumerate() {
            self.at(format!("[{i}]"), |w| {
                if !matches!(item, CostItem::OneOf(_)) {
                    w.uses("Cost", &tag_of(item));
                }
                if !is_fixed(item) {
                    w.uses("Cost", &format!("{}.Counted", tag_of(item)));
                }
                match item {
                    CostItem::PayLife(amount)
                    | CostItem::PayEnergy(amount)
                    | CostItem::Mill(amount) => {
                        w.amount(amount);
                    }
                    CostItem::RemoveCounters { count, .. } => {
                        w.at("count", |w| w.amount(count));
                    }
                    CostItem::Sacrifice { filter, count }
                    | CostItem::TapPermanents { filter, count }
                    | CostItem::ExileFromGraveyard { filter, count }
                    | CostItem::ExileFromHand { filter, count }
                    | CostItem::RevealFromHand { filter, count }
                    | CostItem::ReturnToHand { filter, count } => {
                        w.at("filter", |w| w.selector(filter));
                        w.at("count", |w| w.amount(count));
                    }
                    CostItem::OneOf(costs) => w.alternatives(costs),
                    CostItem::Discard { count, filter } => {
                        w.at("count", |w| w.amount(count));
                        if let Some(filter) = filter {
                            w.at("filter", |w| w.selector(filter));
                        }
                    }
                    CostItem::TapPower { filter, .. } => {
                        w.at("filter", |w| w.selector(filter));
                    }
                    CostItem::Changed { per, condition, .. } => {
                        w.at("per", |w| w.amount(per));
                        if let Some(condition) = condition {
                            w.at("condition", |w| w.condition(condition));
                        }
                    }
                    CostItem::Mana(_)
                    | CostItem::Tap
                    | CostItem::Untap
                    | CostItem::SacrificeSelf
                    | CostItem::ExileSelf
                    | CostItem::DiscardSelf => {}
                }
            });
        }
    }

    /// A choice between costs: the caster pays one, which only a required additional cost of a spell asks. Each cost is a
    /// plain cost of at least one part.
    fn alternatives(&mut self, costs: &[Cost]) {
        let tag = if self.choosing {
            "OneOf"
        } else {
            "OneOf.Elsewhere"
        };
        self.uses("Cost", tag);
        let plain = |cost: &Cost| {
            !cost.0.is_empty() && !cost.0.iter().any(|item| matches!(item, CostItem::OneOf(_)))
        };
        if costs.len() < 2 || !costs.iter().all(plain) {
            self.problem(
                "a choice between costs has two or more costs of at least one part, none a choice itself",
            );
        }
        for (i, cost) in costs.iter().enumerate() {
            self.at(format!("[{i}]"), |w| w.cost(cost));
        }
    }

    /// An additional cost is paid by the caster as the spell is announced, never by the spell itself. The announcement
    /// pays the parts of a required one (life, picked objects) and its mana, but not an `{X}` of mana.
    fn additional_cost(&mut self, cost: &Cost, required: bool) {
        let x_mana = cost
            .0
            .iter()
            .any(|item| matches!(item, CostItem::Mana(mana) if mana.has_x()));
        if required && x_mana {
            self.uses("Ability", "AdditionalCost.Required.Mana.X");
        }
        // The X a required cost counts is the one the caster announces as the spell is cast.
        let saved = std::mem::replace(&mut self.x_now, required && cost.has_x());
        let choosing = std::mem::replace(&mut self.choosing, required);
        if cost.0.len() > 1 && cost.0.iter().any(|item| matches!(item, CostItem::OneOf(_))) {
            self.problem("a choice between costs is the only part of an additional cost");
        }
        self.at("cost", |w| w.cost(cost));
        self.choosing = choosing;
        self.x_now = saved;
        let by_the_source = cost.0.iter().any(|item| {
            matches!(
                item,
                CostItem::Tap
                    | CostItem::Untap
                    | CostItem::SacrificeSelf
                    | CostItem::ExileSelf
                    | CostItem::DiscardSelf
                    | CostItem::RemoveCounters { .. }
            )
        });
        if by_the_source {
            self.problem("an additional cost is paid by the caster, not by the spell itself");
        }
        self.parts_a_spell_cost_does_not_pay("Ability", "AdditionalCost", cost, required);
    }

    /// The parts of the cost of a keyword of a spell (kicker, buyback, entwine, flashback, evoke, madness) that the
    /// announcement of the spell does not pay: counters removed from the source, which a card in a hand or a graveyard
    /// has none of, and an amount that is the X announced, which the cost of a spell does not count. They are named
    /// `<kind>.<prefix>.<part>`. The announcement asks for the X of a required additional cost (`x_paid`).
    fn parts_a_spell_cost_does_not_pay(
        &mut self,
        kind: &str,
        prefix: &str,
        cost: &Cost,
        x_paid: bool,
    ) {
        for item in &cost.0 {
            if matches!(item, CostItem::RemoveCounters { .. }) {
                self.uses(kind, &format!("{prefix}.{}", tag_of(item)));
            } else if amount_is_x(item) && !x_paid {
                self.uses(kind, &format!("{prefix}.{}.X", tag_of(item)));
            }
        }
    }

    /// The parts of a cost that an ability pays as it is announced (life, counters, a pick of permanents or cards) are not
    /// paid in a payment an effect asks for or in the cost of a keyword that is not an ability: they are named
    /// `<kind>.<prefix>.<part>` so the card waits for an engine that pays them there.
    fn parts_that_need_an_announcement(&mut self, kind: &str, prefix: &str, cost: &Cost) {
        for item in &cost.0 {
            if matches!(
                item,
                CostItem::PayLife(_)
                    | CostItem::Mill(_)
                    | CostItem::RemoveCounters { .. }
                    | CostItem::Sacrifice { .. }
                    | CostItem::Discard { .. }
                    | CostItem::ExileFromGraveyard { .. }
                    | CostItem::ExileFromHand { .. }
                    | CostItem::RevealFromHand { .. }
                    | CostItem::TapPermanents { .. }
                    | CostItem::TapPower { .. }
                    | CostItem::ReturnToHand { .. }
            ) {
                self.uses(kind, &format!("{prefix}.{}", tag_of(item)));
            }
        }
    }

    /// A payment an effect asks takes mana, life, counters, energy and picked objects. Losing the source (sacrificing,
    /// exiling or discarding it) is paid as part of an ability's cost; no effect asks a player to pay it yet, so a
    /// payment that asks for it names what is missing.
    fn payment_of_the_source(&mut self, cost: &Cost) {
        for (item, name) in [
            (CostItem::SacrificeSelf, "Pay.SacrificeSelf"),
            (CostItem::ExileSelf, "Pay.ExileSelf"),
            (CostItem::DiscardSelf, "Pay.DiscardSelf"),
        ] {
            if cost.0.contains(&item) {
                self.uses("Effect", name);
            }
        }
        // The payer announces the X of an `{X}` once they say yes; a part that counts X is not asked there.
        if has_x_mana(cost) {
            self.uses("Effect", "Pay.X");
        }
        if cost.0.iter().any(amount_is_x) {
            self.uses("Effect", "Pay.X.Parts");
        }
    }

    /// An ability is activated from the battlefield, a hand, a graveyard, exile or the command zone, and a cost that
    /// needs its source in a place is only paid from there: `{T}`, `{Q}` and sacrificing it on the battlefield,
    /// discarding it from a hand.
    fn activated_zone(&mut self, cost: &Cost, from: ZoneKind) {
        if matches!(from, ZoneKind::Library | ZoneKind::Stack) {
            self.problem(
                "an activated ability works from the battlefield, a hand, a graveyard, exile or the command zone",
            );
        }
        for item in &cost.0 {
            let (needs, place) = match item {
                CostItem::Tap
                | CostItem::Untap
                | CostItem::SacrificeSelf
                | CostItem::RemoveCounters { .. } => (ZoneKind::Battlefield, "the battlefield"),
                CostItem::DiscardSelf => (ZoneKind::Hand, "a hand"),
                _ => continue,
            };
            if from != needs {
                self.problem(format!("`{}` is paid from {place}", tag_of(item)));
            }
        }
    }

    fn backup(&mut self, count: u32, abilities: &[Ability]) {
        if count == 0 {
            self.problem("backup needs a number of at least 1");
        }
        // The abilities that follow the keyword are what the target gains.
        for ability in abilities {
            match ability {
                Ability::Keyword(granted) => {
                    self.granted.push(granted.clone());
                    self.keyword(granted);
                }
                other => self.granted(other),
            }
        }
    }

    /// A companion condition may only ask about the card itself, since the deck check reads it from the card alone.
    fn companion(&mut self, condition: &crate::companion::DeckCondition) {
        for pred in condition.unreadable() {
            self.problem(format!(
                "a companion condition reads the card alone, not `{pred}`"
            ));
        }
    }

    /// A keyword ability: the card's own, or the one a token or a granted ability has, which grants that keyword.
    fn keyword_ability(&mut self, keyword: &Keyword) {
        if self.nested > 0 {
            self.granted.push(keyword.clone());
        }
        self.keyword(keyword);
    }

    fn keyword(&mut self, keyword: &Keyword) {
        self.uses("Keyword", &tag_of(keyword));
        match keyword {
            Keyword::Ward(cost) | Keyword::Echo(cost) | Keyword::CumulativeUpkeep(cost) => {
                self.at("cost", |w| w.cost(cost));
                // Ward, echo and cumulative upkeep ask the player for their cost through a payment.
                self.payment_of_the_source(cost);
            }
            // The cost of these is paid as the cost of an activated ability the keyword adds.
            Keyword::Equip(cost)
            | Keyword::Reconfigure(cost)
            | Keyword::Cycling(cost)
            | Keyword::Unearth(cost)
            | Keyword::Embalm(cost)
            | Keyword::Eternalize(cost)
            | Keyword::Scavenge(cost)
            | Keyword::Transmute(cost)
            | Keyword::Ninjutsu(cost)
            | Keyword::LevelUp(cost)
            | Keyword::Outlast(cost) => {
                self.at("cost", |w| w.cost(cost));
            }
            // The cost of these is paid as the spell is announced: the mana, and the parts the engine pays there.
            Keyword::Kicker(cost)
            | Keyword::Squad(cost)
            | Keyword::Replicate(cost)
            | Keyword::Offspring(cost)
            | Keyword::Flashback(cost)
            | Keyword::Madness(cost)
            | Keyword::Miracle(cost)
            | Keyword::Buyback(cost)
            | Keyword::Entwine(cost)
            | Keyword::Evoke(cost)
            | Keyword::Escape(cost)
            | Keyword::Dash(cost)
            | Keyword::Bestow(cost)
            | Keyword::Blitz(cost)
            | Keyword::Prowl(cost)
            | Keyword::Mayhem(cost)
            | Keyword::Emerge(cost)
            | Keyword::Harmonize(cost)
            | Keyword::Spectacle(cost) => {
                self.at("cost", |w| w.cost(cost));
                self.parts_a_spell_cost_does_not_pay("Keyword", "Cost", cost, false);
            }
            Keyword::Multikicker(cost)
            | Keyword::Morph(cost)
            | Keyword::Megamorph(cost)
            | Keyword::Disguise(cost)
            | Keyword::Overload(cost)
            | Keyword::Mutate(cost)
            | Keyword::Encore(cost)
            | Keyword::Foretell(cost)
            | Keyword::Awaken { cost, .. }
            | Keyword::Suspend { cost, .. } => {
                self.at("cost", |w| w.cost(cost));
                // Only mana is paid for these: a part that takes objects, life or counters is not.
                self.parts_that_need_an_announcement("Keyword", "Cost", cost);
            }
            Keyword::Crew(0) => self.problem("crew needs a power of at least 1"),
            Keyword::Backup { count, abilities } => self.backup(*count, abilities),
            Keyword::Saddle(0) => self.problem("saddle needs a power of at least 1"),
            Keyword::Enchant(selector) => {
                self.selector(selector);
                // An Aura is attached to a permanent; one that enchants a card in another zone waits for the cards that do.
                if selector
                    .0
                    .iter()
                    .any(|pred| matches!(pred, Pred::In(zone) if !zone.is_battlefield()))
                {
                    self.uses("Keyword", "Enchant.Elsewhere");
                }
            }
            Keyword::Protection(quality)
            | Keyword::Landwalk(quality)
            | Keyword::Affinity(quality) => {
                self.selector(quality);
            }
            // The cost is paid as the cost of the activated ability the keyword adds.
            Keyword::Typecycling { find, cost } => {
                self.at("cost", |w| w.cost(cost));
                self.at("find", |w| w.selector(find));
            }
            Keyword::EquipTo { to, cost } => {
                self.at("cost", |w| w.cost(cost));
                self.at("to", |w| w.selector(to));
            }
            Keyword::Toxic(0) => self.problem("toxic needs a number of at least 1"),
            Keyword::Casualty(0) => self.problem("casualty needs a power of at least 1"),
            Keyword::Hideaway(0) => self.problem("hideaway needs a number of at least 1"),
            Keyword::Devour { counters, of } => {
                if *counters == 0 {
                    self.problem("devour needs a number of at least 1");
                }
                self.at("of", |w| w.selector(of));
            }
            Keyword::Annihilator(0) => self.problem("annihilator needs a number of at least 1"),
            Keyword::Dredge(0) => self.problem("dredge needs a number of at least 1"),
            Keyword::Fabricate(0) => self.problem("fabricate needs a number of at least 1"),
            Keyword::Afterlife(0) => self.problem("afterlife needs a number of at least 1"),
            Keyword::Companion(condition) => self.companion(condition),
            Keyword::PartnerWith(name) => {
                self.names
                    .push((self.face, self.current_path(), name.clone()));
            }
            _ => {}
        }
    }

    fn modification(&mut self, modification: &Modification) {
        self.uses("Mod", &tag_of(modification));
        match modification {
            Modification::ModifyPT { power, toughness }
            | Modification::SetPT { power, toughness } => {
                self.at("power", |w| w.amount(power));
                self.at("toughness", |w| w.amount(toughness));
            }
            Modification::AddKeyword(keyword) | Modification::RemoveKeyword(keyword) => {
                if matches!(modification, Modification::AddKeyword(_)) {
                    self.granted.push(keyword.clone());
                }
                self.keyword(keyword);
            }
            Modification::SetController(who) => self.reference(who, Kind::Player),
            Modification::GrantAbility(granted) => self.granted(granted),
            Modification::CantBeBlockedBy(selector) | Modification::CanBlockOnly(selector) => {
                self.selector(selector);
            }
            Modification::MinBlockers(count) | Modification::MaxBlockers(count) => {
                if *count < 1 {
                    self.problem("a limit on blockers needs a count of at least one");
                }
            }
            Modification::SwitchPT
            | Modification::AddType(_)
            | Modification::RemoveType(_)
            | Modification::SetTypes(_)
            | Modification::RemoveSupertype(_)
            | Modification::AddSubtype(_)
            | Modification::SetCreatureTypes(_)
            | Modification::AddAllCreatureTypes
            | Modification::SetColors(_)
            | Modification::AddProtectionFromChosenColor
            | Modification::LoseAbilities
            | Modification::CantAttack
            | Modification::CantAttackYou(_)
            | Modification::CantBlock
            | Modification::DoesntUntap
            | Modification::CantBeBlocked
            | Modification::Goaded => {}
        }
    }

    /// An ability a modification grants is checked as the ability it is, in a scope of its own. Only an activated, a
    /// triggered or a static one can be granted: a keyword is `AddKeyword`, and the others work only as printed.
    fn granted(&mut self, granted: &Ability) {
        if !matches!(
            granted,
            Ability::Activated { .. } | Ability::Triggered { .. } | Ability::Static { .. }
        ) {
            self.problem("only an activated, a triggered or a static ability can be granted (a keyword is `AddKeyword`)");
            return;
        }
        // A given static ability is named by what gives it, so it cannot give abilities in turn, and a characteristic-defining
        // ability is printed on the card it defines (CR 604.3a).
        if let Ability::Static { mods, defining, .. } = granted {
            if *defining {
                self.problem("a static ability that is granted cannot be characteristic-defining");
            }
            if mods
                .iter()
                .any(|modification| matches!(modification, Modification::GrantAbility(_)))
            {
                self.problem("a static ability that is granted cannot grant abilities in turn");
            }
        }
        let scope = std::mem::take(&mut self.scope);
        self.nested += 1;
        self.at("GrantAbility", |w| w.ability(granted));
        self.nested -= 1;
        self.scope = scope;
    }

    /// A characteristic-defining ability (CR 604.3a) defines the power and toughness, colours, types or subtypes of its own
    /// object with no condition, and works in every zone, so it names no zone.
    fn defining(
        &mut self,
        affected: &Ref,
        mods: &[Modification],
        conditional: bool,
        from: crate::refs::ZoneKind,
    ) {
        self.uses("Ability", "Static.Defining");
        if !matches!(affected, Ref::This) {
            self.at("affected", |w| {
                w.problem("a characteristic-defining ability affects only its own object (`This`)");
            });
        }
        if conditional {
            self.at("condition", |w| {
                w.problem("a characteristic-defining ability has no condition (CR 604.3a)");
            });
        }
        if !from.is_battlefield() {
            self.at("from", |w| {
                w.problem(
                    "a characteristic-defining ability works in every zone, so it names none",
                );
            });
        }
        for (i, modification) in mods.iter().enumerate() {
            let defines = matches!(
                modification,
                Modification::SetPT { .. }
                    | Modification::SetColors(_)
                    | Modification::AddType(_)
                    | Modification::AddSubtype(_)
            );
            if !defines {
                self.at("mods", |w| {
                    w.at(format!("[{i}]"), |w| {
                        w.problem("a characteristic-defining ability only sets power and toughness, colours, types or subtypes");
                    });
                });
            }
        }
    }

    fn mods(&mut self, mods: &[Modification]) {
        if mods.is_empty() {
            self.problem("needs at least one modification");
        }
        for (i, modification) in mods.iter().enumerate() {
            self.at(format!("[{i}]"), |w| {
                w.modification(modification);
                if matches!(modification, Modification::RemoveSupertype(_)) {
                    w.problem("a supertype is taken away only from the copiable values of a copy (`except`)");
                }
            });
        }
    }

    fn trigger_pattern(&mut self, pattern: &TriggerPattern) {
        self.uses("Trigger", &tag_of(pattern));
        if let TriggerPattern::Expend { n: 0, .. } = pattern {
            self.problem("expend needs a number of at least 1");
        }
        match pattern {
            TriggerPattern::ZoneChange { what, .. }
            | TriggerPattern::ZoneChangeExcept { what, .. }
            | TriggerPattern::Attacks { what }
            | TriggerPattern::AttacksAlone { what } => {
                self.at("what", |w| w.selector(what));
            }
            TriggerPattern::AttacksPlayer { what, at }
            | TriggerPattern::AttacksPlayerOrPlaneswalker { what, at } => {
                self.at("what", |w| w.selector(what));
                self.at("at", |w| w.reference(at, Kind::Player));
            }
            TriggerPattern::PlayerAttacked { who } => {
                self.at("who", |w| w.reference(who, Kind::Player));
            }
            TriggerPattern::PlayerAttacks { by, at } => {
                self.at("by", |w| w.reference(by, Kind::Player));
                self.at("at", |w| w.reference(at, Kind::Player));
            }
            TriggerPattern::BecomesTarget { what, by } => {
                self.at("what", |w| w.selector(what));
                self.at("by", |w| w.selector(by));
            }
            TriggerPattern::SpellCast { what, by }
            | TriggerPattern::SpellCopied { what, by }
            | TriggerPattern::Sacrificed { what, by }
            | TriggerPattern::Milled { what, by } => {
                self.at("what", |w| w.selector(what));
                self.at("by", |w| w.reference(by, Kind::Player));
            }
            TriggerPattern::StepBegins { whose, .. } => {
                self.at("whose", |w| w.reference(whose, Kind::Player));
            }
            TriggerPattern::DayNightChanges { .. } => {}
            TriggerPattern::DealsDamage { source, to, .. } => {
                self.at("source", |w| w.selector(source));
                if let Some(to) = to {
                    self.at("to", |w| w.reference(to, Kind::Either));
                }
            }
            TriggerPattern::LifeGained { who }
            | TriggerPattern::LifeLost { who }
            | TriggerPattern::BecomesMonarch { who }
            | TriggerPattern::Proliferated { who }
            | TriggerPattern::CoinFlipWon { who }
            | TriggerPattern::DiceRolled { who, .. }
            | TriggerPattern::Amassed { who }
            | TriggerPattern::Surveilled { who }
            | TriggerPattern::Bolstered { who }
            | TriggerPattern::Expend { who, .. } => {
                self.at("who", |w| w.reference(who, Kind::Player));
            }
            TriggerPattern::Drew { who, what } => {
                self.at("who", |w| w.reference(who, Kind::Player));
                self.at("what", |w| w.selector(what));
            }
            TriggerPattern::Discarded { what, by }
            | TriggerPattern::TappedForMana { what, by }
            | TriggerPattern::Cycled { what, by } => {
                self.at("what", |w| w.selector(what));
                self.at("by", |w| w.reference(by, Kind::Player));
            }
            TriggerPattern::Tapped { what }
            | TriggerPattern::Explored { what }
            | TriggerPattern::Connived { what }
            | TriggerPattern::BecomesMonstrous { what }
            | TriggerPattern::BecomesRenowned { what }
            | TriggerPattern::Exploits { what }
            | TriggerPattern::BecomesCrewed { what }
            | TriggerPattern::BecomesSaddled { what }
            | TriggerPattern::BecomesLevel { what, .. }
            | TriggerPattern::TurnedFaceUp { what }
            | TriggerPattern::Mutates { what }
            | TriggerPattern::Adapted { what }
            | TriggerPattern::Untapped { what }
            | TriggerPattern::Blocks { what }
            | TriggerPattern::BecomesBlocked { what }
            | TriggerPattern::Unblocked { what }
            | TriggerPattern::CountersPut { what, .. }
            | TriggerPattern::CountersRemoved { what, .. } => {
                self.at("what", |w| w.selector(what));
            }
            TriggerPattern::Attached { what, to } => {
                self.at("what", |w| w.selector(what));
                self.at("to", |w| w.selector(to));
            }
            TriggerPattern::GroupAttacks { .. } | TriggerPattern::Chapter { .. } => {
                self.w2e_counted(pattern);
            }
            TriggerPattern::State { .. } => {
                self.problem("a state is the whole pattern of a triggered ability");
            }
            TriggerPattern::AnyOf(patterns) => self.any_of(patterns),
            TriggerPattern::Batch(inner) => self.batch(inner),
            TriggerPattern::Nth { event, nth } => self.nth(event, *nth),
        }
    }

    /// The patterns that count: the attackers of a `GroupAttacks` and the number of a `Chapter`.
    fn w2e_counted(&mut self, pattern: &TriggerPattern) {
        match pattern {
            TriggerPattern::GroupAttacks { what, at_least } => {
                self.at("what", |w| w.selector(what));
                if *at_least == 0 {
                    self.at("at_least", |w| w.problem("at least one creature attacks"));
                }
            }
            TriggerPattern::Chapter { number } if *number == 0 => {
                self.at("number", |w| w.problem("the first chapter is 1"));
            }
            _ => {}
        }
    }

    /// The patterns of an `AnyOf`: at least two; a batch may be one of them ("When this enters and whenever one or more
    /// tokens you control deal combat damage to a player") but then the list holds events of one kind of pattern each.
    fn any_of(&mut self, patterns: &[TriggerPattern]) {
        if patterns.len() < 2 {
            self.problem("`AnyOf` needs at least two patterns");
        }
        for (i, inner) in patterns.iter().enumerate() {
            self.at(format!("[{i}]"), |w| w.trigger_pattern(inner));
        }
    }

    /// A batch is of one event that can happen to several things at once.
    fn batch(&mut self, inner: &TriggerPattern) {
        if let TriggerPattern::AnyOf(members) = inner
            && c6_gap_batch_of_moves(members)
        {
            self.at("Batch", |w| w.trigger_pattern(inner));
            return;
        }
        if matches!(
            inner,
            TriggerPattern::Batch(_)
                | TriggerPattern::Nth { .. }
                | TriggerPattern::AnyOf(_)
                | TriggerPattern::State { .. }
                | TriggerPattern::StepBegins { .. }
                | TriggerPattern::BecomesMonarch { .. }
                | TriggerPattern::DayNightChanges { .. }
                | TriggerPattern::Proliferated { .. }
                | TriggerPattern::CoinFlipWon { .. }
                | TriggerPattern::DiceRolled { .. }
                | TriggerPattern::Amassed { .. }
                | TriggerPattern::Explored { .. }
                | TriggerPattern::Connived { .. }
                | TriggerPattern::BecomesMonstrous { .. }
                | TriggerPattern::BecomesRenowned { .. }
                | TriggerPattern::Exploits { .. }
                | TriggerPattern::BecomesCrewed { .. }
                | TriggerPattern::BecomesSaddled { .. }
                | TriggerPattern::BecomesLevel { .. }
                | TriggerPattern::TurnedFaceUp { .. }
                | TriggerPattern::Mutates { .. }
                | TriggerPattern::Adapted { .. }
                | TriggerPattern::Bolstered { .. }
                | TriggerPattern::Expend { .. }
                | TriggerPattern::Surveilled { .. }
                | TriggerPattern::SpellCast { .. }
                | TriggerPattern::BecomesTarget { .. }
                | TriggerPattern::GroupAttacks { .. }
                | TriggerPattern::Chapter { .. }
        ) {
            self.problem("a batch is of one event that can happen to several things at once");
        }
        self.at("Batch", |w| w.trigger_pattern(inner));
    }
}

/// Whether a batch of several patterns is of moves that each name the zone left, none of them the battlefield: the
/// engine judges those events together against the state after them, so one batch holds them all ("cards are put into
/// exile from your library and/or your graveyard").
fn c6_gap_batch_of_moves(members: &[TriggerPattern]) -> bool {
    members.iter().all(|member| {
        matches!(
            member,
            TriggerPattern::ZoneChange { from: Some(from), .. } if *from != ZoneKind::Battlefield
        )
    })
}

impl Walker<'_> {
    /// The nth time in a turn is counted for cards drawn, spells cast and life gained by a player, and for attacks,
    /// targets (of what anyone controls or the controller of the ability) and damage dealt to an object.
    fn nth(&mut self, event: &TriggerPattern, nth: u32) {
        if nth == 0 {
            self.at("nth", |w| w.problem("the first time is 1"));
        }
        let counted = match event {
            TriggerPattern::Drew { .. }
            | TriggerPattern::SpellCast { .. }
            | TriggerPattern::LifeGained { .. }
            | TriggerPattern::LifeLost { .. }
            | TriggerPattern::Surveilled { .. }
            | TriggerPattern::Attacks { .. }
            | TriggerPattern::BecomesCrewed { .. }
            | TriggerPattern::BecomesSaddled { .. }
            // The counters put on an object, of any kind.
            | TriggerPattern::CountersPut { counter: None, .. } => true,
            // Every cycling of the player, whatever the card.
            TriggerPattern::Cycled { what, .. } => what.0.iter().all(|p| *p == Pred::Other),
            // Tokens under the player's control, from no zone.
            TriggerPattern::ZoneChange {
                what,
                from: None,
                to,
            } => {
                what.0 == [Pred::Token, Pred::ControlledBy(Ref::You)]
                    && matches!(to, None | Some(ZoneKind::Battlefield))
            }
            TriggerPattern::BecomesTarget { by, .. } => {
                by.is_empty() || by.0 == [Pred::ControlledBy(Ref::You)]
            }
            // Damage is counted for the object that is dealt it, from any source.
            TriggerPattern::DealsDamage {
                source,
                to: Some(to),
                combat: None,
            } => source.is_empty() && !matches!(to, Ref::You | Ref::Opponents | Ref::EachPlayer),
            _ => false,
        };
        if !counted {
            self.problem(
                "only cards drawn, spells cast, life gained or lost, cards cycled, surveils, tokens you create, attacks, counters put on an object, becoming crewed or saddled, becoming a target of what anyone or you control and damage dealt to an object from any source are counted in a turn",
            );
        }
        self.at("event", |w| w.trigger_pattern(event));
    }

    /// The pattern of a triggered ability, the only place a state may stand. No event is behind a state, so nothing in
    /// it is the triggering one.
    fn trigger_when(&mut self, when: &TriggerPattern) {
        let TriggerPattern::State { condition } = when else {
            self.trigger_pattern(when);
            return;
        };
        self.uses("Trigger", "State");
        let saved = std::mem::replace(&mut self.scope.in_trigger, false);
        self.at("State", |w| w.at("condition", |w| w.condition(condition)));
        self.scope.in_trigger = saved;
    }

    fn target_spec(&mut self, spec: &TargetSpec) {
        // X is announced before the targets are chosen, so what may be targeted reads it.
        self.at("what", |w| w.targetable(&spec.what));
        self.at("bounds", |w| w.bounds(&spec.bounds));
        if let Some(divide) = &spec.divide {
            self.at("divide", |w| w.amount(divide));
        }
    }

    fn targetable(&mut self, what: &Targetable) {
        match what {
            Targetable::Object(selector) => self.selector(selector),
            Targetable::Either(list) => {
                if list.len() < 2 {
                    self.problem("`Either` needs at least two kinds");
                }
                for (i, inner) in list.iter().enumerate() {
                    self.at(format!("[{i}]"), |w| w.targetable(inner));
                }
            }
            Targetable::PerPlayer { players, what } => {
                self.uses("Target", "PerPlayer");
                self.at("players", |w| w.reference(players, Kind::Player));
                self.at("what", |w| w.selector(what));
            }
            Targetable::AnyTarget | Targetable::Player | Targetable::Opponent => {}
        }
    }

    fn destination(&mut self, to: &Destination) {
        if let Destination::Battlefield {
            under: Some(who), ..
        } = to
        {
            self.at("under", |w| w.reference(who, Kind::Player));
        }
    }

    fn token(&mut self, token: &TokenRef) {
        match token {
            TokenRef::Named(id) => {
                if !self.tokens.contains(id) {
                    self.problem(format!(
                        "unknown token `{id}` (no file cards/tokens/{id}.json)"
                    ));
                }
                // What the token does is part of what the card needs: a card that makes a Treasure needs its mana ability.
                if self.nested < MOST_NESTED
                    && let Some(spec) = crate::builtin::token_spec(id)
                {
                    self.at(id.clone(), |w| w.token_spec(&spec, false));
                }
            }
            TokenRef::Custom(spec) => self.at("Custom", |w| w.token_spec(spec, false)),
            TokenRef::Sized { size, token } => {
                self.at("size", |w| w.amount(size));
                self.at("token", |w| w.token_spec(token, true));
            }
            TokenRef::CopyOf { of, except, .. } => {
                self.at("of", |w| w.reference(of, Kind::Object));
                self.copy_exceptions(except);
            }
        }
    }

    /// The exceptions of a copy effect are changes to its copiable values; a modification that is not one waits.
    fn copy_exceptions(&mut self, except: &[Modification]) {
        for (i, modification) in except.iter().enumerate() {
            self.at(format!("except[{i}]"), |w| {
                w.modification(modification);
                if !crate::CardDefinition::new("").except(modification) {
                    w.uses("Copy", &format!("Except.{}", tag_of(modification)));
                }
            });
        }
    }

    fn token_spec(&mut self, spec: &TokenSpec, sized: bool) {
        if spec.name.is_empty() {
            self.problem("a token needs a name");
        }
        if spec.types.is_empty() {
            self.problem("a token needs at least one card type");
        }
        let creature = spec.types.contains(&mtg_core::card::CardType::Creature);
        if sized {
            if !creature || spec.power.is_some() || spec.toughness.is_some() {
                self.problem("a token sized by an amount is a creature with no power and toughness of its own");
            }
        } else if creature != (spec.power.is_some() && spec.toughness.is_some()) {
            self.problem("a creature token needs power and toughness, any other token none");
        }
        let saved = std::mem::take(&mut self.scope);
        self.nested += 1;
        for (i, ability) in spec.abilities.iter().enumerate() {
            self.at(format!("abilities[{i}]"), |w| w.ability(ability));
        }
        self.nested -= 1;
        self.scope = saved;
    }

    /// An emblem: the players who get it, and its abilities, each of a kind a permanent can have, walked as the abilities
    /// of a token are (in a scope of their own).
    fn w6b_emblem(&mut self, who: &Ref, abilities: &[Ability]) {
        self.at("who", |w| w.reference(who, Kind::Player));
        if abilities.is_empty() {
            self.problem("an emblem needs at least one ability");
        }
        let saved = std::mem::take(&mut self.scope);
        self.nested += 1;
        for (i, ability) in abilities.iter().enumerate() {
            if matches!(
                ability,
                Ability::Spell { .. } | Ability::Loyalty { .. } | Ability::Manual { .. }
            ) {
                self.problem("an emblem has no spell, loyalty or manual ability");
            }
            self.at(format!("abilities[{i}]"), |w| w.ability(ability));
        }
        self.nested -= 1;
        self.scope = saved;
    }

    fn effect_list(&mut self, field: &str, list: &[Effect], top: bool) {
        self.effect_list_with(field, list, top, None);
    }

    /// What a reference names: a player, an object, or either when it is a name or a target of either kind.
    fn kind_of(&self, r: &Ref) -> Kind {
        match r {
            Ref::You
            | Ref::Opponents
            | Ref::EachPlayer
            | Ref::AttachedPlayer
            | Ref::Monarch
            | Ref::Chosen
            | Ref::Promised
            | Ref::Attacked
            | Ref::MostLife
            | Ref::Defending
            | Ref::ControllerOf(_)
            | Ref::OwnerOf(_) => Kind::Player,
            Ref::This | Ref::Dealer | Ref::Attached | Ref::Paid | Ref::Each(_) => Kind::Object,
            Ref::Target(index) => self
                .scope
                .targets
                .get(usize::from(*index))
                .map_or(Kind::Either, |t| t.0),
            Ref::Bound(name) => self.scope.lookup(name).unwrap_or(Kind::Either),
            Ref::Triggering => Kind::Either,
        }
    }

    /// `RepeatEach`: the list it repeats over and the body, which sees the name of the one a pass is for.
    fn repeat_each(&mut self, over: &Ref, bind: &str, effects: &[Effect]) {
        self.at("over", |w| w.reference(over, Kind::Either));
        let kind = self.kind_of(over);
        if effects.is_empty() {
            self.problem("a repeat with nothing to do");
        }
        self.effect_list_with("effects", effects, false, Some((bind, kind)));
    }

    /// `RollDice`: a die of at least two faces and rows that each hold a range of totals; several dice only for a choice
    /// of one result, which has no table and adds nothing.
    fn roll_dice(
        &mut self,
        who: &Ref,
        sides: u32,
        (dice, choose): (u32, bool),
        plus: &Amount,
        results: &[DieRow],
    ) {
        self.at("who", |w| w.reference(who, Kind::Player));
        if sides < 2 {
            self.problem("a die has at least two sides");
        }
        if choose != (dice > 1) {
            self.problem(
                "a choice of one result is among two or more dice, and only a choice rolls several",
            );
        }
        if choose && (!results.is_empty() || !plus.is_zero()) {
            self.problem("a chosen result has no table and adds nothing");
        }
        self.at("plus", |w| w.amount(plus));
        for (i, row) in results.iter().enumerate() {
            self.at(format!("results[{i}]"), |w| {
                if row.to.is_some_and(|to| to < row.from) {
                    w.problem("a row ends below where it starts");
                }
                w.effect_list("effects", &row.effects, false);
            });
        }
    }

    /// `Vote`: at least two named choices that are names of their own, or the players or objects to vote for with the
    /// name of the winners.
    fn vote(&mut self, options: &VoteFor, secret: bool, bind: Option<&str>) {
        match options {
            VoteFor::Named(names) => {
                if names.len() < 2 {
                    self.problem("a vote has at least two choices");
                }
                if bind.is_some() {
                    self.problem("a vote on named choices binds the choices, not a winner");
                }
                for name in names {
                    self.at("options", |w| w.bind(name, Kind::Player));
                }
            }
            VoteFor::Of(of) => {
                if secret {
                    self.problem("only a vote on named choices is secret");
                }
                self.at("options", |w| w.reference(of, Kind::Either));
                let kind = self.kind_of(of);
                match bind {
                    Some(name) => self.at("as", |w| w.bind(name, kind)),
                    None => self.problem("a vote on players or objects names its winners"),
                }
            }
        }
    }

    /// `FlipCoin`: the player and both lists; a flip with no list still counts for "whenever you win a coin flip".
    fn flip_coin(&mut self, who: &Ref, win: &[Effect], lose: &[Effect]) {
        self.at("who", |w| w.reference(who, Kind::Player));
        self.effect_list("win", win, false);
        self.effect_list("lose", lose, false);
    }

    /// `Clash`: the player and both lists, in which the opponent is bound.
    fn w9b_clash(&mut self, who: &Ref, win: &[Effect], lose: &[Effect]) {
        self.at("who", |w| w.reference(who, Kind::Player));
        let clashed = Some((crate::effect::W9B_CLASHED, Kind::Player));
        self.effect_list_with("win", win, false, clashed);
        self.effect_list_with("lose", lose, false, clashed);
    }

    /// An effect list in a scope of its own, with the name of a repeat pass bound when there is one.
    fn effect_list_with(
        &mut self,
        field: &str,
        list: &[Effect],
        top: bool,
        pass: Option<(&str, Kind)>,
    ) {
        self.scope.names.push(BTreeMap::new());
        if let Some((name, kind)) = pass {
            self.at("as", |w| w.bind(name, kind));
        }
        let saved = (self.after, self.after_may);
        for (i, effect) in list.iter().enumerate() {
            let only = top && list.len() == 1;
            self.after = if i > 0 { After::Effect } else { After::Nothing };
            self.after_may = i > 0
                && matches!(
                    list.get(i - 1),
                    Some(
                        Effect::May { .. }
                            | Effect::CastFromZone { .. }
                            | Effect::Sacrifice { .. }
                            | Effect::RemoveCounters { .. }
                    )
                );
            self.at(format!("{field}[{i}]"), |w| w.effect(effect, only));
        }
        (self.after, self.after_may) = saved;
        self.scope.names.pop();
    }

    /// The mana an effect adds: no list that adds nothing, a choice of at least two, and the amounts and conditions it reads.
    fn produce(&mut self, mana: &Produce) {
        match mana {
            Produce::Fixed(list) if list.is_empty() => self.problem("adds no mana"),
            Produce::ChosenColor(0)
            | Produce::CommanderIdentity(0)
            | Produce::ProducedBy { count: 0, .. } => self.problem("adds no mana"),
            Produce::AnyColor(count) | Produce::AnyCombination { count, .. }
                if count.fixed() == Some(0) =>
            {
                self.problem("adds no mana");
            }
            Produce::AnyColor(count) => {
                self.at("AnyColor", |w| w.amount(count));
            }
            Produce::AnyCombination { among, .. } if among.len() == 1 => {
                self.problem("a combination of one type is that mana");
            }
            Produce::AnyCombination { count, .. } => {
                self.at("AnyCombination", |w| w.at("count", |w| w.amount(count)));
            }
            Produce::OneOf(list) if list.len() < 2 => {
                self.problem("a choice of mana needs at least two types");
            }
            Produce::OneOfLists(lists) if lists.len() < 2 => {
                self.problem("a choice of mana needs at least two lists");
            }
            Produce::OneOfLists(lists) if lists.len() > MANA_WAYS => {
                self.problem("a choice of mana has at most six lists");
            }
            Produce::OneOfLists(lists) if lists.iter().any(Vec::is_empty) => {
                self.problem("a list of mana adds no mana");
            }
            Produce::ProducedBy { among, .. } => {
                self.at("ProducedBy", |w| w.at("among", |w| w.selector(among)));
            }
            Produce::Instead {
                base,
                when,
                instead,
            } => {
                self.at("Instead", |w| {
                    w.at("base", |w| w.produce(base));
                    w.at("when", |w| w.condition(when));
                    w.at("instead", |w| w.produce(instead));
                });
            }
            Produce::Fixed(_)
            | Produce::OneOf(_)
            | Produce::OneOfLists(_)
            | Produce::ChosenColor(_)
            | Produce::OrChosenColor(_)
            | Produce::CommanderIdentity(_)
            | Produce::OfTapped => {}
        }
        // Mana the engine adds without a choice is a primitive of its own; a choice of mana is not.
        match mana {
            Produce::Fixed(_) => {}
            Produce::AnyColor(count) if count.fixed().is_some() => {}
            other => self.uses("Effect", &format!("AddMana.{}", tag_of(other))),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn effect(&mut self, effect: &Effect, only_effect: bool) {
        let tag = tag_of(effect);
        self.uses("Effect", &tag);
        // A discard at random needs the engine's randomness, which a discard of the player's choice does not.
        if matches!(effect, Effect::Discard { random: true, .. }) {
            self.uses("Effect", "Discard.Random");
        }
        // A move onto the battlefield needs entering permanents and a random order needs the engine's randomness;
        // a move to a hand, a graveyard or a library in the owner's order needs neither.
        if matches!(
            effect,
            Effect::Move {
                to: Destination::Battlefield {
                    transformed: true,
                    ..
                },
                ..
            }
        ) {
            self.uses("Effect", "Move.Transformed");
        }
        match effect {
            Effect::Move {
                to: Destination::Battlefield { .. },
                ..
            } => self.uses("Effect", "Move.Battlefield"),
            Effect::Move {
                to:
                    Destination::Library {
                        place: Place::Top,
                        order: Some(Order::Random),
                    },
                ..
            } => self.uses("Effect", "Move.Random.Top"),
            _ => {}
        }
        self.at(tag, |w| match effect {
            Effect::Draw { who, count }
            | Effect::Mill { who, count }
            | Effect::Scry { who, count }
            | Effect::Surveil { who, count } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("count", |w| w.amount(count));
            }
            Effect::Discard {
                who, count, bind, ..
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("count", |w| w.amount(count));
                if let Some(name) = bind {
                    w.at("as", |w| w.bind(name, Kind::Object));
                }
            }
            Effect::LookAtTop { who, count, bind } | Effect::ExileTop { who, count, bind } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("count", |w| w.amount(count));
                w.at("as", |w| w.bind(bind, Kind::Object));
            }
            Effect::Manifest {
                who,
                count,
                from,
                bind,
            }
            | Effect::Cloak {
                who,
                count,
                from,
                bind,
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("count", |w| w.amount(count));
                if let Some(library) = from {
                    w.at("library", |w| w.reference(library, Kind::Player));
                }
                if let Some(name) = bind {
                    w.at("as", |w| w.bind(name, Kind::Object));
                }
            }
            Effect::Exile {
                what,
                bind,
                duration,
            } => {
                w.at("what", |w| w.reference(what, Kind::Object));
                if let Some(name) = bind {
                    w.at("as", |w| w.bind(name, Kind::Object));
                }
                // The engine returns the cards when the source leaves the battlefield or an opponent becomes the monarch,
                // and no other duration.
                let known = |d: &Duration| {
                    matches!(
                        d,
                        Duration::WhileSourceOnBattlefield | Duration::UntilOpponentBecomesMonarch
                    )
                };
                if let Some(duration) = duration.filter(|d| !known(d)) {
                    w.uses("Exile", &format!("Duration.{}", tag_of(&duration)));
                }
            }
            Effect::Reveal { what }
            | Effect::ExileHidden { what }
            | Effect::Counter { what }
            | Effect::Tap { what }
            | Effect::Untap { what }
            | Effect::TapOrUntap { what }
            | Effect::PhaseOut { what }
            | Effect::Transform { what }
            | Effect::Flip { what }
            | Effect::Prepare { what }
            | Effect::Goad { what }
            | Effect::Explore { what }
            | Effect::Connive { what }
            | Effect::Destroy { what, .. } => w.at("what", |w| w.reference(what, Kind::Object)),
            Effect::Fight { what, with } => {
                w.at("what", |w| w.reference(what, Kind::Object));
                w.at("with", |w| w.reference(with, Kind::Object));
            }
            Effect::Encode { what, on } => {
                w.at("what", |w| w.reference(what, Kind::Object));
                w.at("on", |w| w.reference(on, Kind::Object));
            }
            Effect::CopyCard { who, what, bind } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("what", |w| w.reference(what, Kind::Object));
                w.at("as", |w| w.bind(bind, Kind::Object));
            }
            Effect::Incubate { who, count } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("count", |w| w.amount(count));
            }
            Effect::Amass { count, .. }
            | Effect::Monstrosity { count }
            | Effect::Renown { count }
            | Effect::Adapt { count }
            | Effect::Fabricate { count }
            | Effect::Bolster { count } => w.at("count", |w| w.amount(count)),
            Effect::Search {
                who,
                filter,
                count,
                bind,
                ..
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("filter", |w| w.selector(filter));
                w.at("count", |w| w.bounds(count));
                w.at("as", |w| w.bind(bind, Kind::Object));
            }
            Effect::ExileUntil {
                who,
                filter,
                bind,
                exiled_as,
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("filter", |w| w.selector(filter));
                w.at("as", |w| w.bind(bind, Kind::Object));
                w.at("exiled_as", |w| w.bind(exiled_as, Kind::Object));
            }
            Effect::RevealUntil {
                who,
                filter,
                count,
                bind,
                revealed_as,
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("filter", |w| w.selector(filter));
                w.at("count", |w| w.amount(count));
                w.at("as", |w| w.bind(bind, Kind::Object));
                w.at("revealed_as", |w| w.bind(revealed_as, Kind::Object));
            }
            Effect::CastFromZone {
                who,
                what,
                way,
                bind,
                otherwise,
                ..
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("what", |w| w.reference(what, Kind::Object));
                if let Some(way) = way
                    && !matches!(
                        way,
                        CastWay::Madness | CastWay::Miracle | CastWay::Suspend
                    )
                {
                    w.problem("only madness, miracle and suspend are cast for a way of their own");
                }
                w.effect_list("otherwise", otherwise, false);
                if let Some(name) = bind {
                    w.bind(name, Kind::Object);
                }
            }
            Effect::Choose {
                who,
                from,
                count,
                bind,
                rest_as,
            } => {
                // A choice among players ("an opponent", CR 800.4) binds players; any other binds objects.
                let kind = match w.kind_of(from) {
                    Kind::Player => Kind::Player,
                    _ => Kind::Object,
                };
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("from", |w| w.reference(from, kind));
                w.at("count", |w| w.bounds(count));
                w.at("as", |w| w.bind(bind, kind));
                if let Some(rest) = rest_as {
                    w.at("rest_as", |w| w.bind(rest, kind));
                }
            }
            Effect::ChooseAtRandom { from, count, bind } => {
                w.at("from", |w| w.reference(from, Kind::Object));
                w.at("count", |w| w.amount(count));
                w.at("as", |w| w.bind(bind, Kind::Object));
            }
            Effect::Attach { what, to } => {
                w.at("what", |w| w.reference(what, Kind::Object));
                w.at("to", |w| w.reference(to, Kind::Object));
            }
            Effect::Unattach { what } | Effect::JoinAttack { what } => {
                w.at("what", |w| w.reference(what, Kind::Object));
            }
            Effect::MyriadCopies { of, bind } => {
                w.at("of", |w| w.reference(of, Kind::Object));
                if let Some(name) = bind {
                    w.at("as", |w| w.bind(name, Kind::Object));
                }
            }
            Effect::Shuffle { who }
            | Effect::WinGame { who }
            | Effect::Proliferate { who }
            | Effect::BecomeMonarch { who }
            | Effect::TimeTravel { who }
            | Effect::Learn { who }
            | Effect::Exploit { who } => {
                w.at("who", |w| w.reference(who, Kind::Player));
            }
            Effect::ExtraTurn { who, count, .. } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("count", |w| w.amount(count));
            }
            Effect::CreateEmblem { who, abilities } => w.w6b_emblem(who, abilities),
            Effect::AddPhase { phases } => {
                if phases.is_empty() {
                    w.problem("an added phase list names no phase");
                }
            }
            Effect::Move { what, to } => {
                w.at("what", |w| w.reference(what, Kind::Object));
                w.at("to", |w| w.destination(to));
            }
            Effect::Sacrifice { who, filter, count } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("filter", |w| w.selector(filter));
                w.at("count", |w| w.amount(count));
            }
            Effect::DealDamage { source, to, amount } => {
                if let Some(source) = source {
                    w.at("source", |w| w.reference(source, Kind::Object));
                }
                w.at("to", |w| w.reference(to, Kind::Either));
                // The recipients of the damage each read their share of a division.
                w.amounting = Amounting::Shared;
                w.at("amount", |w| w.amount(amount));
            }
            Effect::GainLife { who, amount } | Effect::LoseLife { who, amount } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("amount", |w| w.amount(amount));
            }
            Effect::AddMana {
                who,
                mana,
                times,
                only,
                lasts,
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                if only.iter().any(|on| matches!(on, SpendOn::Spell(f) | SpendOn::Ability(f) if f.types.iter().any(|t| f.without.contains(t)))) {
                    w.problem("a restriction names a card type it also excludes");
                }
                if !only.is_empty() {
                    w.uses("Effect", "AddMana.Restricted");
                }
                if !lasts.is_step() {
                    w.uses("Effect", "AddMana.Lasts");
                }
                w.at("mana", |w| w.produce(mana));
                w.at("times", |w| w.amount(times));
            }
            Effect::PutCounters { on, count, .. } => {
                w.at("on", |w| w.reference(on, Kind::Either));
                w.amounting = Amounting::Shared;
                w.at("count", |w| w.amount(count));
            }
            Effect::RemoveCounters { from, count, .. } => {
                w.at("from", |w| w.reference(from, Kind::Either));
                w.at("count", |w| w.amount(count));
            }
            Effect::RepeatEach {
                over,
                bind,
                effects,
            } => w.repeat_each(over, bind, effects),
            Effect::RollDice {
                who,
                sides,
                dice,
                choose,
                plus,
                results,
            } => w.roll_dice(who, *sides, (*dice, *choose), plus, results),
            Effect::FlipCoin { who, win, lose, .. } => w.flip_coin(who, win, lose),
            Effect::Clash { who, win, lose, .. } => w.w9b_clash(who, win, lose),
            Effect::VillainousChoice {
                who, first, second, ..
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.effect_list("first", first, false);
                w.effect_list("second", second, false);
            }
            Effect::Vote {
                options,
                secret,
                bind,
            } => w.vote(options, *secret, bind.as_deref()),
            Effect::MultiplyCounters { on, factor, .. } => {
                w.at("on", |w| w.reference(on, Kind::Either));
                if *factor < 2 {
                    w.problem("a factor below two multiplies the counters by nothing");
                }
            }
            Effect::MoveCounters {
                from, to, count, ..
            } => {
                w.at("from", |w| w.reference(from, Kind::Object));
                w.at("to", |w| w.reference(to, Kind::Object));
                w.at("count", |w| w.amount(count));
            }
            Effect::CreateToken {
                who,
                token,
                count,
                bind,
                ..
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("token", |w| w.token(token));
                w.at("count", |w| w.amount(count));
                if let Some(name) = bind {
                    w.at("as", |w| w.bind(name, Kind::Object));
                }
                // A token that copies an object waits for the copy effects.
                if matches!(token, TokenRef::CopyOf { .. }) {
                    w.uses("Effect", "CreateToken.Copy");
                }
            }
            Effect::CopySpell {
                what,
                count,
                except,
                ..
            } => {
                w.at("what", |w| w.reference(what, Kind::Object));
                w.at("count", |w| w.amount(count));
                w.copy_exceptions(except);
            }
            Effect::CopyAbility { what, count, .. } => {
                w.at("what", |w| w.reference(what, Kind::Object));
                w.at("count", |w| w.amount(count));
            }
            Effect::BecomeCopy {
                what,
                of,
                except,
                duration,
            } => {
                w.at("what", |w| w.reference(what, Kind::Object));
                w.at("of", |w| w.reference(of, Kind::Object));
                w.copy_exceptions(except);
                // A copy that ends otherwise than with the turn or never is a capability of its own.
                if !matches!(duration, Duration::UntilEndOfTurn | Duration::Forever) {
                    w.uses("Copy", &format!("Duration.{}", tag_of(duration)));
                }
            }
            Effect::Continuous {
                affected,
                mods,
                duration,
            } => {
                w.at("affected", |w| w.reference(affected, Kind::Object));
                w.at("mods", |w| w.mods(mods));
                if *duration == Duration::WhileYouControlSource {
                    w.uses("Duration", "WhileYouControlSource");
                }
                if *duration == Duration::NextUntapStep && mods.as_slice() != [Modification::DoesntUntap] {
                    w.problem(C7_UNTAP_ONLY);
                }
            }
            Effect::ExchangeControl { what, duration } => {
                w.at("what", |w| {
                    for (at, one) in what.iter().enumerate() {
                        w.at(at.to_string(), |w| w.reference(one, Kind::Object));
                    }
                });
                if !matches!(what.len(), 1 | 2) {
                    w.problem(W6E_EXCHANGE_PAIR);
                }
                if *duration == Duration::WhileYouControlSource {
                    w.uses("Duration", "WhileYouControlSource");
                }
                if *duration == Duration::NextUntapStep {
                    w.problem(C7_UNTAP_ONLY);
                }
            }
            Effect::Rule { rule, duration } => w.at("rule", |w| {
                w.rule(rule);
                // Only the layers end an effect when the source changes controller.
                if *duration == Duration::WhileYouControlSource {
                    w.uses("Rule", "Duration.WhileYouControlSource");
                }
                if *duration == Duration::NextUntapStep {
                    w.problem(C7_UNTAP_ONLY);
                }
                if matches!(rule, GameRule::CanBeCommander) {
                    w.problem("a card can be a commander by a static ability, not by an effect");
                }
            }),
            Effect::Crewed { .. }
            | Effect::BecomeDayNight { .. }
            | Effect::Planechase { .. } => {}
            Effect::BecomeLevel { level } => {
                if *level < 2 {
                    w.problem("a Class starts at level 1 and gains level 2 or more");
                }
            }
            Effect::Regenerate { what } => w.at("what", |w| w.reference(what, Kind::Object)),
            Effect::Shield {
                when,
                by,
                duration,
            } => {
                if *duration == Duration::WhileYouControlSource {
                    w.uses("Shield", "Duration.WhileYouControlSource");
                }
                if *duration == Duration::NextUntapStep {
                    w.problem(C7_UNTAP_ONLY);
                }
                w.at("when", |w| w.replace_when(when));
                w.at("by", |w| w.replace_by(when, by));
                // The engine makes shields of damage prevention and of a death sent elsewhere; any other waits.
                match (when, by) {
                    (
                        ReplaceWhen::Damage { .. },
                        ReplaceBy::Prevent | ReplaceBy::PreventUpTo(_),
                    )
                    | (
                        ReplaceWhen::ZoneChange {
                            from: Some(ZoneKind::Battlefield | ZoneKind::Stack),
                            to: Some(ZoneKind::Graveyard),
                            ..
                        },
                        ReplaceBy::MoveTo(_),
                    ) => {}
                    _ => w.uses("Effect", &format!("Shield.{}", tag_of(when))),
                }
            }
            Effect::DelayedTrigger { when, effects, .. } => {
                w.at("when", |w| w.trigger_pattern(when));
                let saved = std::mem::replace(&mut w.scope.in_trigger, true);
                // The trigger it leaves behind remembers the X of the spell or ability that sets it up.
                w.effect_list("effects", effects, false);
                w.scope.in_trigger = saved;
            }
            Effect::If {
                condition,
                then,
                otherwise,
            } => {
                if w.after == After::Effect {
                    w.after = After::Condition;
                }
                w.at("condition", |w| w.condition(condition));
                if w.after == After::Condition {
                    w.after = After::Effect;
                }
                w.effect_list("then", then, false);
                w.effect_list("otherwise", otherwise, false);
            }
            Effect::May { who, effects } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.effect_list("effects", effects, false);
            }
            Effect::Pay {
                who,
                cost,
                times,
                paid,
                unpaid,
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("cost", |w| w.c6_eg_paid_cost(cost));
                w.at("times", |w| w.amount(times));
                w.payment_of_the_source(cost);
                w.c11f_paid_x(cost, paid);
                w.effect_list("unpaid", unpaid, false);
            }
            Effect::Modal {
                choose,
                modes,
                repeat,
                at_random,
                once_each_turn,
                once_ever,
                all_if,
            } => {
                w.modal(choose, modes, *repeat, only_effect);
                w.modal_flags(choose, *repeat, *at_random, (*once_each_turn, *once_ever));
                if let Some(condition) = all_if {
                    w.uses("Effect", "Modal.AllIf");
                    w.at("all_if", |w| w.condition(condition));
                    if *at_random || *repeat {
                        w.problem("every mode under a condition cannot be a pick at random or repeat");
                    }
                }
            }
        });
    }

    /// A pick at random chooses a fixed number of modes, none twice; each flag is a primitive of its own.
    fn modal_flags(&mut self, choose: &Bounds, repeat: bool, at_random: bool, once: (bool, bool)) {
        let (once_each_turn, once_ever) = once;
        if at_random {
            self.uses("Effect", "Modal.AtRandom");
            let fixed = choose.max.as_ref() == Some(&choose.min)
                && matches!(choose.min, Amount::Fixed(count) if count >= 1);
            if !fixed || repeat {
                self.problem("a pick at random chooses a fixed number of modes, none twice");
            }
        }
        if once_each_turn {
            self.uses("Effect", "Modal.OnceEachTurn");
            if repeat {
                self.problem("a mode chosen once each turn cannot repeat");
            }
        }
        if once_ever {
            self.uses("Effect", "Modal.OnceEver");
            if repeat || once_each_turn {
                self.problem("a mode chosen once ever cannot repeat or be chosen once each turn");
            }
        }
    }

    fn modal(&mut self, choose: &Bounds, modes: &[Mode], repeat: bool, only_effect: bool) {
        if !only_effect {
            self.problem("`Modal` must be the only effect of its ability");
        }
        if modes.len() < 2 {
            self.problem("a modal ability needs at least two modes");
        }
        self.at("choose", |w| w.bounds(choose));
        let limit = choose.max.as_ref().map(|m| match m {
            Amount::Fixed(n) => i64::from(*n),
            _ => i64::MAX,
        });
        if !repeat && limit.is_some_and(|n| n > i64::try_from(modes.len()).unwrap_or(i64::MAX)) {
            self.problem("chooses more modes than there are");
        }
        for (i, mode) in modes.iter().enumerate() {
            self.at(format!("modes[{i}]"), |w| {
                let in_trigger = w.scope.in_trigger;
                let saved = std::mem::replace(&mut w.scope, Scope::new(&mode.targets, in_trigger));
                w.body_checks(&mode.targets, &mode.effects, false);
                w.scope = saved;
            });
        }
    }

    /// Walks target specifications and effects in the current scope and checks every target is used.
    fn body_checks(&mut self, targets: &[TargetSpec], effects: &[Effect], top: bool) {
        for (i, spec) in targets.iter().enumerate() {
            self.at(format!("targets[{i}]"), |w| w.target_spec(spec));
        }
        self.effect_list("effects", effects, top);
        for index in 0..targets.len() {
            if !self.scope.used.contains(&index) {
                self.at(format!("targets[{index}]"), |w| {
                    w.problem(format!("target {index} is never used"));
                });
            }
        }
    }

    fn body(&mut self, targets: &[TargetSpec], effects: &[Effect], in_trigger: bool) {
        self.scope = Scope::new(targets, in_trigger);
        if effects.is_empty() {
            self.at("effects", |w| w.problem("needs at least one effect"));
        }
        if matches!(effects, [Effect::Modal { .. }]) && !targets.is_empty() {
            self.at("targets", |w| {
                w.problem("a modal ability keeps its targets in the modes, not beside them");
            });
        }
        self.body_checks(targets, effects, true);
    }

    /// How often, when and from where an ability works are rules of their own, so an engine that has not got
    /// them does not run the ability, whatever else it runs.
    fn where_and_how_often(&mut self, ability: &Ability) {
        if matches!(
            ability,
            Ability::Activated {
                timing: Timing::Sorcery,
                ..
            }
        ) {
            self.uses("Ability", "Sorcery");
        }
        if matches!(
            ability,
            Ability::Activated {
                condition: Some(_),
                ..
            }
        ) {
            self.uses("Ability", "Condition");
        }
        let (limit, from) = match ability {
            Ability::Activated { limit, from, .. } | Ability::Triggered { limit, from, .. } => {
                (limit.is_some(), !from.is_battlefield())
            }
            Ability::Static { from, .. } | Ability::Replacement { from, .. } => {
                (false, !from.is_battlefield())
            }
            // A spell that can't be countered says so from the stack, which every spell is on.
            Ability::Rule { from, .. } => (
                false,
                !matches!(from, ZoneKind::Battlefield | ZoneKind::Stack),
            ),
            _ => (false, false),
        };
        if limit {
            self.uses("Ability", "Limit");
        }
        if from {
            self.uses("Ability", "From");
            // A mana ability is activated at priority from the battlefield alone.
            if ability.is_mana_ability() {
                self.uses("Ability", "ManaAbility.From");
            }
        }
    }

    /// A static ability: who it affects, what it does, when, and from where.
    fn static_ability(
        &mut self,
        affected: &Ref,
        mods: &[Modification],
        condition: Option<&Condition>,
        defining: bool,
        from: ZoneKind,
    ) {
        self.scope = Scope::new(&[], false);
        // A keyword a player has is given to the player: "You have hexproof."
        let kind = if gives_player_keywords(affected, mods) {
            Kind::Player
        } else {
            Kind::Object
        };
        self.at("affected", |w| w.reference(affected, kind));
        // The layers change permanents, and the source of this one is not on the battlefield.
        if !from.is_battlefield() && !matches!(affected, Ref::Each(_)) {
            self.at("affected", |w| {
                w.problem(
                    "a static ability that works from another zone affects the objects a selector names",
                );
            });
        }
        self.at("mods", |w| w.mods(mods));
        if names_spells(affected) {
            self.spell_keywords(mods);
        }
        if mods
            .iter()
            .any(|m| matches!(m, Modification::AddProtectionFromChosenColor))
        {
            self.at("mods", |w| {
                w.problem("a colour is chosen as an effect begins, so only a `Continuous` effect adds protection from it");
            });
        }
        if let Some(condition) = condition {
            self.at("condition", |w| w.condition(condition));
        }
        if defining {
            self.defining(affected, mods, condition.is_some(), from);
        }
    }

    /// A static ability that names spells ("Artifact spells you cast have convoke") gives only the keywords that help
    /// pay for a spell, since the engine reads no other keyword a spell is given.
    fn spell_keywords(&mut self, mods: &[Modification]) {
        self.uses("Ability", "Static.SpellKeywords");
        let helps = |m: &Modification| {
            matches!(
                m,
                Modification::AddKeyword(Keyword::Convoke | Keyword::Improvise | Keyword::Delve)
            )
        };
        if !mods.iter().all(helps) {
            self.at("mods", |w| {
                w.problem("a spell is given only convoke, improvise or delve by a static ability");
            });
        }
    }

    /// An activated ability: its cost, the zone it is activated from, "activate only if" and what it does.
    fn activated_ability(
        &mut self,
        cost: &Cost,
        from: ZoneKind,
        condition: Option<&Condition>,
        targets: &[TargetSpec],
        effects: &[Effect],
    ) {
        let paid = std::mem::replace(&mut self.pays_objects, picks(cost));
        // The X a part of the cost counts is the one the ability announces.
        self.x_free = cost.has_x();
        self.x_now = self.x_free;
        self.at("cost", |w| w.cost(cost));
        self.at("from", |w| w.activated_zone(cost, from));
        if let Some(condition) = condition {
            // It is judged as the ability is activated, before any target is chosen.
            self.scope = Scope::new(&[], false);
            self.at("condition", |w| w.condition(condition));
        }
        self.x_free = cost.has_x();
        // An `{X}` of the mana and a part that counts X are asked as the ability is announced.
        if has_x_mana(cost) {
            self.uses("Cost", "Mana.X");
        }
        self.x_now = cost.has_x();
        self.body(targets, effects, false);
        self.pays_objects = paid;
    }

    /// A loyalty ability: a `-X` cost is asked as the ability is announced, and the effects read it.
    fn loyalty_ability(&mut self, cost: LoyaltyCost, targets: &[TargetSpec], effects: &[Effect]) {
        if cost == LoyaltyCost::MinusX {
            self.uses("Cost", "Loyalty.X");
            self.x_free = true;
            self.x_now = true;
        }
        self.body(targets, effects, false);
    }

    /// A cost modifier: fixed mana changes the cost as many times as `per` counts, over the spells `applies_to`
    /// selects; its own target counts read the targets of the spell ability beside it.
    fn cost_modifier(
        &mut self,
        applies_to: &Selector,
        mana: &crate::mana::ManaCostX,
        per: &Amount,
        condition: Option<&Condition>,
    ) {
        if mana.has_x() {
            self.problem(
                "a cost modifier changes the cost by fixed mana, repeated by `per`, not by an {X}",
            );
        }
        let specs = self.spell_targets.clone();
        self.scope = Scope::new(&specs, false);
        self.at("applies_to", |w| w.selector(applies_to));
        self.at("per", |w| w.amount(per));
        if let Some(condition) = condition {
            self.at("condition", |w| w.condition(condition));
        }
    }

    /// The engine activates a mana ability of the shape `mana_shape` reads at priority (the plain one is its own
    /// primitive, a Treasure's needs no more than its parts); one of any other shape (a mana cost, a limit, mixed mana)
    /// waits for its own task.
    fn mana_ability(&mut self, ability: &Ability) {
        if ability.is_mana_ability() && plain_mana_ability(ability).is_none() {
            match mana_shape(ability) {
                Some(shape) if shape.is_simple() => {}
                Some(_) => self.uses("Ability", "ManaAbility.Complex"),
                None => self.uses("Ability", "ManaAbility.Other"),
            }
        }
    }

    fn ability(&mut self, ability: &Ability) {
        let tag = tag_of(ability);
        if !matches!(ability, Ability::Keyword(_)) {
            self.uses("Ability", &tag);
        }
        self.mana_ability(ability);
        self.where_and_how_often(ability);
        let saved_x = self.x_free;
        let saved_now = std::mem::replace(&mut self.x_now, false);
        self.at(tag, |w| match ability {
            Ability::Spell { targets, effects } => {
                // The spell has the X of its own cost; one that another ability grants or a token carries has none.
                w.x_now = w.nested == 0;
                w.body(targets, effects, false);
            }
            Ability::Loyalty {
                cost,
                targets,
                effects,
            } => w.loyalty_ability(*cost, targets, effects),
            Ability::Activated {
                cost,
                targets,
                effects,
                from,
                condition,
                ..
            } => w.activated_ability(cost, *from, condition.as_ref(), targets, effects),
            Ability::Triggered {
                when,
                intervening_if,
                targets,
                effects,
                ..
            } => {
                w.scope = Scope::new(&[], true);
                w.at("when", |w| w.trigger_when(when));
                // The permanent remembers the X of the spell it came from for its own enters trigger (CR 107.3m); a
                // trigger on a spell being cast reads the X that spell was cast with, which another card's trigger
                // finds in a spell whose cost this card does not print.
                w.x_now = enters_itself(when) || casts_a_spell(when) || w6c_reads_cost_x(when);
                // "When you cycle this card", "when this creature becomes monstrous": the X of that cost, when the face prints one.
                w.x_free |= casts_another_spell(when) || (w6c_reads_cost_x(when) && w.w6c_cost_x);
                if let Some(condition) = intervening_if {
                    w.at("if", |w| w.condition(condition));
                }
                w.body(
                    targets,
                    effects,
                    !matches!(when, TriggerPattern::State { .. }),
                );
            }
            Ability::Static {
                affected,
                mods,
                condition,
                defining,
                from,
            } => w.static_ability(affected, mods, condition.as_ref(), *defining, *from),
            Ability::Rule {
                rule,
                condition,
                from,
            } => w.rule_ability(rule, condition.as_ref(), *from),
            Ability::Replacement {
                when,
                by,
                condition,
                ..
            } => {
                w.scope = Scope::new(&[], false);
                w.at("when", |w| w.replace_when(when));
                // A permanent entering as its spell resolves has the X the spell was cast with (CR 107.3m).
                w.x_now =
                    matches!(when, ReplaceWhen::Enters { what } if what.0.contains(&Pred::This));
                w.at("by", |w| w.replace_by(when, by));
                if let Some(condition) = condition {
                    w.at("condition", |w| w.condition(condition));
                }
            }
            Ability::CostModifier {
                applies_to,
                mana,
                per,
                condition,
                ..
            } => w.cost_modifier(applies_to, mana, per, condition.as_ref()),
            Ability::AdditionalCost { cost, required } => w.additional_cost(cost, *required),
            Ability::FreeCast { condition } => {
                w.scope = Scope::new(&[], false);
                if let Some(condition) = condition {
                    w.at("condition", |w| w.condition(condition));
                }
            }
            Ability::Keyword(keyword) => w.keyword_ability(keyword),
            Ability::Manual { reason, span } => {
                if reason.trim().is_empty() {
                    w.problem("a clause left to the table needs a reason");
                }
                if span.start >= span.end {
                    w.problem("the span is empty or backwards");
                }
            }
        });
        self.x_free = saved_x;
        self.x_now = saved_now;
    }

    /// A rule of the game names what it is about: players or objects, and for the one that triggers again the event that
    /// causes the trigger, which is judged without a target, a triggering event or a name bound earlier.
    fn rule(&mut self, rule: &GameRule) {
        let tag = tag_of(rule);
        self.uses("Rule", &tag);
        match rule {
            GameRule::DamageCantBePrevented { source, .. } => {
                self.at(tag, |w| w.at("source", |w| w.selector(source)));
            }
            GameRule::MustAttack(who)
            | GameRule::MustBlock(who)
            | GameRule::CantBeCountered(who) => self.at(tag, |w| w.reference(who, Kind::Object)),
            GameRule::MustAttackPlayers { who, players } => self.at(tag, |w| {
                w.at("who", |w| w.reference(who, Kind::Object));
                w.at("players", |w| w.reference(players, Kind::Player));
            }),
            GameRule::AttackTax {
                who,
                attackers,
                mana,
                life,
                per,
                ..
            } => self.at(tag, |w| {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("attackers", |w| w.selector(attackers));
                w.at("per", |w| w.amount(per));
                if mana.rest().symbols().is_empty() && !mana.has_x() && *life == 0 {
                    w.problem("an attack tax costs mana, life or both");
                }
                if mana.has_x() {
                    w.at("mana", |w| {
                        w.problem("an attack tax is fixed mana, repeated by `per`, not an {X}");
                    });
                }
            }),
            GameRule::AttackLimit(most) | GameRule::BlockLimit(most) => {
                if *most == 0 {
                    self.at(tag, |w| w.problem("a limit on a declaration lets at least one creature in"));
                }
            }
            GameRule::CantGainLife(who)
            | GameRule::NoLifeLossFromDamage(who)
            | GameRule::RadiationGainsLife(who)
            | GameRule::LifeTotalCantChange(who)
            | GameRule::CantDraw(who)
            | GameRule::CantSearch(who)
            | GameRule::SkipExtraTurns(who)
            | GameRule::RevealTop(who)
            | GameRule::LookAtTopAnyTime(who)
            | GameRule::SkipDrawStep(who)
            | GameRule::NoMaxHandSize(who) => self.at(tag, |w| w.reference(who, Kind::Player)),
            GameRule::MayCast(permission) => self.at(tag, |w| {
                w.at("who", |w| w.reference(&permission.who, Kind::Player));
                w.at("what", |w| w.selector(&permission.what));
                if let Some(only) = &permission.only {
                    w.at("only", |w| w.reference(only, Kind::Object));
                }
                if matches!(
                    permission.from,
                    Some(ZoneKind::Battlefield | ZoneKind::Stack)
                ) {
                    w.at("from", |w| {
                        w.problem("a card is cast from a hand, a library, a graveyard, exile or the command zone");
                    });
                }
            }),
            GameRule::AdditionalLandPlays { who, count } => self.at(tag, |w| {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("count", |w| w.amount(count));
                if matches!(count, Amount::Fixed(n) if *n < 1) {
                    w.problem("an additional land play is at least one");
                }
            }),
            GameRule::PlayersHave { who, .. } => self.at(tag, |w| w.reference(who, Kind::Player)),
            GameRule::CanBeCommander => {}
            GameRule::TriggerAgain { cause } => {
                let saved = std::mem::replace(&mut self.scope, Scope::new(&[], false));
                self.at(tag, |w| w.at("cause", |w| w.trigger_pattern(cause)));
                self.scope = saved;
            }
        }
    }

    /// A static ability that changes a rule: the rule, the condition it works under and where its source must be.
    fn rule_ability(&mut self, rule: &GameRule, condition: Option<&Condition>, from: ZoneKind) {
        self.scope = Scope::new(&[], false);
        self.at("rule", |w| w.rule(rule));
        if let Some(condition) = condition {
            self.at("condition", |w| w.condition(condition));
        }
        self.rule_zone(rule, from);
    }

    /// A rule works from the battlefield, "this spell can't be countered" from the stack (CR 113.6b), and a card's
    /// permission to cast itself from its graveyard from there (CR 113.6).
    fn rule_zone(&mut self, rule: &GameRule, from: ZoneKind) {
        let spell = matches!(rule, GameRule::CantBeCountered(Ref::This));
        let c8_eg_itself = matches!(
            rule,
            GameRule::MayCast(permission)
                if permission.what.0 == [Pred::This] && permission.from == Some(ZoneKind::Graveyard)
        );
        match from {
            ZoneKind::Battlefield => {}
            ZoneKind::Stack if spell => {}
            ZoneKind::Graveyard if c8_eg_itself => {}
            _ if matches!(rule, GameRule::CanBeCommander) => self.at("from", |w| {
                w.problem("a card can be a commander in every zone: write the rule without `from`");
            }),
            _ => self.at("from", |w| {
                w.problem(
                    "a rule works from the battlefield, or from the stack when it says this spell can't be countered",
                );
            }),
        }
    }

    fn replace_when(&mut self, when: &ReplaceWhen) {
        self.uses("ReplaceWhen", &tag_of(when));
        if matches!(when, ReplaceWhen::Draw { except_draw_step, .. } if *except_draw_step > 0) {
            self.uses("ReplaceWhen", "Draw.ExceptDrawStep");
        }
        match when {
            ReplaceWhen::TokenCreation { controller } => {
                self.at("controller", |w| w.reference(controller, Kind::Player));
            }
            ReplaceWhen::Damage { to, source, .. } => {
                if let Some(to) = to {
                    self.at("to", |w| w.reference(to, Kind::Either));
                }
                self.at("source", |w| w.selector(source));
            }
            ReplaceWhen::ZoneChange { what, .. }
            | ReplaceWhen::Destroyed { what }
            | ReplaceWhen::Discarded { what }
            | ReplaceWhen::Enters { what } => {
                self.at("what", |w| w.selector(what));
            }
            ReplaceWhen::CountersPut { on, .. } => {
                if let Some(on) = on {
                    self.at("on", |w| w.reference(on, Kind::Either));
                }
            }
            ReplaceWhen::LifeGain { who } | ReplaceWhen::Draw { who, .. } => {
                self.at("who", |w| w.reference(who, Kind::Player));
            }
        }
    }

    /// What happens instead reads the event it replaces as a trigger reads its event: `Triggering` is the player or object
    /// the event is about, `Amount::Triggering` its amount and `Dealer` the source of damage. A step that asks a question
    /// is named apart (`ReplaceBy.Instead.Asks`): a replacement asks nothing in the middle of an event.
    fn w5d_instead(&mut self, when: &ReplaceWhen, effects: &[Effect]) {
        if effects.is_empty() {
            self.problem("needs at least one effect");
        }
        if effects.iter().any(w5d_asks) {
            self.uses("ReplaceBy", "Instead.Asks");
        }
        let saved = (self.scope.in_trigger, self.scope.dealer);
        self.scope.in_trigger = true;
        self.scope.dealer = matches!(when, ReplaceWhen::Damage { .. });
        self.effect_list("Instead", effects, false);
        (self.scope.in_trigger, self.scope.dealer) = saved;
    }

    fn replace_by(&mut self, when: &ReplaceWhen, by: &ReplaceBy) {
        self.uses("ReplaceBy", &tag_of(by));
        let enters = matches!(when, ReplaceWhen::Enters { .. });
        match by {
            ReplaceBy::Multiply(0) => self.problem("multiplying by zero is `Prevent`"),
            ReplaceBy::Add(0) => self.problem("adding nothing changes nothing"),
            ReplaceBy::Multiply(_) | ReplaceBy::Add(_)
                if matches!(
                    when,
                    ReplaceWhen::ZoneChange { .. }
                        | ReplaceWhen::Destroyed { .. }
                        | ReplaceWhen::Discarded { .. }
                        | ReplaceWhen::Enters { .. }
                ) =>
            {
                self.problem("a zone change has no number to change");
            }
            ReplaceBy::PreventUpTo(0) => self.problem("preventing nothing changes nothing"),
            ReplaceBy::Prevent
                if matches!(
                    when,
                    ReplaceWhen::ZoneChange { .. }
                        | ReplaceWhen::Destroyed { .. }
                        | ReplaceWhen::Discarded { .. }
                        | ReplaceWhen::Enters { .. }
                ) =>
            {
                self.problem("a zone change or an entering is not prevented");
            }
            ReplaceBy::PreventUpTo(_) if !matches!(when, ReplaceWhen::Damage { .. }) => {
                self.problem("only damage is prevented in part");
            }
            ReplaceBy::PreventRemovingCounter(_) if !matches!(when, ReplaceWhen::Damage { .. }) => {
                self.problem("only damage is prevented with a counter removed");
            }
            ReplaceBy::Instead(effects) => self.w5d_instead(when, effects),
            ReplaceBy::Dredge(cards) => {
                if !matches!(when, ReplaceWhen::Draw { .. }) {
                    self.problem("`Dredge` replaces a `Draw`");
                }
                if *cards == 0 {
                    self.problem("dredging no cards is drawing");
                }
            }
            ReplaceBy::MoveTo(_)
                if !matches!(
                    when,
                    ReplaceWhen::ZoneChange { .. } | ReplaceWhen::Discarded { .. }
                ) =>
            {
                self.problem("`MoveTo` replaces a `ZoneChange` or a `Discarded`");
            }
            ReplaceBy::WithCounters { count, .. } => {
                if !enters {
                    self.problem("`WithCounters` replaces an `Enters`");
                }
                self.at("count", |w| w.amount(count));
            }
            ReplaceBy::Tapped
            | ReplaceBy::Choose(_)
            | ReplaceBy::PayLifeOrTapped(_)
            | ReplaceBy::RevealOrTapped(_)
            | ReplaceBy::Riot
            | ReplaceBy::DayIfNeither
            | ReplaceBy::EntersPrepared
                if !enters =>
            {
                self.problem("only an `Enters` event is entered tapped, chosen on or paid for");
            }
            ReplaceBy::Devour { counters, of } => {
                if !enters {
                    self.problem("only an `Enters` event devours");
                }
                if *counters == 0 {
                    self.problem("devouring for no counters does nothing");
                }
                self.at("of", |w| w.selector(of));
            }
            ReplaceBy::PayLifeOrTapped(0) => self.problem("paying no life is entering untapped"),
            ReplaceBy::RevealOrTapped(of) => self.at("of", |w| w.selector(of)),
            ReplaceBy::UmbraArmor if !matches!(when, ReplaceWhen::Destroyed { .. }) => {
                self.problem("`UmbraArmor` replaces a `Destroyed`");
            }
            ReplaceBy::AsCopy { of, except, .. } => {
                if !enters {
                    self.problem("only an `Enters` event is entered as a copy");
                }
                self.at("of", |w| w.selector(of));
                self.copy_exceptions(except);
            }
            _ => {}
        }
    }

    fn face(&mut self, index: usize, face: &Face) {
        self.face = index;
        self.w6c_cost_x = w6c_cost_x(face);
        self.spell_targets = face
            .abilities
            .iter()
            .find_map(|a| matches!(a, Ability::Spell { .. }).then(|| a.targets().to_vec()))
            .unwrap_or_default();
        for (i, ability) in face.abilities.iter().enumerate() {
            self.at(format!("abilities[{i}]"), |w| w.ability(ability));
        }
    }
}

/// Whether the trigger is on an action of the card itself whose cost may hold an `{X}`: "when you cycle this card",
/// "when this creature becomes monstrous". It reads the X of that cost.
fn w6c_reads_cost_x(when: &TriggerPattern) -> bool {
    matches!(
        when,
        TriggerPattern::Cycled { what, .. } | TriggerPattern::BecomesMonstrous { what }
            if what.0.contains(&Pred::This)
    )
}

/// Whether the face has an `{X}` in a cycling cost or in the cost of an activated ability that makes it monstrous.
fn w6c_cost_x(face: &Face) -> bool {
    face.abilities.iter().any(|ability| match ability {
        Ability::Keyword(Keyword::Cycling(cost) | Keyword::Typecycling { cost, .. }) => {
            has_x_mana(cost)
        }
        Ability::Activated { cost, effects, .. } => {
            has_x_mana(cost)
                && effects
                    .iter()
                    .any(|effect| matches!(effect, Effect::Monstrosity { .. }))
        }
        _ => false,
    })
}

/// Whether the trigger is "when this enters": the one trigger of a permanent that reads the X its spell was cast with.
fn enters_itself(when: &TriggerPattern) -> bool {
    matches!(
        when,
        TriggerPattern::ZoneChange {
            what,
            to: Some(ZoneKind::Battlefield),
            ..
        } if what.0.contains(&Pred::This)
    )
}

/// Whether the trigger is on a spell being cast: it reads the X of that spell.
const fn casts_a_spell(when: &TriggerPattern) -> bool {
    matches!(when, TriggerPattern::SpellCast { .. })
}

/// Whether the trigger is on a spell other than the card itself being cast, whose X the card does not print.
fn casts_another_spell(when: &TriggerPattern) -> bool {
    matches!(when, TriggerPattern::SpellCast { what, .. } if !what.0.contains(&Pred::This))
}

/// Whether a static ability affects spells being cast: its selector holds the spell predicate.
fn names_spells(affected: &Ref) -> bool {
    matches!(affected, Ref::Each(selector) if selector.0.contains(&Pred::Spell))
}

/// Whether a static ability gives keywords a player can have (hexproof, shroud, protection) to players, and nothing else.
fn gives_player_keywords(affected: &Ref, mods: &[Modification]) -> bool {
    matches!(
        affected,
        Ref::You | Ref::Opponents | Ref::EachPlayer | Ref::AttachedPlayer
    ) && !mods.is_empty()
        && mods.iter().all(Modification::is_player_keyword)
}

/// Whether a cost has a part that picks objects the effects can name.
fn picks(cost: &Cost) -> bool {
    cost.0.iter().any(|item| match item {
        CostItem::OneOf(costs) => costs.iter().any(picks),
        other => matches!(
            other,
            CostItem::Sacrifice { .. }
                | CostItem::Discard { .. }
                | CostItem::ExileFromGraveyard { .. }
                | CostItem::ExileFromHand { .. }
                | CostItem::RevealFromHand { .. }
                | CostItem::ReturnToHand { .. }
                | CostItem::TapPermanents { .. }
                | CostItem::TapPower { .. }
        ),
    })
}

/// Whether a cost has an `{X}` in its mana.
fn has_x_mana(cost: &Cost) -> bool {
    cost.0
        .iter()
        .any(|item| matches!(item, CostItem::Mana(mana) if mana.has_x()))
}

/// Every primitive an ability uses, named `Effect.DealDamage`, `Keyword.Flying`, `Trigger.ZoneChange`,
/// `Cost.Tap`, `Mod.ModifyPT`, `ReplaceWhen.TokenCreation`, `ReplaceBy.Multiply` or `Ability.Loyalty`;
/// an ability with a `limit`, a zone other than the battlefield or sorcery timing also uses `Ability.Limit`,
/// `Ability.From` or `Ability.Sorcery`.
/// A value the engine cannot read yet is named too, wherever it stands (a condition, an amount, a selector):
/// `Amount.Kicks` and `Condition.Kicked`, and a divided amount read anywhere but as the amount of damage or counters
/// (`Amount.Divided.Elsewhere`, `Amount.EvenShare.Elsewhere`). A loyalty ability whose cost is `-X` names `Cost.Loyalty.X`.
///
/// A plain `{T}: Add {X}` is one primitive, [`PLAIN_MANA_ABILITY`]: the engine runs that exact shape
/// and not the general ability, cost and effect it is made of.
pub fn used_primitives(ability: &Ability) -> BTreeSet<String> {
    if plain_mana_ability(ability).is_some() {
        return BTreeSet::from([PLAIN_MANA_ABILITY.to_owned()]);
    }
    let tokens = BTreeSet::new();
    let mut walker = Walker::new(&tokens);
    walker.ability(ability);
    walker.uses
}

fn check_fingerprint(walker: &mut Walker<'_>, face: &Face, status: ImplStatus) {
    let fingerprint = &face.oracle_fingerprint;
    let well_formed = fingerprint.len() == 32
        && fingerprint
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f'));
    if fingerprint.is_empty() {
        if matches!(status, ImplStatus::Full | ImplStatus::Partial) {
            walker.problem("rules need the fingerprint of the oracle text they were written for");
        }
    } else if !well_formed {
        walker.problem("the fingerprint is 32 lowercase hex characters");
    }
}

fn check_status(walker: &mut Walker<'_>, card: &Card) {
    let manual = card.abilities().filter(|(_, a)| a.is_manual()).count();
    let modelled = card.abilities().count() - manual;
    let ok = match card.impl_status {
        ImplStatus::Full => manual == 0,
        ImplStatus::Partial => manual > 0 && modelled > 0,
        ImplStatus::Manual | ImplStatus::Unplayable => manual + modelled == 0,
    };
    if !ok {
        let status = tag_of(&card.impl_status);
        walker.at("impl_status", |w| {
            w.problem(match card.impl_status {
                ImplStatus::Full => format!("{status} cards have no clauses left to the table ({manual} found)"),
                ImplStatus::Partial => format!("Partial cards have at least one clause left to the table and one modelled ability ({manual} and {modelled} found)"),
                _ => format!("{status} cards have no abilities ({} found)", manual + modelled),
            });
        });
    }
}

fn check_spans(walker: &mut Walker<'_>, face: &Face) {
    let spans: Vec<_> = face
        .abilities
        .iter()
        .enumerate()
        .filter_map(|(i, a)| match a {
            Ability::Manual { span, .. } => Some((i, *span)),
            _ => None,
        })
        .collect();
    for (n, (i, span)) in spans.iter().enumerate() {
        if spans[..n].iter().any(|(_, earlier)| earlier.overlaps(span)) {
            walker.at(format!("abilities[{i}]"), |w| {
                w.at("Manual", |w| w.problem("overlaps an earlier clause"));
            });
        }
    }
}

/// Walks a whole card, collecting its problems, the uses of X and the card names it mentions.
fn walk<'a>(card: &Card, tokens: &'a BTreeSet<String>) -> Walker<'a> {
    let mut walker = Walker::new(tokens);
    walker.w7e_has_devour = card
        .abilities()
        .any(|(_, a)| matches!(a, Ability::Keyword(Keyword::Devour { .. })));
    walker.has_kicker = card
        .abilities()
        .any(|(_, a)| matches!(a, Ability::Keyword(k) if k.is_kicker()));
    walker.kickers = card
        .abilities()
        .filter_map(|(_, a)| match a {
            Ability::Keyword(Keyword::Kicker(cost)) => Some(cost.clone()),
            _ => None,
        })
        .collect();
    walker.has_additional_cost = card.abilities().any(|(_, a)| match a {
        Ability::AdditionalCost { required, .. } => !required,
        Ability::Keyword(keyword) => keyword.has_additional_cost(),
        _ => false,
    });
    walker.counts_x = card.abilities().any(
        |(_, a)| matches!(a, Ability::AdditionalCost { cost, required: true } if cost.has_x()),
    );
    walker.pays_objects = card
        .abilities()
        .any(|(_, a)| matches!(a, Ability::AdditionalCost { cost, required: true } if picks(cost)));
    if !is_uuid(&card.oracle_id) {
        walker.at("oracle_id", |w| w.problem("not a lowercase UUID"));
    }
    if card.faces.is_empty() || card.faces.len() > 4 {
        walker.at("faces", |w| w.problem("a card has one to four faces"));
    }
    let joined = card
        .faces
        .iter()
        .map(|f| f.name.as_str())
        .collect::<Vec<_>>()
        .join(" // ");
    if card.name.is_empty() || card.name != joined {
        walker.at("name", |w| {
            w.problem(format!(
                "the name is the face names joined by \" // \": expected `{joined}`"
            ));
        });
    }
    let mut seen = BTreeSet::new();
    for (index, face) in card.faces.iter().enumerate() {
        walker.at(format!("faces[{index}]"), |w| {
            if face.name.is_empty() || !seen.insert(face.name.clone()) {
                w.at("name", |w| w.problem("face names are non-empty and unique"));
            }
            check_fingerprint(w, face, card.impl_status);
            check_spans(w, face);
            w.face(index, face);
        });
    }
    check_status(&mut walker, card);
    walker
}

/// The problems of a card file by itself, with `tokens` the ids of the predefined tokens.
pub(crate) fn check_card(card: &Card, tokens: &BTreeSet<String>) -> Vec<Problem> {
    walk(card, tokens).problems
}

/// The problems of a dungeon room's ability, with `tokens` the ids of the predefined tokens.
pub(crate) fn w11d_check_room(ability: &Ability, tokens: &BTreeSet<String>) -> Vec<Problem> {
    let mut walker = Walker::new(tokens);
    walker.ability(ability);
    walker.problems
}

/// The problems of a predefined token by itself.
pub(crate) fn check_token(id: &str, spec: &TokenSpec, tokens: &BTreeSet<String>) -> Vec<Problem> {
    let mut walker = Walker::new(tokens);
    if !is_name(id) {
        walker.at("id", |w| {
            w.problem("a token id is lowercase letters, digits and underscores");
        });
    }
    walker.at("token", |w| w.token_spec(spec, false));
    walker.problems
}

fn type_words(type_line: &str) -> (Vec<&str>, Vec<&str>) {
    let (types, subtypes) = type_line.split_once('\u{2014}').unwrap_or((type_line, ""));
    (
        types.split_whitespace().collect(),
        subtypes.split_whitespace().collect(),
    )
}

/// The problems of a card file against the imported card: names, fingerprints, keywords and the
/// printed characteristics its abilities rely on.
pub(crate) fn check_printed(
    card: &Card,
    printed: &PrintedCard,
    known_names: &BTreeSet<String>,
    tokens: &BTreeSet<String>,
) -> Vec<Problem> {
    let checked = walk(card, tokens);
    let (x_uses, names, granted) = (checked.x_uses, checked.names, checked.granted);
    let mut walker = Walker::new(tokens);
    if card.oracle_id != printed.oracle_id {
        walker.at("oracle_id", |w| {
            w.problem(format!(
                "the imported card has oracle id {}",
                printed.oracle_id
            ));
        });
    }
    if card.name != printed.name {
        walker.at("name", |w| {
            w.problem(format!("the imported name is `{}`", printed.name));
        });
    }
    if card.faces.len() != printed.faces.len() {
        walker.at("faces", |w| {
            w.problem(format!(
                "the imported card has {} face(s), the file {}",
                printed.faces.len(),
                card.faces.len()
            ));
        });
        return walker.problems;
    }
    let live = matches!(card.impl_status, ImplStatus::Full | ImplStatus::Partial);
    for (index, (face, shown)) in card.faces.iter().zip(&printed.faces).enumerate() {
        walker.at(format!("faces[{index}]"), |w| {
            check_face_printed(w, card.impl_status, face, shown, live);
        });
    }
    if live {
        for keyword in &printed.keywords {
            if Keyword::is_known_name(keyword) && !has_keyword(card, printed, keyword, &granted) {
                walker.at("faces", |w| {
                    w.problem(format!(
                        "the imported card has the keyword `{keyword}` but no face has it"
                    ));
                });
            }
        }
    }
    for (face, path) in x_uses {
        if !printed.faces[face].mana_cost.contains("{X}") {
            walker.path = vec![path];
            walker.problem("uses X but the imported mana cost has none");
        }
    }
    for (_, path, name) in names {
        if !known_names.contains(&name.to_lowercase()) {
            walker.path = vec![path];
            walker.problem(format!("no imported card is named `{name}`"));
        }
    }
    walker.problems
}

/// Whether some face has the keyword as an ability, an ability grants it (`granted`: the
/// `AddKeyword`s and the keyword abilities of tokens and of granted abilities, which Scryfall lists
/// for "Enchanted creature has flying" and "a token with flying" as for a keyword line), or a
/// `Manual` clause names it in that face's printed text (a typecycling or landwalk form).
fn has_keyword(card: &Card, printed: &PrintedCard, keyword: &str, granted: &[Keyword]) -> bool {
    granted.iter().any(|k| lists(k, keyword))
        || card.abilities().any(|(face, ability)| match ability {
            Ability::Keyword(k) => lists(k, keyword),
            Ability::Manual { span, .. } => printed
                .faces
                .get(face)
                .and_then(|shown| span.slice(&shown.oracle_text))
                .is_some_and(|clause| {
                    names_keyword(clause, keyword)
                        || (keyword.eq_ignore_ascii_case("Typecycling")
                            && names_typecycling(clause))
                        || (keyword.eq_ignore_ascii_case("Landwalk") && names_landwalk(clause))
                }),
            _ => false,
        })
}

/// Whether Scryfall lists `keyword` (any case) for a card that has the keyword `k`.
fn lists(k: &Keyword, keyword: &str) -> bool {
    k.name().eq_ignore_ascii_case(keyword)
        // Scryfall lists `Partner` as well for a "Partner with" card and a "Partner—text" card (CR 702.124).
        || (matches!(k, Keyword::PartnerWith(_) | Keyword::PartnerGroup(_))
            && Keyword::Partner.name().eq_ignore_ascii_case(keyword))
        // Scryfall lists `Enchant` for an Aura that enchants a player as it does for any other.
        || (matches!(k, Keyword::EnchantPlayer { .. }) && "Enchant".eq_ignore_ascii_case(keyword))
        // Scryfall lists `Equip` for an Equipment that equips only some creatures as for any other.
        || (matches!(k, Keyword::EquipTo { .. })
            && Keyword::Equip(Cost::default()).name().eq_ignore_ascii_case(keyword))
        // Scryfall lists `Cycling` as well for a typecycling card (CR 702.29).
        || (matches!(k, Keyword::Typecycling { .. })
            && Keyword::Cycling(Cost::default()).name().eq_ignore_ascii_case(keyword))
}

/// Whether the clause holds the keyword (any case) at the end of a word: no letter or digit
/// follows it inside the clause. No word has to start there, so `Plainscycling` names `Cycling`
/// but `Flashback` does not name `Flash`.
fn names_keyword(clause: &str, keyword: &str) -> bool {
    let (clause, keyword) = (clause.to_ascii_lowercase(), keyword.to_ascii_lowercase());
    clause.match_indices(&keyword).any(|(at, found)| {
        let after = clause
            .get(at + found.len()..)
            .and_then(|rest| rest.chars().next());
        !after.is_some_and(char::is_alphanumeric)
    })
}

/// Whether the clause has a word that is a typecycling: some letters before `cycling` (`Plainscycling`).
fn names_typecycling(clause: &str) -> bool {
    clause.split(|c: char| !c.is_alphabetic()).any(|word| {
        let word = word.to_ascii_lowercase();
        word.len() > "cycling".len() && word.ends_with("cycling")
    })
}

/// Whether the clause has a word that is a landwalk: some letters before `walk` (`Mountainwalk`), which
/// Scryfall lists as `Landwalk` (CR 702.14).
fn names_landwalk(clause: &str) -> bool {
    clause.split(|c: char| !c.is_alphabetic()).any(|word| {
        let word = word.to_ascii_lowercase();
        word.len() > "walk".len() && word.ends_with("walk")
    })
}

fn check_face_printed(
    walker: &mut Walker<'_>,
    status: ImplStatus,
    face: &Face,
    shown: &PrintedFace,
    live: bool,
) {
    if face.name != shown.name {
        walker.at("name", |w| {
            w.problem(format!("the imported face is named `{}`", shown.name));
        });
    }
    let now = oracle_fingerprint(&shown.oracle_text);
    if live && face.oracle_fingerprint != now {
        walker.at("oracle_fingerprint", |w| {
            w.problem(format!(
                "the oracle text changed since the rules were written (file {}, imported {now})",
                face.oracle_fingerprint
            ));
        });
    }
    let (types, subtypes) = type_words(&shown.type_line);
    let is_spell = types.iter().any(|t| matches!(*t, "Instant" | "Sorcery"));
    let has_spell = face
        .abilities
        .iter()
        .any(|a| matches!(a, Ability::Spell { .. }));
    if has_spell && !is_spell {
        walker.at("abilities", |w| {
            w.problem("a Spell ability belongs to an instant or sorcery");
        });
    }
    if is_spell && !has_spell && status == ImplStatus::Full {
        walker.at("abilities", |w| {
            w.problem("an instant or sorcery needs a Spell ability");
        });
    }
    let planeswalker = types.contains(&"Planeswalker");
    let length = shown.oracle_text.chars().count();
    for (i, ability) in face.abilities.iter().enumerate() {
        walker.at(format!("abilities[{i}]"), |w| match ability {
            Ability::Loyalty { .. } if !planeswalker => {
                w.problem("a Loyalty ability belongs to a planeswalker");
            }
            Ability::Keyword(Keyword::Enchant(_) | Keyword::EnchantPlayer { .. })
                if !subtypes.contains(&"Aura") =>
            {
                w.problem("Enchant belongs to an Aura");
            }
            Ability::Manual { span, .. }
                if usize::try_from(span.end).map_or(true, |end| end > length) =>
            {
                w.problem(format!(
                    "the span ends past the oracle text ({length} characters)"
                ));
            }
            _ => {}
        });
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every rule of the structural checks is exercised by a small bad card, and a good one passes with the primitives it uses named.
    use super::*;
    use rstest::rstest;

    fn card(abilities: &str) -> Card {
        let json = format!(
            r#"{{"oracle_id":"4457ed35-7c10-48c8-9776-456485fdf070","name":"Test","impl_status":"Full","faces":[{{"name":"Test","oracle_fingerprint":"e3b0c44298fc1c149afbf4c8996fb924","abilities":[{abilities}]}}]}}"#
        );
        serde_json::from_str(&json).unwrap()
    }

    fn problems(abilities: &str) -> Vec<String> {
        let tokens: BTreeSet<String> = ["treasure".to_owned()].into();
        check_card(&card(abilities), &tokens)
            .into_iter()
            .map(|p| format!("{}: {}", p.path, p.message))
            .collect()
    }

    #[rstest]
    #[case::a_choice_of_two(r#""dice":2,"choose":true,"results":[]"#, 0)]
    #[case::one_die_with_no_table(r#""results":[]"#, 0)]
    #[case::two_dice_without_a_choice(r#""dice":2,"results":[]"#, 1)]
    #[case::a_choice_of_one_die(r#""choose":true,"results":[]"#, 1)]
    #[case::a_choice_with_a_table(
        r#""dice":2,"choose":true,"results":[{"from":1,"effects":[{"Draw":{}}]}]"#,
        1
    )]
    fn c11e_several_dice_are_only_a_choice_of_one_result_with_no_table(
        #[case] fields: &str,
        #[case] expected: usize,
    ) {
        let json = format!(
            r#"{{"Spell":{{"effects":[{{"RollDice":{{"sides":6,{fields}}}}},{{"Draw":{{"count":{{"DieResult":"Chosen"}}}}}}]}}}}"#
        );
        assert_eq!(problems(&json).len(), expected, "{json}");
    }

    #[test]
    fn a_good_ability_passes_and_names_what_it_uses() {
        let json = r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#;
        assert_eq!(problems(json), Vec::<String>::new());
        let ability: Ability = serde_json::from_str(json).unwrap();
        assert_eq!(
            used_primitives(&ability),
            ["Ability.Spell", "Effect.DealDamage"]
                .map(String::from)
                .into()
        );
    }

    #[test]
    fn a_cost_modifier_changes_the_cost_by_fixed_mana_and_never_by_an_x() {
        let json = r#"{"CostModifier":{"applies_to":["Spell"],"change":"Reduce","mana":"{X}"}}"#;
        let found = problems(json);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("not by an {X}"), "{found:?}");
    }

    #[rstest]
    #[case::plain_activation(r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Cost.Tap", "Effect.Draw"])]
    #[case::at_sorcery_speed(r#"{"Activated":{"cost":["Tap"],"timing":"Sorcery","effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Ability.Sorcery", "Cost.Tap", "Effect.Draw"])]
    #[case::once_a_turn(r#"{"Activated":{"cost":["Tap"],"limit":"OncePerTurn","effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Ability.Limit", "Cost.Tap", "Effect.Draw"])]
    #[case::from_the_graveyard(r#"{"Activated":{"cost":"{1}","from":"Graveyard","effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Ability.From", "Cost.Mana", "Effect.Draw"])]
    #[case::exiling_the_source_from_the_graveyard(r#"{"Activated":{"cost":[{"Mana":"{3}{G}{G}"},"ExileSelf"],"from":"Graveyard","effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Ability.From", "Cost.ExileSelf", "Cost.Mana", "Effect.Draw"])]
    #[case::exiling_the_source_from_the_battlefield(r#"{"Activated":{"cost":["ExileSelf"],"effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Cost.ExileSelf", "Effect.Draw"])]
    #[case::discarding_the_source_from_a_hand(r#"{"Activated":{"cost":[{"Mana":"{U/R}{U/R}"},"DiscardSelf"],"from":"Hand","effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Ability.From", "Cost.DiscardSelf", "Cost.Mana", "Effect.Draw"])]
    #[case::from_the_command_zone(r#"{"Activated":{"cost":"{2}","from":"Command","effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Ability.From", "Cost.Mana", "Effect.Draw"])]
    #[case::a_mana_ability_from_the_graveyard(r#"{"Activated":{"cost":"{1}","from":"Graveyard","effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}}]}}"#, &["Ability.Activated", "Ability.From", "Ability.ManaAbility.From", "Ability.ManaAbility.Other", "Cost.Mana", "Effect.AddMana"])]
    #[case::a_state_trigger(r#"{"Triggered":{"when":{"State":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Island"},{"ControlledBy":"You"}]},"op":"Eq","right":0}}}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#, &["Ability.Triggered", "Effect.Sacrifice", "Trigger.State"])]
    #[case::a_state_trigger_from_a_hand(r#"{"Triggered":{"when":{"State":{"condition":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Eq","right":0}}}},"from":"Hand","effects":[{"Draw":{}}]}}"#, &["Ability.Triggered", "Ability.From", "Effect.Draw", "Trigger.State"])]
    #[case::a_static_ability_from_the_graveyard(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Haste"}],"condition":{"Compare":{"left":{"Count":[{"Subtype":"Mountain"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"from":"Graveyard"}}"#, &["Ability.Static", "Ability.From", "Keyword.Haste", "Mod.AddKeyword"])]
    #[case::trigger_from_the_graveyard(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"limit":"OncePerTurn","from":"Graveyard","effects":[{"Draw":{}}]}}"#, &["Ability.Triggered", "Ability.Limit", "Ability.From", "Effect.Draw", "Trigger.StepBegins"])]
    #[case::a_discard_of_your_choice(r#"{"Spell":{"effects":[{"Discard":{"count":2}}]}}"#, &["Ability.Spell", "Effect.Discard"])]
    #[case::a_discard_at_random(r#"{"Spell":{"effects":[{"Discard":{"random":true}}]}}"#, &["Ability.Spell", "Effect.Discard", "Effect.Discard.Random"])]
    #[case::a_move_to_the_hand(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#, &["Ability.Spell", "Effect.Move"])]
    #[case::a_move_to_the_top_of_a_library(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":{"Library":{"place":"Top","order":"Chosen"}}}}]}}"#, &["Ability.Spell", "Effect.Move"])]
    #[case::a_move_shuffled_in(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":{"Library":{"place":"Shuffled","order":"Random"}}}}]}}"#, &["Ability.Spell", "Effect.Move"])]
    #[case::a_move_to_the_battlefield(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#, &["Ability.Spell", "Effect.Move", "Effect.Move.Battlefield"])]
    #[case::a_move_to_the_battlefield_transformed(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":{"Battlefield":{"transformed":true}}}}]}}"#, &["Ability.Spell", "Effect.Move", "Effect.Move.Battlefield", "Effect.Move.Transformed"])]
    #[case::a_move_to_the_top_in_a_random_order(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":{"Library":{"place":"Top","order":"Random"}}}}]}}"#, &["Ability.Spell", "Effect.Move", "Effect.Move.Random.Top"])]
    #[case::a_move_to_the_bottom_in_a_random_order(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#, &["Ability.Spell", "Effect.Move"])]
    #[case::a_named_token_brings_its_own_abilities(r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}"#, &["Ability.Activated", "Ability.Spell", "Cost.SacrificeSelf", "Cost.Tap", "Effect.AddMana", "Effect.CreateToken"])]
    #[case::an_inline_token_with_an_ability(r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Gem","types":["Artifact"],"abilities":[{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}]}},"count":2,"tapped":true}}]}}"#, &["Ability.Activated", "Ability.Spell", "Cost.SacrificeSelf", "Cost.Tap", "Effect.AddMana", "Effect.CreateToken"])]
    #[case::a_copy_of_an_object(r#"{"Spell":{"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"This"}}}}]}}"#, &["Ability.Spell", "Effect.CreateToken", "Effect.CreateToken.Copy"])]
    #[case::a_copy_of_a_spell(r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#, &["Ability.Spell", "Effect.CopySpell"])]
    #[case::a_copy_of_a_spell_with_an_exception_that_runs(r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"CopySpell":{"what":{"Target":0},"except":[{"SetColors":["Red"]}]}}]}}"#, &["Ability.Spell", "Effect.CopySpell", "Mod.SetColors"])]
    #[case::a_copy_of_a_spell_with_an_exception_that_waits(r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"CopySpell":{"what":{"Target":0},"except":["SwitchPT"]}}]}}"#, &["Ability.Spell", "Copy.Except.SwitchPT", "Effect.CopySpell", "Mod.SwitchPT"])]
    #[case::a_copy_of_an_ability(r#"{"Spell":{"targets":[{"what":{"Object":[{"Ability":"Triggered"}]}}],"effects":[{"CopyAbility":{"what":{"Target":0}}}]}}"#, &["Ability.Spell", "Effect.CopyAbility"])]
    #[case::a_copy_until_end_of_turn(r#"{"Spell":{"effects":[{"BecomeCopy":{"what":"This","of":"This","duration":"UntilEndOfTurn"}}]}}"#, &["Ability.Spell", "Effect.BecomeCopy"])]
    #[case::a_copy_without_end(r#"{"Spell":{"effects":[{"BecomeCopy":{"what":"This","of":"This","duration":"Forever"}}]}}"#, &["Ability.Spell", "Effect.BecomeCopy"])]
    #[case::a_copy_that_ends_with_another_duration(r#"{"Spell":{"effects":[{"BecomeCopy":{"what":"This","of":"This","duration":"UntilYourNextTurn"}}]}}"#, &["Ability.Spell", "Copy.Duration.UntilYourNextTurn", "Effect.BecomeCopy"])]
    #[case::storm_is_a_keyword(r#"{"Keyword":"Storm"}"#, &["Keyword.Storm"])]
    #[case::a_copy_with_the_exceptions_the_values_keep(r#"{"Spell":{"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"This","except":[{"AddKeyword":"Haste"},{"AddSubtype":"Zombie"},{"SetColors":["Blue"]},{"SetPT":{"power":1,"toughness":1}}]}}}}]}}"#, &["Ability.Spell", "Effect.CreateToken", "Effect.CreateToken.Copy", "Keyword.Haste", "Mod.AddKeyword", "Mod.AddSubtype", "Mod.SetColors", "Mod.SetPT"])]
    #[case::a_copy_with_an_exception_that_waits(r#"{"Spell":{"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"This","except":["SwitchPT"]}}}}]}}"#, &["Ability.Spell", "Copy.Except.SwitchPT", "Effect.CreateToken", "Effect.CreateToken.Copy", "Mod.SwitchPT"])]
    #[case::populate(r#"{"Spell":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"as":"token"}},{"CreateToken":{"token":{"CopyOf":{"of":{"Bound":"token"}}}}}]}}"#, &["Ability.Spell", "Effect.Choose", "Effect.CreateToken", "Effect.CreateToken.Copy"])]
    #[case::a_copy_that_is_exiled_at_the_end_step(r#"{"Spell":{"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"This","except":[{"AddKeyword":"Haste"}]}},"as":"copy"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Exile":{"what":{"Bound":"copy"}}}]}}]}}"#, &["Ability.Spell", "Effect.CreateToken", "Effect.CreateToken.Copy", "Effect.DelayedTrigger", "Effect.Exile", "Keyword.Haste", "Mod.AddKeyword", "Trigger.StepBegins"])]
    #[case::enters_as_a_copy(r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"}],"except":[{"AddSubtype":"Illusion"}]}}}}"#, &["Ability.Replacement", "Mod.AddSubtype", "ReplaceBy.AsCopy", "ReplaceWhen.Enters"])]
    #[case::a_cost_that_sacrifices_a_permanent(r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Cost.Sacrifice", "Effect.Draw"])]
    #[case::a_cost_that_sacrifices_the_source(r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Cost.SacrificeSelf", "Effect.Draw"])]
    #[case::a_clue_pays_mana_and_sacrifices_itself(r#"{"Activated":{"cost":[{"Mana":"{2}"},"SacrificeSelf"],"effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Cost.Mana", "Cost.SacrificeSelf", "Effect.Draw"])]
    #[case::ward_paid_with_mana(r#"{"Keyword":{"Ward":"{2}"}}"#, &["Cost.Mana", "Keyword.Ward"])]
    #[case::ward_paid_by_sacrificing_the_source(r#"{"Keyword":{"Ward":["SacrificeSelf"]}}"#, &["Cost.SacrificeSelf", "Effect.Pay.SacrificeSelf", "Keyword.Ward"])]
    #[case::a_payment_that_exiles_the_source(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Pay":{"cost":["ExileSelf"]}}]}}"#, &["Ability.Triggered", "Cost.ExileSelf", "Effect.Pay", "Effect.Pay.ExileSelf", "Trigger.StepBegins"])]
    #[case::a_payment_that_discards_the_source(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Pay":{"cost":["DiscardSelf"]}}]}}"#, &["Ability.Triggered", "Cost.DiscardSelf", "Effect.Pay", "Effect.Pay.DiscardSelf", "Trigger.StepBegins"])]
    #[case::a_payment_that_sacrifices_the_source(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Pay":{"cost":["SacrificeSelf"]}}]}}"#, &["Ability.Triggered", "Cost.SacrificeSelf", "Effect.Pay", "Effect.Pay.SacrificeSelf", "Trigger.StepBegins"])]
    #[case::an_x_in_the_cost_of_a_spell_is_read_by_its_effects(r#"{"Spell":{"effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Spell", "Effect.Draw"])]
    #[case::an_x_in_the_mana_of_an_ability_is_read_by_its_effects(r#"{"Activated":{"cost":"{X}{G}","effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Activated", "Cost.Mana", "Cost.Mana.X", "Effect.Draw"])]
    #[case::a_mode_picked_at_random(r#"{"Spell":{"effects":[{"Modal":{"at_random":true,"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"GainLife":{"amount":1}}]}]}}]}}"#, &["Ability.Spell", "Effect.Draw", "Effect.GainLife", "Effect.Modal", "Effect.Modal.AtRandom"])]
    #[case::a_mode_not_chosen_twice_in_a_turn(r#"{"Spell":{"effects":[{"Modal":{"once_each_turn":true,"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"GainLife":{"amount":1}}]}]}}]}}"#, &["Ability.Spell", "Effect.Draw", "Effect.GainLife", "Effect.Modal", "Effect.Modal.OnceEachTurn"])]
    #[case::c11d_a_mode_not_chosen_ever(r#"{"Spell":{"effects":[{"Modal":{"once_ever":true,"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"GainLife":{"amount":1}}]}]}}]}}"#, &["Ability.Spell", "Effect.Draw", "Effect.GainLife", "Effect.Modal", "Effect.Modal.OnceEver"])]
    #[case::an_x_beside_a_tap(r#"{"Activated":{"cost":[{"Mana":"{X}{R}{G}"},"Tap"],"effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Activated", "Cost.Mana", "Cost.Mana.X", "Cost.Tap", "Effect.Draw"])]
    #[case::an_exile_until_the_source_leaves(r#"{"Spell":{"effects":[{"Exile":{"what":"This","duration":"WhileSourceOnBattlefield"}}]}}"#, &["Ability.Spell", "Effect.Exile"])]
    #[case::a_control_for_as_long_as_you_control_the_source(r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"WhileYouControlSource"}}]}}"#, &["Ability.Triggered", "Duration.WhileYouControlSource", "Effect.Continuous", "Mod.SetController", "Trigger.ZoneChange"])]
    #[case::a_rule_for_as_long_as_you_control_the_source(r#"{"Spell":{"effects":[{"Rule":{"rule":{"CantDraw":"You"},"duration":"WhileYouControlSource"}}]}}"#, &["Ability.Spell", "Effect.Rule", "Rule.CantDraw", "Rule.Duration.WhileYouControlSource"])]
    #[case::an_exile_with_another_duration(r#"{"Spell":{"effects":[{"Exile":{"what":"This","duration":"UntilYourNextTurn"}}]}}"#, &["Ability.Spell", "Effect.Exile", "Exile.Duration.UntilYourNextTurn"])]
    #[case::mana_spent_to_cast(r#"{"Spell":{"effects":[{"If":{"condition":{"ManaSpent":"Red"},"then":[{"Draw":{}}]}}]}}"#, &["Ability.Spell", "Condition.ManaSpent", "Effect.Draw", "Effect.If"])]
    #[case::an_x_that_the_ability_never_announced(r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Activated", "Amount.X.Elsewhere", "Cost.Tap", "Effect.Draw"])]
    #[case::an_x_in_a_triggered_ability(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Triggered", "Amount.X.Elsewhere", "Effect.Draw", "Trigger.StepBegins"])]
    #[case::an_x_in_a_delayed_trigger_the_spell_leaves_behind(r#"{"Spell":{"effects":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Draw":{"count":"X"}}]}}]}}"#, &["Ability.Spell", "Effect.DelayedTrigger", "Effect.Draw", "Trigger.StepBegins"])]
    #[case::an_x_in_a_delayed_trigger_an_ability_without_x_leaves_behind(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Draw":{"count":"X"}}]}}]}}"#, &["Ability.Triggered", "Amount.X.Elsewhere", "Effect.DelayedTrigger", "Effect.Draw", "Trigger.StepBegins"])]
    #[case::an_x_in_the_enters_trigger_of_the_card(r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Triggered", "Effect.Draw", "Trigger.ZoneChange"])]
    #[case::an_x_in_the_dies_trigger_of_the_card(r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Triggered", "Amount.X.Elsewhere", "Effect.Draw", "Trigger.ZoneChange"])]
    #[case::an_x_in_the_enters_trigger_of_another_card(r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other"],"to":"Battlefield"}},"effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Triggered", "Amount.X.Elsewhere", "Effect.Draw", "Trigger.ZoneChange"])]
    #[case::an_x_in_the_intervening_if_of_the_enters_trigger(r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Compare":{"left":"X","op":"Ge","right":5}},"effects":[{"Draw":{}}]}}"#, &["Ability.Triggered", "Effect.Draw", "Trigger.ZoneChange"])]
    #[case::an_x_in_the_intervening_if_of_another_trigger(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":"X","op":"Ge","right":5}},"effects":[{"Draw":{}}]}}"#, &["Ability.Triggered", "Amount.X.Elsewhere", "Effect.Draw", "Trigger.StepBegins"])]
    #[case::an_x_in_the_intervening_if_of_the_dies_trigger(r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"if":{"Compare":{"left":"X","op":"Ge","right":5}},"effects":[{"Draw":{}}]}}"#, &["Ability.Triggered", "Amount.X.Elsewhere", "Effect.Draw", "Trigger.ZoneChange"])]
    #[case::an_x_in_the_cast_trigger_of_the_spell(r#"{"Triggered":{"when":{"SpellCast":{"what":["This"],"by":"You"}},"from":"Stack","effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.From", "Ability.Triggered", "Effect.Draw", "Trigger.SpellCast"])]
    #[case::an_x_in_the_trigger_on_another_spell_cast(r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Triggered", "Effect.Draw", "Trigger.SpellCast"])]
    #[case::an_x_in_the_counters_the_card_enters_with(r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":"X"}}}}"#, &["Ability.Replacement", "ReplaceBy.WithCounters", "ReplaceWhen.Enters"])]
    #[case::an_x_in_the_counters_another_card_enters_with(r#"{"Replacement":{"when":{"Enters":{"what":[{"Type":"Creature"},"Other"]}},"by":{"WithCounters":{"counter":"+1/+1","count":"X"}}}}"#, &["Ability.Replacement", "Amount.X.Elsewhere", "ReplaceBy.WithCounters", "ReplaceWhen.Enters"])]
    #[case::an_x_in_what_a_spell_targets(r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}]}}],"effects":[{"Destroy":{"what":{"Target":0},"regenerate":false}}]}}"#, &["Ability.Spell", "Effect.Destroy"])]
    #[case::an_x_in_what_an_enters_trigger_targets(r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}]}}],"effects":[{"Destroy":{"what":{"Target":0},"regenerate":false}}]}}"#, &["Ability.Triggered", "Effect.Destroy", "Trigger.ZoneChange"])]
    #[case::an_x_in_what_another_trigger_targets(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}]}}],"effects":[{"Destroy":{"what":{"Target":0},"regenerate":false}}]}}"#, &["Ability.Triggered", "Amount.X.Elsewhere", "Effect.Destroy", "Trigger.StepBegins"])]
    #[case::an_x_in_a_loyalty_ability_that_removes_no_x(r#"{"Loyalty":{"cost":{"Minus":2},"effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Loyalty", "Amount.X.Elsewhere", "Effect.Draw"])]
    #[case::an_x_in_the_effects_of_a_loyalty_ability_that_removes_x(r#"{"Loyalty":{"cost":"MinusX","effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Loyalty", "Cost.Loyalty.X", "Effect.Draw"])]
    #[case::an_x_in_the_ability_a_token_carries(r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Gem","types":["Artifact"],"abilities":[{"Activated":{"cost":["Tap"],"effects":[{"Draw":{"count":"X"}}]}}]}}}}]}}"#, &["Ability.Activated", "Ability.Spell", "Amount.X.Elsewhere", "Cost.Tap", "Effect.CreateToken", "Effect.Draw"])]
    #[case::c6_eg_counted_life_in_an_effect(r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"effects":[{"Pay":{"cost":[{"PayLife":"Triggering"}],"paid":[{"Draw":{"count":"Triggering"}}]}}]}}"#, &["Ability.Triggered", "Amount.Triggering", "Cost.PayLife", "Effect.Draw", "Effect.Pay", "Effect.Pay.Counted", "Trigger.LifeGained"])]
    #[case::c6_eg_counted_life_in_a_cost(r#"{"Activated":{"cost":[{"PayLife":{"Life":"You"}}],"effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Cost.PayLife", "Cost.PayLife.Counted", "Effect.Draw"])]
    #[case::a_payment_of_x_in_an_effect(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Pay":{"cost":"{X}"}}]}}"#, &["Ability.Triggered", "Cost.Mana", "Effect.Pay", "Effect.Pay.X", "Trigger.StepBegins"])]
    #[case::w6c_the_x_a_payment_announces(r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":"AnyTarget"}],"effects":[{"Pay":{"cost":"{X}{R}","paid":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}]}}"#, &["Ability.Triggered", "Cost.Mana", "Effect.DealDamage", "Effect.Pay", "Effect.Pay.X", "Trigger.Attacks"])]
    #[case::w6c_a_part_that_counts_x_in_a_payment(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Pay":{"cost":[{"PayLife":"X"}]}}]}}"#, &["Ability.Triggered", "Amount.X.Elsewhere", "Cost.PayLife", "Effect.Pay", "Effect.Pay.X.Parts", "Trigger.StepBegins"])]
    #[case::w6c_the_x_of_the_cycling_cost(r#"{"Triggered":{"when":{"Cycled":{"what":["This"],"by":"You"}},"effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Triggered", "Effect.Draw", "Trigger.Cycled"])]
    #[case::w6c_the_x_of_a_monstrosity(r#"{"Triggered":{"when":{"BecomesMonstrous":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":"X"}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#, &["Ability.Triggered", "Effect.Tap", "Trigger.BecomesMonstrous"])]
    #[case::w6c_the_x_of_another_cards_cycling(r#"{"Triggered":{"when":{"Cycled":{"by":"You"}},"effects":[{"Draw":{"count":"X"}}]}}"#, &["Ability.Triggered", "Amount.X.Elsewhere", "Effect.Draw", "Trigger.Cycled"])]
    #[case::attaching_the_source_to_a_target(r#"{"Activated":{"cost":"{2}","timing":"Sorcery","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Attach":{"what":"This","to":{"Target":0}}}]}}"#, &["Ability.Activated", "Ability.Sorcery", "Cost.Mana", "Effect.Attach"])]
    #[case::equip_pays_a_mana_cost(r#"{"Keyword":{"Equip":"{2}"}}"#, &["Cost.Mana", "Keyword.Equip"])]
    #[case::reconfigure_pays_a_mana_cost(r#"{"Keyword":{"Reconfigure":"{2}"}}"#, &["Cost.Mana", "Keyword.Reconfigure"])]
    #[case::unattaching_the_source_only_while_it_is_attached(r#"{"Activated":{"cost":"{2}","timing":"Sorcery","condition":{"Compare":{"left":{"Count":[{"Is":"Attached"},{"Type":"Creature"}]},"op":"Ge","right":1}},"effects":[{"Unattach":{"what":"This"}}]}}"#, &["Ability.Activated", "Ability.Condition", "Ability.Sorcery", "Cost.Mana", "Effect.Unattach"])]
    #[case::an_aura_that_enchants_a_creature(r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}}"#, &["Keyword.Enchant"])]
    #[case::an_aura_that_enchants_a_card_in_a_graveyard(r#"{"Keyword":{"Enchant":[{"Type":"Creature"},{"In":"Graveyard"}]}}"#, &["Keyword.Enchant", "Keyword.Enchant.Elsewhere"])]
    #[case::a_treasure_like_mana_ability(r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#, &["Ability.Activated", "Cost.SacrificeSelf", "Cost.Tap", "Effect.AddMana"])]
    #[case::sol_ring(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}]}}"#, &["Ability.Activated", "Ability.ManaAbility.Complex", "Cost.Tap", "Effect.AddMana"])]
    #[case::mana_for_creature_spells_only(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1},"only":[{"Spell":{"types":["Creature"]}}]}}]}}"#, &["Ability.Activated", "Cost.Tap", "Effect.AddMana", "Effect.AddMana.Restricted"])]
    #[case::a_cost_increase(r#"{"CostModifier":{"applies_to":["Spell",{"Not":{"Type":"Creature"}}],"change":"Increase","mana":"{1}"}}"#, &["Ability.CostModifier"])]
    #[case::a_cost_reduction_for_each_creature(r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}","per":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#, &["Ability.CostModifier"])]
    #[case::a_condition_on_a_cost_modifier(r#"{"CostModifier":{"applies_to":["Spell"],"change":"Reduce","mana":"{1}","condition":"YourTurn"}}"#, &["Ability.CostModifier"])]
    fn how_often_from_where_and_by_what_chance_an_ability_works_counts_as_a_primitive(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(problems(json), Vec::<String>::new());
        let ability: Ability = serde_json::from_str(json).unwrap();
        let expected: BTreeSet<String> = expected.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(used_primitives(&ability), expected);
    }

    const ALL_MINE: &str = r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#;

    #[rstest]
    #[case::an_anthem(ALL_MINE, r#"{"ModifyPT":{"power":1,"toughness":1}}"#, &["Mod.ModifyPT"])]
    #[case::set_power_and_toughness(ALL_MINE, r#"{"SetPT":{"power":1,"toughness":1}}"#, &["Mod.SetPT"])]
    #[case::switching_them(r#""This""#, r#""SwitchPT""#, &["Mod.SwitchPT"])]
    #[case::a_type(ALL_MINE, r#"{"AddType":"Artifact"}"#, &["Mod.AddType"])]
    #[case::a_type_removed(ALL_MINE, r#"{"RemoveType":"Creature"}"#, &["Mod.RemoveType"])]
    #[case::a_subtype(ALL_MINE, r#"{"AddSubtype":"Elf"}"#, &["Mod.AddSubtype"])]
    #[case::colours(ALL_MINE, r#"{"SetColors":["Blue"]}"#, &["Mod.SetColors"])]
    #[case::a_keyword(ALL_MINE, r#"{"AddKeyword":"Flying"}"#, &["Keyword.Flying", "Mod.AddKeyword"])]
    #[case::a_keyword_taken_away(ALL_MINE, r#"{"RemoveKeyword":"Flying"}"#, &["Keyword.Flying", "Mod.RemoveKeyword"])]
    #[case::protection(ALL_MINE, r#"{"AddKeyword":{"Protection":[{"Color":"Red"}]}}"#, &["Keyword.Protection", "Mod.AddKeyword"])]
    #[case::shroud(ALL_MINE, r#"{"AddKeyword":"Shroud"}"#, &["Keyword.Shroud", "Mod.AddKeyword"])]
    #[case::hexproof_for_you(r#""You""#, r#"{"AddKeyword":"Hexproof"}"#, &["Keyword.Hexproof", "Mod.AddKeyword"])]
    #[case::shroud_for_each_player(r#""EachPlayer""#, r#"{"AddKeyword":"Shroud"}"#, &["Keyword.Shroud", "Mod.AddKeyword"])]
    #[case::protection_for_opponents(r#""Opponents""#, r#"{"AddKeyword":{"Protection":[{"Color":"Red"}]}}"#, &["Keyword.Protection", "Mod.AddKeyword"])]
    #[case::hexproof_taken_from_the_enchanted_player(r#""AttachedPlayer""#, r#"{"RemoveKeyword":"Hexproof"}"#, &["Keyword.Hexproof", "Mod.RemoveKeyword"])]
    #[case::every_ability_lost(ALL_MINE, r#""LoseAbilities""#, &["Mod.LoseAbilities"])]
    #[case::control_of_the_enchanted_creature(r#""Attached""#, r#"{"SetController":"You"}"#, &["Mod.SetController"])]
    #[case::not_attacking(ALL_MINE, r#""CantAttack""#, &["Mod.CantAttack"])]
    #[case::not_blocking(r#""This""#, r#""CantBlock""#, &["Mod.CantBlock"])]
    #[case::an_activated_ability(
        ALL_MINE,
        r#"{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}}"#,
        &["Ability.Activated", "Cost.Tap", "Effect.Draw", "Mod.GrantAbility"]
    )]
    #[case::a_triggered_ability_with_a_target(
        ALL_MINE,
        r#"{"GrantAbility":{"Triggered":{"when":{"Attacks":{"what":[{"This":null}]}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}}"#,
        &["Ability.Triggered", "Effect.DealDamage", "Mod.GrantAbility", "Trigger.Attacks"]
    )]
    #[case::a_static_ability(
        ALL_MINE,
        r#"{"GrantAbility":{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}}"#,
        &["Mod.GrantAbility", "Mod.ModifyPT"]
    )]
    fn a_static_ability_names_the_modifications_it_uses(
        #[case] affected: &str,
        #[case] modification: &str,
        #[case] names: &[&str],
    ) {
        let json = format!(r#"{{"Static":{{"affected":{affected},"mods":[{modification}]}}}}"#);
        assert_eq!(problems(&json), Vec::<String>::new());
        let ability: Ability = serde_json::from_str(&json).unwrap();
        let expected: BTreeSet<String> = ["Ability.Static"]
            .iter()
            .chain(names)
            .map(|name| (*name).to_owned())
            .collect();
        assert_eq!(used_primitives(&ability), expected);
    }

    #[rstest]
    #[case::a_spell_is_not_granted(
        r#"{"Static":{"affected":"This","mods":[{"GrantAbility":{"Spell":{"effects":[{"Draw":{}}]}}}]}}"#,
        "abilities[0].Static.mods[0]: only an activated, a triggered or a static ability can be granted"
    )]
    #[case::a_keyword_is_not_granted_this_way(
        r#"{"Static":{"affected":"This","mods":[{"GrantAbility":{"Keyword":"Flying"}}]}}"#,
        "abilities[0].Static.mods[0]: only an activated, a triggered or a static ability can be granted"
    )]
    #[case::a_granted_static_cannot_grant_in_turn(
        r#"{"Static":{"affected":"This","mods":[{"GrantAbility":{"Static":{"affected":"This","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}}]}}}]}}"#,
        "a static ability that is granted cannot grant abilities in turn"
    )]
    #[case::a_granted_static_is_not_characteristic_defining(
        r#"{"Static":{"affected":"This","mods":[{"GrantAbility":{"Static":{"affected":"This","mods":[{"SetColors":["Blue"]}],"defining":true}}}]}}"#,
        "a static ability that is granted cannot be characteristic-defining"
    )]
    #[case::a_granted_ability_has_a_scope_of_its_own(
        r#"{"Static":{"affected":"This","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"Draw":{"who":{"Target":0}}}]}}}]}}"#,
        "mods[0].GrantAbility.Activated.effects[0].Draw.who: target 0 does not exist"
    )]
    #[case::a_colour_is_chosen_as_an_effect_begins(
        r#"{"Static":{"affected":"This","mods":["AddProtectionFromChosenColor"]}}"#,
        "a colour is chosen as an effect begins, so only a `Continuous` effect adds protection from it"
    )]
    #[case::a_supertype_goes_with_a_copy_only(
        r#"{"Static":{"affected":"This","mods":[{"RemoveSupertype":"Legendary"}]}}"#,
        "a supertype is taken away only from the copiable values of a copy (`except`)"
    )]
    #[case::a_player_takes_only_the_keywords_players_have(
        r#"{"Static":{"affected":"You","mods":[{"AddKeyword":"Flying"}]}}"#,
        "abilities[0].Static.affected: expected an object but this is a player"
    )]
    #[case::a_player_takes_keywords_alone(
        r#"{"Static":{"affected":"You","mods":[{"AddKeyword":"Hexproof"},{"AddType":"Artifact"}]}}"#,
        "abilities[0].Static.affected: expected an object but this is a player"
    )]
    #[case::a_player_is_not_an_affected_object(
        r#"{"Static":{"affected":"You","mods":[{"AddType":"Artifact"}]}}"#,
        "abilities[0].Static.affected: expected an object but this is a player"
    )]
    #[case::a_target_is_not_chosen_by_a_static(
        r#"{"Static":{"affected":{"Target":0},"mods":[{"AddType":"Artifact"}]}}"#,
        "abilities[0].Static.affected: target 0 does not exist"
    )]
    #[case::a_static_changes_something(
        r#"{"Static":{"affected":"This","mods":[]}}"#,
        "abilities[0].Static.mods: needs at least one modification"
    )]
    #[case::control_goes_to_a_player(
        r#"{"Static":{"affected":"This","mods":[{"SetController":"This"}]}}"#,
        "abilities[0].Static.mods[0]: expected a player but this is an object"
    )]
    fn a_bad_static_ability_is_named_where_it_is_wrong(#[case] json: &str, #[case] message: &str) {
        let found = problems(json);
        assert!(
            found.iter().any(|problem| problem.contains(message)),
            "{message:?} is not among {found:#?}"
        );
    }

    #[rstest]
    #[case::must_attack(r#"{"MustAttack":"This"}"#, "MustAttack")]
    #[case::must_block(r#"{"MustBlock":{"Each":[{"Type":"Creature"}]}}"#, "MustBlock")]
    #[case::not_countered(
        r#"{"CantBeCountered":{"Each":["Spell",{"ControlledBy":"You"}]}}"#,
        "CantBeCountered"
    )]
    #[case::no_life_gain(r#"{"CantGainLife":"EachPlayer"}"#, "CantGainLife")]
    #[case::no_life_loss_from_damage(r#"{"NoLifeLossFromDamage":"You"}"#, "NoLifeLossFromDamage")]
    #[case::radiation_gains_life(r#"{"RadiationGainsLife":"You"}"#, "RadiationGainsLife")]
    #[case::no_draw(r#"{"CantDraw":"Opponents"}"#, "CantDraw")]
    #[case::no_search(r#"{"CantSearch":"You"}"#, "CantSearch")]
    #[case::a_revealed_top(r#"{"RevealTop":"You"}"#, "RevealTop")]
    #[case::triggers_again(
        r#"{"TriggerAgain":{"cause":{"ZoneChange":{"what":[{"Type":"Creature"}],"to":"Battlefield"}}}}"#,
        "TriggerAgain"
    )]
    fn a_rule_names_itself_and_the_patterns_it_uses_as_a_static_ability_and_as_an_effect(
        #[case] rule: &str,
        #[case] name: &str,
    ) {
        let pattern = rule.contains("TriggerAgain");
        for (json, head) in [
            (format!(r#"{{"Rule":{{"rule":{rule}}}}}"#), "Ability.Rule"),
            (
                format!(
                    r#"{{"Spell":{{"effects":[{{"Rule":{{"rule":{rule},"duration":"UntilEndOfTurn"}}}}]}}}}"#
                ),
                "Effect.Rule",
            ),
        ] {
            assert_eq!(problems(&json), Vec::<String>::new(), "{json}");
            let ability: Ability = serde_json::from_str(&json).unwrap();
            let mut expected: BTreeSet<String> = [head, &format!("Rule.{name}")]
                .iter()
                .map(|name| (*name).to_owned())
                .collect();
            if head == "Effect.Rule" {
                expected.insert("Ability.Spell".to_owned());
            }
            if pattern {
                expected.insert("Trigger.ZoneChange".to_owned());
            }
            assert_eq!(used_primitives(&ability), expected, "{json}");
        }
    }

    #[rstest]
    #[case::life_of_an_object(
        r#"{"Rule":{"rule":{"CantGainLife":"This"}}}"#,
        "abilities[0].Rule.rule.CantGainLife: expected a player but this is an object"
    )]
    #[case::attack_of_a_player(
        r#"{"Rule":{"rule":{"MustAttack":"You"}}}"#,
        "abilities[0].Rule.rule.MustAttack: expected an object but this is a player"
    )]
    #[case::a_target_in_a_static_rule(
        r#"{"Rule":{"rule":{"MustAttack":{"Target":0}}}}"#,
        "abilities[0].Rule.rule.MustAttack: target 0 does not exist"
    )]
    #[case::a_target_in_the_cause(
        r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"ZoneChange":{"what":[{"Is":{"Target":0}}]}}}}}}"#,
        "abilities[0].Rule.rule.TriggerAgain.cause.what[0]: target 0 does not exist"
    )]
    #[case::the_triggering_event_in_the_cause(
        r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"ZoneChange":{"what":[{"Is":"Triggering"}]}}}}}}"#,
        "`Triggering` is only valid in a triggered ability"
    )]
    #[case::tapping_from_the_graveyard(
        r#"{"Activated":{"cost":["Tap"],"from":"Graveyard","effects":[{"Draw":{}}]}}"#,
        "abilities[0].Activated.from: `Tap` is paid from the battlefield"
    )]
    #[case::sacrificing_the_source_from_a_hand(
        r#"{"Activated":{"cost":["SacrificeSelf"],"from":"Hand","effects":[{"Draw":{}}]}}"#,
        "abilities[0].Activated.from: `SacrificeSelf` is paid from the battlefield"
    )]
    #[case::discarding_the_source_from_the_battlefield(
        r#"{"Activated":{"cost":["DiscardSelf"],"effects":[{"Draw":{}}]}}"#,
        "abilities[0].Activated.from: `DiscardSelf` is paid from a hand"
    )]
    #[case::activated_from_the_library(
        r#"{"Activated":{"cost":"{1}","from":"Library","effects":[{"Draw":{}}]}}"#,
        "abilities[0].Activated.from: an activated ability works from the battlefield, a hand"
    )]
    #[case::activated_from_the_stack(
        r#"{"Activated":{"cost":"{1}","from":"Stack","effects":[{"Draw":{}}]}}"#,
        "abilities[0].Activated.from: an activated ability works from the battlefield, a hand"
    )]
    #[case::a_state_among_events(
        r#"{"Triggered":{"when":{"AnyOf":[{"Attacks":{"what":["This"]}},{"State":{"condition":"YourTurn"}}]},"effects":[{"Draw":{}}]}}"#,
        "abilities[0].Triggered.when[1]: a state is the whole pattern of a triggered ability"
    )]
    #[case::a_state_in_a_delayed_trigger(
        r#"{"Spell":{"effects":[{"DelayedTrigger":{"when":{"State":{"condition":"YourTurn"}},"effects":[{"Draw":{}}]}}]}}"#,
        "a state is the whole pattern of a triggered ability"
    )]
    #[case::a_state_in_the_cause_of_a_rule(
        r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"State":{"condition":"YourTurn"}}}}}}"#,
        "a state is the whole pattern of a triggered ability"
    )]
    #[case::the_triggering_event_in_a_state(
        r#"{"Triggered":{"when":{"State":{"condition":{"Matches":{"subject":"Triggering","selector":[]}}}},"effects":[{"Draw":{}}]}}"#,
        "`Triggering` is only valid in a triggered ability"
    )]
    #[case::the_triggering_event_in_what_a_state_does(
        r#"{"Triggered":{"when":{"State":{"condition":"YourTurn"}},"effects":[{"Draw":{"who":"Triggering"}}]}}"#,
        "`Triggering` is only valid in a triggered ability"
    )]
    #[case::a_static_ability_from_the_graveyard_that_affects_itself(
        r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"from":"Graveyard"}}"#,
        "abilities[0].Static.affected: a static ability that works from another zone affects the objects a selector names"
    )]
    #[case::from_a_hand(
        r#"{"Rule":{"rule":{"CantDraw":"You"},"from":"Hand"}}"#,
        "abilities[0].Rule.from: a rule works from the battlefield"
    )]
    #[case::from_the_stack_for_another_rule(
        r#"{"Rule":{"rule":{"CantDraw":"You"},"from":"Stack"}}"#,
        "abilities[0].Rule.from: a rule works from the battlefield"
    )]
    #[case::from_the_stack_for_another_spell(
        r#"{"Rule":{"rule":{"CantBeCountered":{"Each":["Spell"]}},"from":"Stack"}}"#,
        "abilities[0].Rule.from: a rule works from the battlefield"
    )]
    #[case::a_rule_is_not_granted(
        r#"{"Static":{"affected":"This","mods":[{"GrantAbility":{"Rule":{"rule":{"CantDraw":"You"}}}}]}}"#,
        "only an activated, a triggered or a static ability can be granted"
    )]
    fn a_bad_rule_zone_cost_or_state_is_named_where_it_is_wrong(
        #[case] json: &str,
        #[case] message: &str,
    ) {
        let found = problems(json);
        assert!(
            found.iter().any(|problem| problem.contains(message)),
            "{message:?} is not among {found:#?}"
        );
    }

    #[test]
    fn this_spell_cant_be_countered_works_from_the_stack_and_names_no_zone_gap() {
        let json = r#"{"Rule":{"rule":{"CantBeCountered":"This"},"from":"Stack"}}"#;
        assert_eq!(problems(json), Vec::<String>::new());
        let ability: Ability = serde_json::from_str(json).unwrap();
        let expected: BTreeSet<String> = ["Ability.Rule", "Rule.CantBeCountered"]
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        assert_eq!(used_primitives(&ability), expected, "no `Ability.From`");
    }

    #[rstest]
    #[case::improvise(r#""Improvise""#, true)]
    #[case::convoke(r#""Convoke""#, true)]
    #[case::delve(r#""Delve""#, true)]
    #[case::flying(r#""Flying""#, false)]
    #[case::cascade(r#""Cascade""#, false)]
    fn a_static_ability_over_spells_gives_only_the_keywords_that_help_pay(
        #[case] keyword: &str,
        #[case] allowed: bool,
    ) {
        let json = format!(
            r#"{{"Static":{{"affected":{{"Each":["Spell",{{"ControlledBy":"You"}}]}},"mods":[{{"AddKeyword":{keyword}}}]}}}}"#
        );
        assert_eq!(problems(&json).is_empty(), allowed, "{keyword}");
        let ability: Ability = serde_json::from_str(&json).unwrap();
        assert!(used_primitives(&ability).contains("Ability.Static.SpellKeywords"));
    }

    const SWAMPS: &str = r#"{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]}"#;

    #[rstest]
    #[case::power_and_toughness_from_a_count(
        format!(r#"{{"SetPT":{{"power":{SWAMPS},"toughness":{SWAMPS}}}}}"#),
        &["Mod.SetPT"]
    )]
    #[case::colours(r#"{"SetColors":["Blue","Red"]}"#.to_owned(), &["Mod.SetColors"])]
    #[case::a_type(r#"{"AddType":"Artifact"}"#.to_owned(), &["Mod.AddType"])]
    #[case::a_subtype(r#"{"AddSubtype":"Elf"}"#.to_owned(), &["Mod.AddSubtype"])]
    fn a_characteristic_defining_ability_names_that_it_defines(
        #[case] modification: String,
        #[case] names: &[&str],
    ) {
        let json = format!(
            r#"{{"Static":{{"affected":"This","mods":[{modification}],"defining":true}}}}"#
        );
        assert_eq!(problems(&json), Vec::<String>::new());
        let ability: Ability = serde_json::from_str(&json).unwrap();
        let expected: BTreeSet<String> = ["Ability.Static", "Ability.Static.Defining"]
            .iter()
            .chain(names)
            .map(|name| (*name).to_owned())
            .collect();
        assert_eq!(used_primitives(&ability), expected);
        // The flag is part of the data and stays off when it is not set.
        let plain = json.replace(r#","defining":true"#, "");
        assert!(
            !used_primitives(&serde_json::from_str(&plain).unwrap())
                .contains("Ability.Static.Defining")
        );
        assert_eq!(serde_json::to_string(&ability).unwrap(), json);
    }

    #[rstest]
    #[case::only_its_own_object(
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"SetColors":["Blue"]}],"defining":true}}"#,
        "abilities[0].Static.affected: a characteristic-defining ability affects only its own object"
    )]
    #[case::no_condition(
        r#"{"Static":{"affected":"This","mods":[{"SetColors":["Blue"]}],"condition":"YourTurn","defining":true}}"#,
        "abilities[0].Static.condition: a characteristic-defining ability has no condition"
    )]
    #[case::every_zone(
        r#"{"Static":{"affected":"This","mods":[{"SetColors":["Blue"]}],"defining":true,"from":"Graveyard"}}"#,
        "abilities[0].Static.from: a characteristic-defining ability works in every zone"
    )]
    #[case::not_a_pump(
        r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"defining":true}}"#,
        "abilities[0].Static.mods[0]: a characteristic-defining ability only sets"
    )]
    #[case::not_abilities(
        r#"{"Static":{"affected":"This","mods":[{"SetColors":["Blue"]},{"AddKeyword":"Flying"}],"defining":true}}"#,
        "abilities[0].Static.mods[1]: a characteristic-defining ability only sets"
    )]
    fn a_characteristic_defining_ability_that_defines_more_than_its_own_characteristics_is_refused(
        #[case] json: &str,
        #[case] message: &str,
    ) {
        let found = problems(json);
        assert!(
            found.iter().any(|problem| problem.contains(message)),
            "{message:?} is not among {found:#?}"
        );
    }

    #[test]
    fn a_granted_triggered_ability_leaves_the_scope_of_the_static_as_it_was() {
        // The granted ability is a trigger, where `Triggering` is valid; the condition after it is not.
        let json = r#"{"Static":{"affected":"This","mods":[{"GrantAbility":{"Triggered":{"when":{"Attacks":{"what":[{"This":null}]}},"effects":[{"Draw":{}}]}}}],"condition":{"Matches":{"subject":"Triggering","selector":[]}}}}"#;
        let found = problems(json);
        assert!(
            found
                .iter()
                .any(|p| p.contains("`Triggering` is only valid in a triggered ability")),
            "{found:#?}"
        );
    }

    #[rstest]
    #[case::counters_in_a_condition(
        r#"{"Spell":{"effects":[{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":"+1/+1"}},"op":"Ge","right":1}},"then":[{"Draw":{}}]}}]}}"#,
        &["Ability.Spell", "Effect.Draw", "Effect.If"]
    )]
    #[case::counters_as_an_amount(
        r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Counters":{"on":"This","counter":"+1/+1"}}}}]}}"#,
        &["Ability.Spell", "Effect.DealDamage"]
    )]
    #[case::counters_inside_a_sum_inside_a_negation(
        r#"{"Spell":{"effects":[{"If":{"condition":{"Not":{"Compare":{"left":{"Sum":[1,{"Counters":{"on":"You","counter":"poison"}}]},"op":"Lt","right":3}}},"then":[]}}]}}"#,
        &["Ability.Spell", "Effect.If"]
    )]
    #[case::a_selector_over_counters(
        r#"{"Spell":{"effects":[{"Draw":{"count":{"Count":[{"Type":"Creature"},{"HasCounter":"+1/+1"}]}}}]}}"#,
        &["Ability.Spell", "Effect.Draw"]
    )]
    #[case::putting_counters_on_a_player(
        r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"poison","count":3}}]}}"#,
        &["Ability.Spell", "Effect.PutCounters"]
    )]
    #[case::removing_counters(
        r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"RemoveCounters":{"from":{"Target":0},"counter":"-1/-1"}}]}}"#,
        &["Ability.Spell", "Effect.RemoveCounters"]
    )]
    #[case::moving_counters(
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"MoveCounters":{"from":{"Target":0},"to":{"Target":1},"counter":"+1/+1","count":2}}]}}"#,
        &["Ability.Spell", "Effect.MoveCounters"]
    )]
    fn counters_are_read_like_any_other_value_and_the_effects_that_change_them_are_primitives(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(problems(json), Vec::<String>::new());
        let ability: Ability = serde_json::from_str(json).unwrap();
        let expected: BTreeSet<String> = expected.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(used_primitives(&ability), expected);
    }

    #[rstest]
    #[case::removing_counters_from_a_name_nothing_bound(
        r#"{"Spell":{"effects":[{"RemoveCounters":{"from":{"Bound":"nothing"},"counter":"+1/+1"}}]}}"#,
        "`nothing` is not bound by an earlier step"
    )]
    #[case::moving_counters_from_a_player(
        r#"{"Spell":{"effects":[{"MoveCounters":{"from":"You","to":"This","counter":"poison"}}]}}"#,
        "expected an object"
    )]
    #[case::moving_counters_onto_a_player(
        r#"{"Spell":{"effects":[{"MoveCounters":{"from":"This","to":"You","counter":"poison"}}]}}"#,
        "expected an object"
    )]
    #[case::counters_of_a_target_that_does_not_exist(
        r#"{"Spell":{"effects":[{"Draw":{"count":{"Counters":{"on":{"Target":0},"counter":"charge"}}}}]}}"#,
        "target 0 does not exist"
    )]
    fn the_references_of_counter_effects_and_amounts_are_checked(
        #[case] json: &str,
        #[case] message: &str,
    ) {
        let found = problems(json);
        assert!(
            found.iter().any(|problem| problem.contains(message)),
            "{message:?} is not among {found:#?}"
        );
    }

    #[rstest]
    #[case::loyalty_as_a_stat(
        r#"{"Spell":{"effects":[{"If":{"condition":{"Compare":{"left":{"Stat":{"of":"This","stat":"Loyalty"}},"op":"Ge","right":1}},"then":[]}}]}}"#,
        &["Ability.Spell", "Effect.If"]
    )]
    #[case::loyalty_in_a_selector(
        r#"{"Spell":{"effects":[{"If":{"condition":{"Matches":{"subject":"This","selector":[{"Compare":{"stat":"Loyalty","op":"Ge","to":3}}]}},"then":[]}}]}}"#,
        &["Ability.Spell", "Effect.If"]
    )]
    #[case::a_loyalty_ability(
        r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"Draw":{}}]}}"#,
        &["Ability.Loyalty", "Effect.Draw"]
    )]
    #[case::a_loyalty_ability_that_reads_loyalty(
        r#"{"Loyalty":{"cost":{"Minus":3},"effects":[{"GainLife":{"amount":{"Stat":{"of":"This","stat":"Loyalty"}}}}]}}"#,
        &["Ability.Loyalty", "Effect.GainLife"]
    )]
    #[case::a_loyalty_ability_that_removes_x_counters_waits_for_x(
        r#"{"Loyalty":{"cost":"MinusX","targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}"#,
        &["Ability.Loyalty", "Cost.Loyalty.X", "Effect.DealDamage"]
    )]
    #[case::power_and_mana_value_are_read(
        r#"{"Spell":{"effects":[{"If":{"condition":{"All":[{"Compare":{"left":{"Stat":{"of":"This","stat":"Power"}},"op":"Ge","right":3}},{"Matches":{"subject":"This","selector":[{"Compare":{"stat":"ManaValue","op":"Le","to":2}}]}}]},"then":[]}}]}}"#,
        &["Ability.Spell", "Effect.If"]
    )]
    #[case::a_divided_amount_of_life(
        r#"{"Spell":{"targets":[{"what":"Player","bounds":{"min":1,"max":2},"divide":2}],"effects":[{"GainLife":{"who":{"Target":0},"amount":"Divided"}}]}}"#,
        &["Ability.Spell", "Amount.Divided.Elsewhere", "Effect.GainLife"]
    )]
    #[case::an_even_share_of_cards(
        r#"{"Spell":{"targets":[{"what":"Player","bounds":{"min":1,"max":null}}],"effects":[{"Draw":{"who":{"Target":0},"count":{"EvenShare":3}}}]}}"#,
        &["Ability.Spell", "Amount.EvenShare.Elsewhere", "Effect.Draw"]
    )]
    #[case::a_divided_amount_read_by_a_selector_of_the_damage(
        r#"{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":2},"divide":2}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Count":[{"Compare":{"stat":"Power","op":"Ge","to":"Divided"}}]}}}]}}"#,
        &["Ability.Spell", "Amount.Divided.Elsewhere", "Effect.DealDamage"]
    )]
    fn a_value_the_engine_cannot_read_yet_counts_as_a_primitive(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(problems(json), Vec::<String>::new());
        let ability: Ability = serde_json::from_str(json).unwrap();
        let expected: BTreeSet<String> = expected.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(used_primitives(&ability), expected);
    }

    #[rstest]
    #[case::divided_damage(
        r#"{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":2},"divide":2}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
        &["Ability.Spell", "Effect.DealDamage"]
    )]
    #[case::damage_divided_evenly(
        r#"{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":null}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"EvenShare":"X"}}}]}}"#,
        &["Ability.Spell", "Effect.DealDamage"]
    )]
    #[case::divided_counters(
        r#"{"Spell":{"targets":[{"what":{"Object":[]},"bounds":{"min":1,"max":3},"divide":3}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":"Divided"}}]}}"#,
        &["Ability.Spell", "Effect.PutCounters"]
    )]
    #[case::a_share_in_a_sum(
        r#"{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":2},"divide":2}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Sum":["Divided",1]}}}]}}"#,
        &["Ability.Spell", "Effect.DealDamage"]
    )]
    fn a_divided_amount_of_damage_or_counters_is_no_primitive_of_its_own(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        let ability: Ability = serde_json::from_str(json).unwrap();
        let expected: BTreeSet<String> = expected.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(used_primitives(&ability), expected);
    }

    #[rstest]
    #[case::being_kicked(
        r#"{"Spell":{"effects":[{"If":{"condition":"Kicked","then":[{"Draw":{}}]}}]}}"#,
        &["Ability.Spell", "Condition.Kicked", "Effect.Draw", "Effect.If"]
    )]
    #[case::being_kicked_with_one_cost(
        r#"{"Spell":{"effects":[{"If":{"condition":{"KickedWith":"{W}"},"then":[{"Draw":{}}]}}]}}"#,
        &["Ability.Spell", "Condition.KickedWith", "Effect.Draw", "Effect.If"]
    )]
    #[case::the_number_of_kicks(
        r#"{"Spell":{"effects":[{"Draw":{"count":"Kicks"}}]}}"#,
        &["Ability.Spell", "Amount.Kicks", "Effect.Draw"]
    )]
    fn being_kicked_counts_as_a_primitive_beside_the_keyword(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        let ability: Ability = serde_json::from_str(json).unwrap();
        let expected: BTreeSet<String> = expected.iter().map(|name| (*name).to_owned()).collect();
        assert_eq!(used_primitives(&ability), expected);
    }

    #[rstest]
    #[case::target_out_of_range(
        r#"{"Spell":{"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#,
        "faces[0].abilities[0].Spell.effects[0].DealDamage.to: target 0 does not exist"
    )]
    #[case::indices_attach_to_the_name(
        r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"ControlledBy":{"Target":0}}]}}}]}}"#,
        "faces[0].abilities[0].Spell.effects[0].Destroy.what[0]: target 0 does not exist"
    )]
    #[case::target_unused(
        r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Draw":{}}]}}"#,
        "faces[0].abilities[0].Spell.targets[0]: target 0 is never used"
    )]
    #[case::unbound(
        r#"{"Spell":{"effects":[{"Move":{"what":{"Bound":"ghost"},"to":"Hand"}}]}}"#,
        "`ghost` is not bound by an earlier step"
    )]
    #[case::bound_in_a_branch_is_not_visible_after(
        r#"{"Spell":{"effects":[{"May":{"effects":[{"LookAtTop":{"as":"top"}}]}},{"Reveal":{"what":{"Bound":"top"}}}]}}"#,
        "`top` is not bound by an earlier step"
    )]
    #[case::bound_twice(
        r#"{"Spell":{"effects":[{"LookAtTop":{"as":"top"}},{"LookAtTop":{"as":"top"}}]}}"#,
        "`top` is already bound"
    )]
    #[case::a_vote_with_one_choice(
        r#"{"Spell":{"effects":[{"Vote":{"options":{"Named":["only"]}}}]}}"#,
        "faces[0].abilities[0].Spell.effects[0].Vote: a vote has at least two choices"
    )]
    #[case::a_vote_on_names_that_binds_a_winner(
        r#"{"Spell":{"effects":[{"Vote":{"options":{"Named":["a","b"]},"as":"won"}}]}}"#,
        "faces[0].abilities[0].Spell.effects[0].Vote: a vote on named choices binds the choices, not a winner"
    )]
    #[case::a_vote_on_objects_with_no_winner_named(
        r#"{"Spell":{"effects":[{"Vote":{"options":{"Of":{"Each":[]}}}}]}}"#,
        "faces[0].abilities[0].Spell.effects[0].Vote: a vote on players or objects names its winners"
    )]
    #[case::a_secret_vote_on_objects(
        r#"{"Spell":{"effects":[{"Vote":{"options":{"Of":{"Each":[]}},"secret":true,"as":"won"}}]}}"#,
        "faces[0].abilities[0].Spell.effects[0].Vote: only a vote on named choices is secret"
    )]
    #[case::a_vote_name_bound_twice(
        r#"{"Spell":{"effects":[{"Vote":{"options":{"Named":["a","b"]}}},{"Vote":{"options":{"Named":["b","c"]}}}]}}"#,
        "`b` is already bound"
    )]
    #[case::the_name_of_a_choice_is_a_name(
        r#"{"Spell":{"effects":[{"Vote":{"options":{"Named":["Time","knowledge"]}}}]}}"#,
        "`Time` is not a name"
    )]
    #[case::bad_name(
        r#"{"Spell":{"effects":[{"LookAtTop":{"as":"Top Cards"}}]}}"#,
        "`Top Cards` is not a name"
    )]
    #[case::an_exile_of_the_top_bound_twice(
        r#"{"Spell":{"effects":[{"ExileTop":{"as":"impulse"}},{"ExileTop":{"as":"impulse"}}]}}"#,
        "`impulse` is already bound"
    )]
    #[case::an_exile_of_the_top_of_a_player_card(
        r#"{"Spell":{"effects":[{"ExileTop":{"who":{"Each":[]},"as":"impulse"}}]}}"#,
        "faces[0].abilities[0].Spell.effects[0].ExileTop.who: expected a player but this is an object"
    )]
    #[case::a_permission_for_cards_nothing_exiled_named(
        r#"{"Spell":{"effects":[{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"impulse"},"play":true}},"duration":"UntilEndOfTurn"}}]}}"#,
        "faces[0].abilities[0].Spell.effects[0].Rule.rule.MayCast.only: `impulse` is not bound by an earlier step"
    )]
    #[case::a_permission_for_a_player(
        r#"{"Spell":{"effects":[{"Rule":{"rule":{"MayCast":{"from":"Exile","only":"You","play":true}},"duration":"UntilEndOfTurn"}}]}}"#,
        "faces[0].abilities[0].Spell.effects[0].Rule.rule.MayCast.only: expected an object but this is a player"
    )]
    #[case::a_chosen_opponent_is_a_player(
        r#"{"Spell":{"effects":[{"Choose":{"from":"Opponents","as":"foe"}},{"Destroy":{"what":{"Bound":"foe"}}}]}}"#,
        "faces[0].abilities[0].Spell.effects[1].Destroy.what: expected an object but this is a player"
    )]
    #[case::triggering_outside_a_trigger(
        r#"{"Spell":{"effects":[{"Draw":{"who":"Triggering"}}]}}"#,
        "`Triggering` is only valid in a triggered ability"
    )]
    #[case::player_where_object(
        r#"{"Spell":{"effects":[{"Destroy":{"what":"You"}}]}}"#,
        "expected an object but this is a player"
    )]
    #[case::object_where_player(
        r#"{"Spell":{"effects":[{"Draw":{"who":"This"}}]}}"#,
        "expected a player but this is an object"
    )]
    #[case::attaching_a_player(
        r#"{"Spell":{"effects":[{"Attach":{"what":"You","to":"This"}}]}}"#,
        "abilities[0].Spell.effects[0].Attach.what: expected an object but this is a player"
    )]
    #[case::attaching_to_a_player(
        r#"{"Spell":{"effects":[{"Attach":{"what":"This","to":"You"}}]}}"#,
        "abilities[0].Spell.effects[0].Attach.to: expected an object but this is a player"
    )]
    #[case::unknown_token(
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"blood"}}}]}}"#,
        "unknown token `blood`"
    )]
    #[case::divided_without_division(
        r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
        "`Divided` needs a target specification with `divide`"
    )]
    #[case::kicked_without_kicker(
        r#"{"Spell":{"effects":[{"If":{"condition":"Kicked","then":[{"Draw":{}}]}}]}}"#,
        "`Kicked` needs a Kicker"
    )]
    #[case::kicked_with_a_cost_no_kicker_has(
        r#"{"Spell":{"effects":[{"If":{"condition":{"KickedWith":"{W}"},"then":[{"Draw":{}}]}}]}}"#,
        "`KickedWith` needs a Kicker keyword with that cost"
    )]
    #[case::kicks_without_kicker(
        r#"{"Spell":{"effects":[{"Draw":{"count":"Kicks"}}]}}"#,
        "`Kicks` needs a Kicker"
    )]
    #[case::modal_not_alone(
        r#"{"Spell":{"effects":[{"Draw":{}},{"Modal":{"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"Draw":{}}]}]}}]}}"#,
        "`Modal` must be the only effect"
    )]
    #[case::a_pick_at_random_of_up_to_one(
        r#"{"Spell":{"effects":[{"Modal":{"at_random":true,"choose":{"min":0,"max":1},"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"Draw":{}}]}]}}]}}"#,
        "chooses a fixed number of modes, none twice"
    )]
    #[case::a_pick_at_random_that_repeats(
        r#"{"Spell":{"effects":[{"Modal":{"at_random":true,"repeat":true,"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"Draw":{}}]}]}}]}}"#,
        "chooses a fixed number of modes, none twice"
    )]
    #[case::a_fresh_mode_that_repeats(
        r#"{"Spell":{"effects":[{"Modal":{"once_each_turn":true,"repeat":true,"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"Draw":{}}]}]}}]}}"#,
        "cannot repeat"
    )]
    #[case::c11d_a_mode_once_ever_that_repeats(
        r#"{"Spell":{"effects":[{"Modal":{"once_ever":true,"repeat":true,"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"Draw":{}}]}]}}]}}"#,
        "cannot repeat or be chosen once each turn"
    )]
    #[case::empty_spell(r#"{"Spell":{"effects":[]}}"#, "needs at least one effect")]
    #[case::empty_clause_reason(
        r#"{"Manual":{"reason":" ","span":{"start":1,"end":4}}}"#,
        "needs a reason"
    )]
    #[case::backwards_span(
        r#"{"Manual":{"reason":"x","span":{"start":4,"end":4}}}"#,
        "the span is empty or backwards"
    )]
    #[case::any_of_alone(
        r#"{"Triggered":{"when":{"AnyOf":[{"Attacks":{"what":["This"]}}]},"effects":[{"Draw":{}}]}}"#,
        "`AnyOf` needs at least two patterns"
    )]
    fn each_rule_names_its_path(#[case] ability: &str, #[case] expected: &str) {
        let found = problems(ability);
        assert!(
            found.iter().any(|p| p.contains(expected)),
            "wanted {expected:?} in {found:#?}"
        );
    }

    #[rstest]
    #[case::bad_uuid("oracle_id", "\"nope\"", "not a lowercase UUID")]
    #[case::name("name", "\"Other\"", "the name is the face names joined")]
    fn card_level_rules(#[case] field: &str, #[case] value: &str, #[case] expected: &str) {
        let mut json: serde_json::Value = serde_json::to_value(card("")).unwrap();
        json[field] = serde_json::from_str(value).unwrap();
        let bad: Card = serde_json::from_value(json).unwrap();
        let found = check_card(&bad, &BTreeSet::new());
        assert!(
            found.iter().any(|p| p.message.contains(expected)),
            "{found:#?}"
        );
    }

    #[rstest]
    #[case::full_with_clause(
        "Full",
        r#"{"Manual":{"reason":"x","span":{"start":0,"end":3}}}"#,
        "Full cards have no clauses"
    )]
    #[case::partial_without_clause(
        "Partial",
        r#"{"Keyword":"Flying"}"#,
        "Partial cards have at least one clause"
    )]
    #[case::manual_with_abilities(
        "Manual",
        r#"{"Keyword":"Flying"}"#,
        "Manual cards have no abilities"
    )]
    fn status_matches_the_abilities(
        #[case] status: &str,
        #[case] ability: &str,
        #[case] expected: &str,
    ) {
        let mut bad = card(ability);
        bad.impl_status = serde_json::from_str(&format!("\"{status}\"")).unwrap();
        let found = check_card(&bad, &BTreeSet::new());
        assert!(
            found.iter().any(|p| p.message.contains(expected)),
            "{found:#?}"
        );
    }

    #[test]
    fn overlapping_clauses_are_refused() {
        let found = problems(
            r#"{"Manual":{"reason":"a","span":{"start":0,"end":5}}},{"Manual":{"reason":"b","span":{"start":4,"end":9}}}"#,
        );
        assert!(
            found
                .iter()
                .any(|p| p.contains("overlaps an earlier clause")),
            "{found:#?}"
        );
    }

    #[test]
    fn a_missing_fingerprint_is_refused_for_live_rules_only() {
        let mut bad = card("");
        bad.faces[0].oracle_fingerprint = String::new();
        assert!(
            check_card(&bad, &BTreeSet::new())
                .iter()
                .any(|p| p.message.contains("fingerprint"))
        );
        bad.impl_status = ImplStatus::Manual;
        assert_eq!(check_card(&bad, &BTreeSet::new()), Vec::<Problem>::new());
    }

    /// A replacement whose steps read the event it replaces: `when`, `Instead` steps, the problems.
    #[rstest]
    #[case::that_player_loses_that_much(
        r#"{"LifeGain":{"who":"Opponents"}}"#,
        r#"[{"LoseLife":{"who":"Triggering","amount":"Triggering"}}]"#,
        &[]
    )]
    #[case::the_source_deals_it_elsewhere(
        r#"{"Damage":{"to":"You"}}"#,
        r#"[{"DealDamage":{"source":"Dealer","to":"Attached","amount":"Triggering"}}]"#,
        &[]
    )]
    #[case::that_card_is_exiled(
        r#"{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}}"#,
        r#"[{"Exile":{"what":"Triggering"}}]"#,
        &[]
    )]
    #[case::no_source_but_of_damage(
        r#"{"LifeGain":{"who":"Opponents"}}"#,
        r#"[{"DealDamage":{"source":"Dealer","to":"Triggering","amount":"Triggering"}}]"#,
        &["faces[0].abilities[0].Replacement.by.Instead[0].DealDamage.source: `Dealer` is the source of damage: only a `Damage` replacement has one"]
    )]
    fn w5d_a_replacement_reads_the_event_it_replaces(
        #[case] when: &str,
        #[case] steps: &str,
        #[case] expected: &[&str],
    ) {
        let json = format!(r#"{{"Replacement":{{"when":{when},"by":{{"Instead":{steps}}}}}}}"#);
        assert_eq!(problems(&json), expected, "{json}");
    }

    #[rstest]
    #[case::a_draw(r#"[{"Draw":{}}]"#, false)]
    #[case::a_choice(r#"[{"May":{"effects":[{"Draw":{}}]}}]"#, true)]
    #[case::tap_or_untap(r#"[{"TapOrUntap":{"what":"Triggering"}}]"#, true)]
    fn w5d_steps_that_ask_are_named_apart(#[case] steps: &str, #[case] asks: bool) {
        let json = format!(
            r#"{{"Replacement":{{"when":{{"LifeGain":{{}}}},"by":{{"Instead":{steps}}}}}}}"#
        );
        let ability: Ability = serde_json::from_str(&json).unwrap();
        assert_eq!(
            used_primitives(&ability).contains("ReplaceBy.Instead.Asks"),
            asks
        );
    }

    #[test]
    fn w5d_outside_a_replacement_the_event_is_still_unnamed() {
        let found =
            problems(r#"{"Spell":{"effects":[{"LoseLife":{"who":"Triggering","amount":1}}]}}"#);
        assert!(
            found
                .iter()
                .any(|p| p.contains("`Triggering` is only valid in a triggered ability")),
            "{found:?}"
        );
    }
}

#[cfg(test)]
mod printed_keywords {
    //! Goal: a Scryfall keyword counts as present when an ability has it (a "Partner with" card has `Partner` too), an ability grants it (a static `AddKeyword`, a token's keyword) or a `Manual` clause names it at the end of a word in its face's printed text (`Mountainwalk` names `Landwalk`), and as missing otherwise.
    use super::*;
    use rstest::rstest;

    const ID: &str = "4457ed35-7c10-48c8-9776-456485fdf070";
    const BIRD: &str = "Creature \u{2014} Bird";
    const AURA: &str = "Enchantment \u{2014} Aura";
    /// The one other imported card: the partner a "Partner with" card names.
    const PARTNER: &str = "Pir, Imaginative Rascal";

    /// Every problem of a rules file against its imported card, one `(type line, oracle text, abilities)` per face.
    fn problems(status: &str, faces: &[(&str, &str, &str)], keywords: &[&str]) -> Vec<String> {
        let names: Vec<String> = (0..faces.len()).map(|i| format!("Face {i}")).collect();
        let name = names.join(" // ");
        let rules: Vec<String> = faces
            .iter()
            .zip(&names)
            .map(|((_, text, abilities), face)| {
                let fingerprint = oracle_fingerprint(text);
                format!(
                    r#"{{"name":"{face}","oracle_fingerprint":"{fingerprint}","abilities":[{abilities}]}}"#
                )
            })
            .collect();
        let json = format!(
            r#"{{"oracle_id":"{ID}","name":"{name}","impl_status":"{status}","faces":[{}]}}"#,
            rules.join(",")
        );
        let card: Card = serde_json::from_str(&json).unwrap();
        let shown = PrintedCard {
            layout: String::new(),
            oracle_id: ID.into(),
            name,
            keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
            faces: faces
                .iter()
                .zip(&names)
                .map(|((type_line, text, _), face)| PrintedFace {
                    name: face.clone(),
                    type_line: (*type_line).into(),
                    oracle_text: (*text).into(),
                    ..PrintedFace::default()
                })
                .collect(),
        };
        let known = BTreeSet::from([PARTNER.to_lowercase()]);
        check_printed(&card, &shown, &known, &BTreeSet::new())
            .into_iter()
            .map(|problem| problem.message)
            .collect()
    }

    #[rstest]
    #[case::a_clause_names_the_keyword(
        "Partial",
        &[(BIRD, "Flying", r#"{"Manual":{"reason":"x","span":{"start":0,"end":6}}}"#)],
        &["Flying"],
        &[]
    )]
    #[case::the_clause_is_longer_than_the_keyword(
        "Partial",
        &[(BIRD, "Flying\nDraw a card.", r#"{"Manual":{"reason":"x","span":{"start":0,"end":19}}}"#)],
        &["Flying"],
        &[]
    )]
    #[case::capitals_differ_from_scryfalls(
        "Partial",
        &[(BIRD, "Gets first strike.", r#"{"Manual":{"reason":"x","span":{"start":0,"end":18}}}"#)],
        &["First strike"],
        &[]
    )]
    #[case::the_keyword_is_named_nowhere(
        "Partial",
        &[(BIRD, "Draw a card.", r#"{"Manual":{"reason":"x","span":{"start":0,"end":12}}}"#)],
        &["Flying"],
        &["Flying"]
    )]
    #[case::a_full_card_has_no_clause_to_name_it(
        "Full",
        &[(BIRD, "Flying", r#"{"Keyword":"Reach"}"#)],
        &["Flying"],
        &["Flying"]
    )]
    #[case::a_full_card_with_the_ability_passes(
        "Full",
        &[(BIRD, "Flying", r#"{"Keyword":"Flying"}"#)],
        &["Flying"],
        &[]
    )]
    #[case::plainscycling_is_cycling(
        "Partial",
        &[(BIRD, "Plainscycling {2}", r#"{"Manual":{"reason":"x","span":{"start":0,"end":17}}}"#)],
        &["Plainscycling", "Cycling"],
        &[]
    )]
    #[case::plainscycling_is_typecycling(
        "Partial",
        &[(BIRD, "Plainscycling {2}", r#"{"Manual":{"reason":"x","span":{"start":0,"end":17}}}"#)],
        &["Plainscycling", "Typecycling"],
        &[]
    )]
    #[case::a_typecycling_ability_is_cycling_and_typecycling(
        "Full",
        &[(BIRD, "Plainscycling {2}", r#"{"Keyword":{"Typecycling":{"find":[{"Subtype":"Plains"}],"cost":[{"Mana":"{2}"}]}}}"#)],
        &["Plainscycling", "Landcycling", "Typecycling", "Cycling"],
        &[]
    )]
    #[case::cycling_is_not_typecycling(
        "Partial",
        &[(BIRD, "Cycling {2}", r#"{"Manual":{"reason":"x","span":{"start":0,"end":11}}}"#)],
        &["Typecycling"],
        &["Typecycling"]
    )]
    #[case::an_aura_grants_a_keyword(
        "Partial",
        &[(
            AURA,
            "Enchant creature\nEnchanted creature has flying.",
            r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}},{"Manual":{"reason":"x","span":{"start":17,"end":47}}}"#
        )],
        &["Enchant", "Flying"],
        &[]
    )]
    #[case::the_clause_is_over_another_span(
        "Partial",
        &[(BIRD, "Flying\nDraw a card.", r#"{"Manual":{"reason":"x","span":{"start":7,"end":19}}}"#)],
        &["Flying"],
        &["Flying"]
    )]
    #[case::the_span_ends_before_the_keyword_does(
        "Partial",
        &[(BIRD, "Flying", r#"{"Manual":{"reason":"x","span":{"start":0,"end":5}}}"#)],
        &["Flying"],
        &["Flying"]
    )]
    #[case::offsets_count_characters_not_bytes(
        "Partial",
        &[(BIRD, "\u{2022} Flying", r#"{"Manual":{"reason":"x","span":{"start":2,"end":8}}}"#)],
        &["Flying"],
        &[]
    )]
    #[case::a_clause_is_read_in_its_own_face(
        "Partial",
        &[
            (BIRD, "Draw a card.", r#"{"Manual":{"reason":"x","span":{"start":0,"end":12}}}"#),
            (BIRD, "Flying", r#"{"Manual":{"reason":"x","span":{"start":0,"end":6}}}"#),
        ],
        &["Flying"],
        &[]
    )]
    #[case::a_clause_is_not_read_in_another_face(
        "Partial",
        &[
            (BIRD, "Flying", ""),
            (BIRD, "Draw a card.", r#"{"Manual":{"reason":"x","span":{"start":0,"end":6}}}"#),
        ],
        &["Flying"],
        &["Flying"]
    )]
    #[case::an_ability_on_the_other_face_counts(
        "Partial",
        &[
            (BIRD, "Flying", r#"{"Keyword":"Flying"}"#),
            (BIRD, "Draw a card.", r#"{"Manual":{"reason":"x","span":{"start":0,"end":12}}}"#),
        ],
        &["Flying"],
        &[]
    )]
    #[case::a_full_partner_with_card_has_partner(
        "Full",
        &[(BIRD, "Partner with Pir, Imaginative Rascal", r#"{"Keyword":{"PartnerWith":"Pir, Imaginative Rascal"}}"#)],
        &["Partner with", "Partner"],
        &[]
    )]
    #[case::a_partial_partner_with_card_has_partner(
        "Partial",
        &[(
            BIRD,
            "Partner with Pir, Imaginative Rascal\nHaste",
            r#"{"Keyword":{"PartnerWith":"Pir, Imaginative Rascal"}},{"Manual":{"reason":"x","span":{"start":37,"end":42}}}"#
        )],
        &["Partner with", "Partner"],
        &[]
    )]
    #[case::partner_is_not_partner_with(
        "Full",
        &[(BIRD, "Partner", r#"{"Keyword":"Partner"}"#)],
        &["Partner with"],
        &["Partner with"]
    )]
    #[case::a_grant_to_the_enchanted_creature_has_the_keyword(
        "Full",
        &[(
            AURA,
            "Enchant creature\nEnchanted creature has flying, first strike, and vigilance.",
            r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}},{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Flying"},{"AddKeyword":"FirstStrike"},{"AddKeyword":"Vigilance"}]}}"#
        )],
        &["Enchant", "First strike"],
        &[]
    )]
    #[case::a_grant_to_other_creatures_has_the_keyword(
        "Full",
        &[(
            BIRD,
            "Other Soldiers you control have trample.",
            r#"{"Static":{"affected":{"Each":[{"Subtype":"Soldier"},"Other",{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Trample"}]}}"#
        )],
        &["Trample"],
        &[]
    )]
    #[case::a_token_with_the_keyword_has_it(
        "Full",
        &[(
            "Sorcery",
            "Create a 1/1 white Spirit creature token with flying.",
            r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Spirit","colors":["White"],"types":["Creature"],"subtypes":["Spirit"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}}}}]}}"#
        )],
        &["Flying"],
        &[]
    )]
    #[case::a_grant_of_another_keyword_is_no_grant_of_this_one(
        "Full",
        &[(
            AURA,
            "Enchant creature\nEnchanted creature has reach.",
            r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}},{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Reach"}]}}"#
        )],
        &["Enchant", "Flying"],
        &["Flying"]
    )]
    #[case::taking_a_keyword_away_is_no_grant(
        "Full",
        &[(
            AURA,
            "Enchant creature\nEnchanted creature loses flying.",
            r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}},{"Static":{"affected":"Attached","mods":[{"RemoveKeyword":"Flying"}]}}"#
        )],
        &["Enchant", "Flying"],
        &["Flying"]
    )]
    #[case::a_card_without_a_grant_or_a_clause_lacks_the_keyword(
        "Full",
        &[(BIRD, "Haste", r#"{"Keyword":"Haste"}"#)],
        &["Haste", "Trample"],
        &["Trample"]
    )]
    #[case::mountainwalk_names_landwalk(
        "Partial",
        &[(BIRD, "Flying, mountainwalk", r#"{"Manual":{"reason":"x","span":{"start":0,"end":20}}}"#)],
        &["Landwalk"],
        &[]
    )]
    #[case::a_walk_without_letters_before_it_is_no_landwalk(
        "Partial",
        &[(BIRD, "Take a walk", r#"{"Manual":{"reason":"x","span":{"start":0,"end":11}}}"#)],
        &["Landwalk"],
        &["Landwalk"]
    )]
    #[case::flashback_is_not_flash(
        "Partial",
        &[(BIRD, "Flashback {2}{R}", r#"{"Manual":{"reason":"x","span":{"start":0,"end":16}}}"#)],
        &["Flash"],
        &["Flash"]
    )]
    #[case::the_keyword_ends_at_punctuation(
        "Partial",
        &[(BIRD, "Gains flash.", r#"{"Manual":{"reason":"x","span":{"start":0,"end":12}}}"#)],
        &["Flash"],
        &[]
    )]
    #[case::the_keyword_ends_at_a_dash(
        "Partial",
        &[(BIRD, "Ward\u{2014}Pay 3 life.", r#"{"Manual":{"reason":"x","span":{"start":0,"end":16}}}"#)],
        &["Ward"],
        &[]
    )]
    #[case::the_keyword_ends_at_a_space(
        "Partial",
        &[(BIRD, "Gains flash until end of turn.", r#"{"Manual":{"reason":"x","span":{"start":0,"end":30}}}"#)],
        &["Flash"],
        &[]
    )]
    #[case::the_span_ends_where_the_keyword_does(
        "Partial",
        &[(BIRD, "Flashback {2}{R}", r#"{"Manual":{"reason":"x","span":{"start":0,"end":5}}}"#)],
        &["Flash"],
        &[]
    )]
    #[case::a_later_occurrence_counts(
        "Partial",
        &[(BIRD, "Flashback and flash", r#"{"Manual":{"reason":"x","span":{"start":0,"end":19}}}"#)],
        &["Flash"],
        &[]
    )]
    #[case::enchanted_is_not_enchant(
        "Partial",
        &[(AURA, "Enchanted creature has flying.", r#"{"Manual":{"reason":"x","span":{"start":0,"end":30}}}"#)],
        &["Enchant"],
        &["Enchant"]
    )]
    fn a_keyword_is_present_when_an_ability_has_or_grants_it_or_a_clause_names_it(
        #[case] status: &str,
        #[case] faces: &[(&str, &str, &str)],
        #[case] keywords: &[&str],
        #[case] missing: &[&str],
    ) {
        let expected: Vec<String> = missing
            .iter()
            .map(|keyword| {
                format!("the imported card has the keyword `{keyword}` but no face has it")
            })
            .collect();
        assert_eq!(problems(status, faces, keywords), expected);
    }

    #[test]
    fn a_clause_past_the_text_names_nothing() {
        let found = problems(
            "Partial",
            &[(
                BIRD,
                "Flying",
                r#"{"Manual":{"reason":"x","span":{"start":0,"end":99}}}"#,
            )],
            &["Flying"],
        );
        assert_eq!(
            found,
            [
                "the span ends past the oracle text (6 characters)",
                "the imported card has the keyword `Flying` but no face has it",
            ]
        );
    }
}
