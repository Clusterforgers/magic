//! Validation of rules data: the structure of a file by itself, and a file against the imported card.
//!
//! Every problem carries the path of the value it is about, in the form serde uses for parse errors
//! (`faces[0].abilities[1].Triggered.effects[2].If.then[0]`).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::ability::Ability;
use crate::assemble::{PLAIN_MANA_ABILITY, plain_mana_ability, simple_mana_ability};
use crate::card::{Card, Face, ImplStatus, TokenSpec};
use crate::cost::{Cost, CostItem};
use crate::effect::{Destination, Effect, Mode, Order, Place, Produce, TokenRef};
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable};
use crate::support::{PrintedCard, PrintedFace, oracle_fingerprint};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::{Amount, Condition, StatKind};

/// One problem found in a file: where, and what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The path inside the file; empty for the file as a whole.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

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
        Targetable::Object(_) => Kind::Object,
        Targetable::AnyTarget | Targetable::Either(_) => Kind::Either,
    }
}

/// The target specifications and bound names an ability body can see.
#[derive(Default)]
struct Scope {
    targets: Vec<(Kind, bool, bool)>,
    used: BTreeSet<usize>,
    in_trigger: bool,
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

struct Walker<'a> {
    tokens: &'a BTreeSet<String>,
    problems: Vec<Problem>,
    uses: BTreeSet<String>,
    path: Vec<String>,
    scope: Scope,
    has_kicker: bool,
    x_free: bool,
    face: usize,
    spell_targets: Vec<TargetSpec>,
    x_uses: Vec<(usize, String)>,
    names: Vec<(usize, String, String)>,
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
            x_free: false,
            face: 0,
            spell_targets: Vec::new(),
            x_uses: Vec::new(),
            names: Vec::new(),
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
            Ref::You | Ref::Opponents | Ref::EachPlayer => self.expect(Kind::Player, want),
            Ref::This | Ref::Attached => self.expect(Kind::Object, want),
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
            Pred::ControlledBy(r) | Pred::OwnedBy(r) => self.reference(r, Kind::Player),
            Pred::Is(r) => self.reference(r, Kind::Object),
            Pred::HasKeyword(keyword) => self.keyword(keyword),
            Pred::Named(name) => self
                .names
                .push((self.face, self.current_path(), name.clone())),
            Pred::Compare { stat, to, .. } => {
                self.stat(*stat);
                self.at("to", |w| w.amount(to));
            }
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
            | Pred::Spell
            | Pred::Tapped
            | Pred::Untapped
            | Pred::Attacking
            | Pred::Blocking
            | Pred::In(_) => {}
        }
    }

    /// A characteristic the value evaluator reads. It cannot read loyalty yet, and a card that needs it waits.
    fn stat(&mut self, stat: StatKind) {
        if stat == StatKind::Loyalty {
            self.uses("Stat", "Loyalty");
        }
    }

    fn amount(&mut self, amount: &Amount) {
        match amount {
            Amount::Fixed(_) => {}
            Amount::X => {
                if !self.x_free {
                    self.x_uses.push((self.face, self.current_path()));
                }
            }
            Amount::Count(selector) => self.selector(selector),
            Amount::Stat { of, stat } => {
                self.stat(*stat);
                self.at("of", |w| w.reference(of, Kind::Object));
            }
            Amount::Life(who) => self.reference(who, Kind::Player),
            // The value evaluator cannot read counters or divided amounts yet, and a card that needs them waits.
            Amount::Counters { on, .. } => {
                self.uses("Amount", "Counters");
                self.at("on", |w| w.reference(on, Kind::Either));
            }
            Amount::Sum(parts) => {
                for (i, part) in parts.iter().enumerate() {
                    self.at(format!("[{i}]"), |w| w.amount(part));
                }
            }
            Amount::Divided => {
                self.uses("Amount", "Divided");
                if !self.scope.targets.iter().any(|t| t.1) {
                    self.problem("`Divided` needs a target specification with `divide`");
                }
            }
            Amount::EvenShare(total) => {
                self.uses("Amount", "EvenShare");
                if !self.scope.targets.iter().any(|t| t.2) {
                    self.problem(
                        "`EvenShare` needs a target specification with more than one target",
                    );
                }
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
        }
    }

    fn bounds(&mut self, bounds: &Bounds) {
        if let Some(Amount::Fixed(max)) = &bounds.max
            && i64::from(bounds.min) > i64::from(*max)
        {
            self.problem(format!(
                "the minimum {} is above the maximum {max}",
                bounds.min
            ));
        }
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
            Condition::YourTurn => {}
        }
    }

    fn cost(&mut self, cost: &Cost) {
        for (i, item) in cost.0.iter().enumerate() {
            self.at(format!("[{i}]"), |w| {
                w.uses("Cost", &tag_of(item));
                match item {
                    CostItem::PayLife(amount) => w.amount(amount),
                    CostItem::Sacrifice { filter, count } => {
                        w.at("filter", |w| w.selector(filter));
                        w.at("count", |w| w.amount(count));
                    }
                    CostItem::Discard { count, filter } => {
                        w.at("count", |w| w.amount(count));
                        if let Some(filter) = filter {
                            w.at("filter", |w| w.selector(filter));
                        }
                    }
                    CostItem::Mana(_)
                    | CostItem::Tap
                    | CostItem::Untap
                    | CostItem::SacrificeSelf => {}
                }
            });
        }
    }

    fn keyword(&mut self, keyword: &Keyword) {
        self.uses("Keyword", &tag_of(keyword));
        match keyword {
            Keyword::Ward(cost) => {
                self.at("cost", |w| w.cost(cost));
                // Ward asks the player for its cost through a payment, which no effect makes by sacrificing the source.
                if cost.0.contains(&CostItem::SacrificeSelf) {
                    self.uses("Effect", "Pay.SacrificeSelf");
                }
            }
            Keyword::Equip(cost)
            | Keyword::Reconfigure(cost)
            | Keyword::Cycling(cost)
            | Keyword::Kicker(cost)
            | Keyword::Multikicker(cost)
            | Keyword::Flashback(cost)
            | Keyword::Madness(cost)
            | Keyword::Morph(cost)
            | Keyword::Disguise(cost)
            | Keyword::Evoke(cost)
            | Keyword::Overload(cost)
            | Keyword::Suspend { cost, .. } => self.at("cost", |w| w.cost(cost)),
            Keyword::Enchant(selector) => self.selector(selector),
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
                self.keyword(keyword);
            }
            Modification::SetController(who) => self.reference(who, Kind::Player),
            Modification::GrantAbility(granted) => self.granted(granted),
            Modification::SwitchPT
            | Modification::AddType(_)
            | Modification::RemoveType(_)
            | Modification::AddSubtype(_)
            | Modification::SetColors(_)
            | Modification::LoseAbilities
            | Modification::CantAttack
            | Modification::CantBlock => {}
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
        self.at("GrantAbility", |w| w.ability(granted));
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
            self.at(format!("[{i}]"), |w| w.modification(modification));
        }
    }

    fn trigger_pattern(&mut self, pattern: &TriggerPattern) {
        self.uses("Trigger", &tag_of(pattern));
        match pattern {
            TriggerPattern::ZoneChange { what, .. } | TriggerPattern::Attacks { what } => {
                self.at("what", |w| w.selector(what));
            }
            TriggerPattern::BecomesTarget { what, by } => {
                self.at("what", |w| w.selector(what));
                self.at("by", |w| w.selector(by));
            }
            TriggerPattern::SpellCast { what, by } | TriggerPattern::Sacrificed { what, by } => {
                self.at("what", |w| w.selector(what));
                self.at("by", |w| w.reference(by, Kind::Player));
            }
            TriggerPattern::StepBegins { whose, .. } => {
                self.at("whose", |w| w.reference(whose, Kind::Player));
            }
            TriggerPattern::AnyOf(patterns) => {
                if patterns.len() < 2 {
                    self.problem("`AnyOf` needs at least two patterns");
                }
                for (i, inner) in patterns.iter().enumerate() {
                    self.at(format!("[{i}]"), |w| w.trigger_pattern(inner));
                }
            }
        }
    }

    fn target_spec(&mut self, spec: &TargetSpec) {
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
            }
            TokenRef::Custom(spec) => self.at("Custom", |w| w.token_spec(spec)),
            TokenRef::CopyOf { of, except } => {
                self.at("of", |w| w.reference(of, Kind::Object));
                for (i, modification) in except.iter().enumerate() {
                    self.at(format!("except[{i}]"), |w| w.modification(modification));
                }
            }
        }
    }

    fn token_spec(&mut self, spec: &TokenSpec) {
        if spec.name.is_empty() {
            self.problem("a token needs a name");
        }
        if spec.types.is_empty() {
            self.problem("a token needs at least one card type");
        }
        let creature = spec.types.contains(&mtg_core::card::CardType::Creature);
        if creature != (spec.power.is_some() && spec.toughness.is_some()) {
            self.problem("a creature token needs power and toughness, any other token none");
        }
        let saved = std::mem::take(&mut self.scope);
        for (i, ability) in spec.abilities.iter().enumerate() {
            self.at(format!("abilities[{i}]"), |w| w.ability(ability));
        }
        self.scope = saved;
    }

    fn effect_list(&mut self, field: &str, list: &[Effect], top: bool) {
        self.scope.names.push(BTreeMap::new());
        for (i, effect) in list.iter().enumerate() {
            let only = top && list.len() == 1;
            self.at(format!("{field}[{i}]"), |w| w.effect(effect, only));
        }
        self.scope.names.pop();
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
        match effect {
            Effect::Move {
                to: Destination::Battlefield { .. },
                ..
            } => self.uses("Effect", "Move.Battlefield"),
            Effect::Move {
                to:
                    Destination::Library {
                        place: Place::Top | Place::Bottom,
                        order: Some(Order::Random),
                    },
                ..
            } => self.uses("Effect", "Move.Random"),
            _ => {}
        }
        self.at(tag, |w| match effect {
            Effect::Draw { who, count }
            | Effect::Mill { who, count }
            | Effect::Scry { who, count }
            | Effect::Discard { who, count, .. } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("count", |w| w.amount(count));
            }
            Effect::LookAtTop { who, count, bind } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("count", |w| w.amount(count));
                w.at("as", |w| w.bind(bind, Kind::Object));
            }
            Effect::Reveal { what }
            | Effect::Exile { what }
            | Effect::Counter { what }
            | Effect::Tap { what }
            | Effect::Untap { what }
            | Effect::Transform { what }
            | Effect::Destroy { what, .. } => w.at("what", |w| w.reference(what, Kind::Object)),
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
            Effect::Choose {
                who,
                from,
                count,
                bind,
                rest_as,
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("from", |w| w.reference(from, Kind::Object));
                w.at("count", |w| w.bounds(count));
                w.at("as", |w| w.bind(bind, Kind::Object));
                if let Some(rest) = rest_as {
                    w.at("rest_as", |w| w.bind(rest, Kind::Object));
                }
            }
            Effect::Shuffle { who } | Effect::WinGame { who } => {
                w.at("who", |w| w.reference(who, Kind::Player));
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
                w.at("amount", |w| w.amount(amount));
            }
            Effect::GainLife { who, amount } | Effect::LoseLife { who, amount } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("amount", |w| w.amount(amount));
            }
            Effect::AddMana { who, mana, times } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                match mana {
                    Produce::Fixed(list) if list.is_empty() => w.problem("adds no mana"),
                    Produce::AnyColor(0) => w.problem("adds no mana"),
                    _ => {}
                }
                w.at("times", |w| w.amount(times));
            }
            Effect::PutCounters { on, count, .. } => {
                w.at("on", |w| w.reference(on, Kind::Either));
                w.at("count", |w| w.amount(count));
            }
            Effect::CreateToken {
                who, token, count, ..
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("token", |w| w.token(token));
                w.at("count", |w| w.amount(count));
                // A token that copies an object waits for the copy effects.
                if matches!(token, TokenRef::CopyOf { .. }) {
                    w.uses("Effect", "CreateToken.Copy");
                }
            }
            Effect::Continuous { affected, mods, .. } => {
                w.at("affected", |w| w.reference(affected, Kind::Object));
                w.at("mods", |w| w.mods(mods));
            }
            Effect::Rule { .. } => {}
            Effect::DelayedTrigger { when, effects, .. } => {
                w.at("when", |w| w.trigger_pattern(when));
                let saved = std::mem::replace(&mut w.scope.in_trigger, true);
                w.effect_list("effects", effects, false);
                w.scope.in_trigger = saved;
            }
            Effect::If {
                condition,
                then,
                otherwise,
            } => {
                w.at("condition", |w| w.condition(condition));
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
                paid,
                unpaid,
            } => {
                w.at("who", |w| w.reference(who, Kind::Player));
                w.at("cost", |w| w.cost(cost));
                // Sacrificing the source is paid as part of an ability's cost; no effect asks for it yet.
                if cost.0.contains(&CostItem::SacrificeSelf) {
                    w.uses("Effect", "Pay.SacrificeSelf");
                }
                w.effect_list("paid", paid, false);
                w.effect_list("unpaid", unpaid, false);
            }
            Effect::Modal {
                choose,
                modes,
                repeat,
            } => w.modal(choose, modes, *repeat, only_effect),
        });
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

    /// How often and from where an ability works are rules of their own, so an engine that has not got
    /// them does not run the ability, whatever else it runs.
    fn where_and_how_often(&mut self, ability: &Ability) {
        let (limit, from) = match ability {
            Ability::Activated { limit, from, .. } | Ability::Triggered { limit, from, .. } => {
                (limit.is_some(), !from.is_battlefield())
            }
            Ability::Static { from, .. } | Ability::Replacement { from, .. } => {
                (false, !from.is_battlefield())
            }
            _ => (false, false),
        };
        if limit {
            self.uses("Ability", "Limit");
        }
        if from {
            self.uses("Ability", "From");
        }
    }

    fn ability(&mut self, ability: &Ability) {
        let tag = tag_of(ability);
        if !matches!(ability, Ability::Keyword(_)) {
            self.uses("Ability", &tag);
        }
        // The engine activates a mana ability of one mana at priority; any other waits for its own task.
        if ability.is_mana_ability() && simple_mana_ability(ability).is_none() {
            self.uses("Ability", "ManaAbility.Complex");
        }
        self.where_and_how_often(ability);
        let saved_x = self.x_free;
        self.at(tag, |w| match ability {
            Ability::Spell { targets, effects }
            | Ability::Loyalty {
                targets, effects, ..
            } => {
                w.body(targets, effects, false);
            }
            Ability::Activated {
                cost,
                targets,
                effects,
                ..
            } => {
                w.at("cost", |w| w.cost(cost));
                w.x_free = cost.has_x();
                w.body(targets, effects, false);
            }
            Ability::Triggered {
                when,
                intervening_if,
                targets,
                effects,
                ..
            } => {
                w.scope = Scope::new(&[], true);
                w.at("when", |w| w.trigger_pattern(when));
                if let Some(condition) = intervening_if {
                    w.at("if", |w| w.condition(condition));
                }
                w.body(targets, effects, true);
            }
            Ability::Static {
                affected,
                mods,
                condition,
                defining,
                from,
            } => {
                w.scope = Scope::new(&[], false);
                w.at("affected", |w| w.reference(affected, Kind::Object));
                w.at("mods", |w| w.mods(mods));
                if let Some(condition) = condition {
                    w.at("condition", |w| w.condition(condition));
                }
                if *defining {
                    w.defining(affected, mods, condition.is_some(), *from);
                }
            }
            Ability::Replacement {
                when,
                by,
                condition,
                ..
            } => {
                w.scope = Scope::new(&[], false);
                w.at("when", |w| w.replace_when(when));
                w.at("by", |w| w.replace_by(by));
                if let Some(condition) = condition {
                    w.at("condition", |w| w.condition(condition));
                }
            }
            Ability::CostModifier {
                applies_to,
                per,
                condition,
                ..
            } => {
                let specs = w.spell_targets.clone();
                w.scope = Scope::new(&specs, false);
                w.at("applies_to", |w| w.selector(applies_to));
                w.at("per", |w| w.amount(per));
                if let Some(condition) = condition {
                    w.at("condition", |w| w.condition(condition));
                }
            }
            Ability::Keyword(keyword) => w.keyword(keyword),
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
    }

    fn replace_when(&mut self, when: &ReplaceWhen) {
        self.uses("ReplaceWhen", &tag_of(when));
        match when {
            ReplaceWhen::TokenCreation { controller } => {
                self.at("controller", |w| w.reference(controller, Kind::Player));
            }
            ReplaceWhen::Damage { to, .. } => self.at("to", |w| w.reference(to, Kind::Either)),
            ReplaceWhen::ZoneChange { what, .. } => self.at("what", |w| w.selector(what)),
        }
    }

    fn replace_by(&mut self, by: &ReplaceBy) {
        self.uses("ReplaceBy", &tag_of(by));
        match by {
            ReplaceBy::Multiply(0) => self.problem("multiplying by zero is `Prevent`"),
            ReplaceBy::Add(0) => self.problem("adding nothing changes nothing"),
            ReplaceBy::Instead(effects) => {
                if effects.is_empty() {
                    self.problem("needs at least one effect");
                }
                self.effect_list("Instead", effects, false);
            }
            _ => {}
        }
    }

    fn face(&mut self, index: usize, face: &Face) {
        self.face = index;
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

/// Every primitive an ability uses, named `Effect.DealDamage`, `Keyword.Flying`, `Trigger.ZoneChange`,
/// `Cost.Tap`, `Mod.ModifyPT`, `ReplaceWhen.TokenCreation`, `ReplaceBy.Multiply` or `Ability.Loyalty`;
/// an ability with a `limit` or a zone other than the battlefield also uses `Ability.Limit` or `Ability.From`.
/// A value the engine cannot read yet is named too, wherever it stands (a condition, an amount, a selector):
/// `Amount.Counters`, `Amount.Divided`, `Amount.EvenShare`, `Stat.Loyalty` and `Condition.Kicked`.
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
    walker.has_kicker = card
        .abilities()
        .any(|(_, a)| matches!(a, Ability::Keyword(k) if k.is_kicker()));
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

/// The problems of a predefined token by itself.
pub(crate) fn check_token(id: &str, spec: &TokenSpec, tokens: &BTreeSet<String>) -> Vec<Problem> {
    let mut walker = Walker::new(tokens);
    if !is_name(id) {
        walker.at("id", |w| {
            w.problem("a token id is lowercase letters, digits and underscores");
        });
    }
    walker.at("token", |w| w.token_spec(spec));
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
    let (x_uses, names) = (checked.x_uses, checked.names);
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
            if Keyword::is_known_name(keyword) && !has_keyword(card, printed, keyword) {
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

/// Whether some face has the keyword as an ability, or a `Manual` clause names it in that face's
/// printed text (a granted keyword, a typecycling form).
fn has_keyword(card: &Card, printed: &PrintedCard, keyword: &str) -> bool {
    card.abilities().any(|(face, ability)| match ability {
        Ability::Keyword(k) => {
            k.name().eq_ignore_ascii_case(keyword)
                // Scryfall lists `Partner` as well for a "Partner with" card (CR 702.124).
                || (matches!(k, Keyword::PartnerWith(_))
                    && Keyword::Partner.name().eq_ignore_ascii_case(keyword))
        }
        Ability::Manual { span, .. } => printed
            .faces
            .get(face)
            .and_then(|shown| span.slice(&shown.oracle_text))
            .is_some_and(|clause| names_keyword(clause, keyword)),
        _ => false,
    })
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
            Ability::Keyword(Keyword::Enchant(_)) if !subtypes.contains(&"Aura") => {
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

    #[rstest]
    #[case::plain_activation(r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Cost.Tap", "Effect.Draw"])]
    #[case::at_sorcery_speed(r#"{"Activated":{"cost":["Tap"],"timing":"Sorcery","effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Cost.Tap", "Effect.Draw"])]
    #[case::once_a_turn(r#"{"Activated":{"cost":["Tap"],"limit":"OncePerTurn","effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Ability.Limit", "Cost.Tap", "Effect.Draw"])]
    #[case::from_the_graveyard(r#"{"Activated":{"cost":["Tap"],"from":"Graveyard","effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Ability.From", "Cost.Tap", "Effect.Draw"])]
    #[case::trigger_from_the_graveyard(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"limit":"OncePerTurn","from":"Graveyard","effects":[{"Draw":{}}]}}"#, &["Ability.Triggered", "Ability.Limit", "Ability.From", "Effect.Draw", "Trigger.StepBegins"])]
    #[case::a_discard_of_your_choice(r#"{"Spell":{"effects":[{"Discard":{"count":2}}]}}"#, &["Ability.Spell", "Effect.Discard"])]
    #[case::a_discard_at_random(r#"{"Spell":{"effects":[{"Discard":{"random":true}}]}}"#, &["Ability.Spell", "Effect.Discard", "Effect.Discard.Random"])]
    #[case::a_move_to_the_hand(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#, &["Ability.Spell", "Effect.Move"])]
    #[case::a_move_to_the_top_of_a_library(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":{"Library":{"place":"Top","order":"Chosen"}}}}]}}"#, &["Ability.Spell", "Effect.Move"])]
    #[case::a_move_shuffled_in(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":{"Library":{"place":"Shuffled","order":"Random"}}}}]}}"#, &["Ability.Spell", "Effect.Move"])]
    #[case::a_move_to_the_battlefield(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#, &["Ability.Spell", "Effect.Move", "Effect.Move.Battlefield"])]
    #[case::a_move_in_a_random_order(r#"{"Spell":{"effects":[{"Move":{"what":"This","to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#, &["Ability.Spell", "Effect.Move", "Effect.Move.Random"])]
    #[case::a_named_token(r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}"#, &["Ability.Spell", "Effect.CreateToken"])]
    #[case::an_inline_token_with_an_ability(r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Gem","types":["Artifact"],"abilities":[{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}]}},"count":2,"tapped":true}}]}}"#, &["Ability.Activated", "Ability.Spell", "Cost.SacrificeSelf", "Cost.Tap", "Effect.AddMana", "Effect.CreateToken"])]
    #[case::a_copy_of_an_object(r#"{"Spell":{"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"This"}}}}]}}"#, &["Ability.Spell", "Effect.CreateToken", "Effect.CreateToken.Copy"])]
    #[case::a_cost_that_sacrifices_a_permanent(r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Cost.Sacrifice", "Effect.Draw"])]
    #[case::a_cost_that_sacrifices_the_source(r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Cost.SacrificeSelf", "Effect.Draw"])]
    #[case::a_clue_pays_mana_and_sacrifices_itself(r#"{"Activated":{"cost":[{"Mana":"{2}"},"SacrificeSelf"],"effects":[{"Draw":{}}]}}"#, &["Ability.Activated", "Cost.Mana", "Cost.SacrificeSelf", "Effect.Draw"])]
    #[case::ward_paid_with_mana(r#"{"Keyword":{"Ward":"{2}"}}"#, &["Cost.Mana", "Keyword.Ward"])]
    #[case::ward_paid_by_sacrificing_the_source(r#"{"Keyword":{"Ward":["SacrificeSelf"]}}"#, &["Cost.SacrificeSelf", "Effect.Pay.SacrificeSelf", "Keyword.Ward"])]
    #[case::a_payment_that_sacrifices_the_source(r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Pay":{"cost":["SacrificeSelf"]}}]}}"#, &["Ability.Triggered", "Cost.SacrificeSelf", "Effect.Pay", "Effect.Pay.SacrificeSelf", "Trigger.StepBegins"])]
    #[case::a_treasure_like_mana_ability(r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#, &["Ability.Activated", "Cost.SacrificeSelf", "Cost.Tap", "Effect.AddMana"])]
    #[case::sol_ring(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}]}}"#, &["Ability.Activated", "Ability.ManaAbility.Complex", "Cost.Tap", "Effect.AddMana"])]
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
        &["Ability.Spell", "Amount.Counters", "Effect.Draw", "Effect.If"]
    )]
    #[case::counters_as_an_amount(
        r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Counters":{"on":"This","counter":"+1/+1"}}}}]}}"#,
        &["Ability.Spell", "Amount.Counters", "Effect.DealDamage"]
    )]
    #[case::counters_inside_a_sum_inside_a_negation(
        r#"{"Spell":{"effects":[{"If":{"condition":{"Not":{"Compare":{"left":{"Sum":[1,{"Counters":{"on":"This","counter":"+1/+1"}}]},"op":"Lt","right":3}}},"then":[]}}]}}"#,
        &["Ability.Spell", "Amount.Counters", "Effect.If"]
    )]
    #[case::loyalty_as_a_stat(
        r#"{"Spell":{"effects":[{"If":{"condition":{"Compare":{"left":{"Stat":{"of":"This","stat":"Loyalty"}},"op":"Ge","right":1}},"then":[]}}]}}"#,
        &["Ability.Spell", "Effect.If", "Stat.Loyalty"]
    )]
    #[case::loyalty_in_a_selector(
        r#"{"Spell":{"effects":[{"If":{"condition":{"Matches":{"subject":"This","selector":[{"Compare":{"stat":"Loyalty","op":"Ge","to":3}}]}},"then":[]}}]}}"#,
        &["Ability.Spell", "Effect.If", "Stat.Loyalty"]
    )]
    #[case::power_and_mana_value_are_read(
        r#"{"Spell":{"effects":[{"If":{"condition":{"All":[{"Compare":{"left":{"Stat":{"of":"This","stat":"Power"}},"op":"Ge","right":3}},{"Matches":{"subject":"This","selector":[{"Compare":{"stat":"ManaValue","op":"Le","to":2}}]}}]},"then":[]}}]}}"#,
        &["Ability.Spell", "Effect.If"]
    )]
    #[case::a_divided_amount(
        r#"{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":2},"divide":2}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
        &["Ability.Spell", "Amount.Divided", "Effect.DealDamage"]
    )]
    #[case::an_even_share(
        r#"{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":null}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"EvenShare":3}}}]}}"#,
        &["Ability.Spell", "Amount.EvenShare", "Effect.DealDamage"]
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

    #[test]
    fn being_kicked_counts_as_a_primitive_beside_the_keyword() {
        let json = r#"{"Spell":{"effects":[{"If":{"condition":"Kicked","then":[{"Draw":{}}]}}]}}"#;
        let ability: Ability = serde_json::from_str(json).unwrap();
        assert_eq!(
            used_primitives(&ability),
            [
                "Ability.Spell",
                "Condition.Kicked",
                "Effect.Draw",
                "Effect.If"
            ]
            .map(String::from)
            .into()
        );
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
    #[case::bad_name(
        r#"{"Spell":{"effects":[{"LookAtTop":{"as":"Top Cards"}}]}}"#,
        "`Top Cards` is not a name"
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
    #[case::unknown_token(
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"gold"}}}]}}"#,
        "unknown token `gold`"
    )]
    #[case::divided_without_division(
        r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
        "`Divided` needs a target specification with `divide`"
    )]
    #[case::kicked_without_kicker(
        r#"{"Spell":{"effects":[{"If":{"condition":"Kicked","then":[{"Draw":{}}]}}]}}"#,
        "`Kicked` needs a Kicker"
    )]
    #[case::modal_not_alone(
        r#"{"Spell":{"effects":[{"Draw":{}},{"Modal":{"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"Draw":{}}]}]}}]}}"#,
        "`Modal` must be the only effect"
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
}

#[cfg(test)]
mod printed_keywords {
    //! Goal: a Scryfall keyword counts as present when an ability has it (a "Partner with" card has `Partner` too) or a `Manual` clause names it at the end of a word in its face's printed text, and as missing otherwise.
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
    fn a_keyword_is_present_when_an_ability_has_it_or_a_clause_names_it(
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
