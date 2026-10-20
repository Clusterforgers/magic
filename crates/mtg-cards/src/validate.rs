//! Validation of rules data: the structure of a file by itself, and a file against the imported card.
//!
//! Every problem carries the path of the value it is about, in the form serde uses for parse errors
//! (`faces[0].abilities[1].Triggered.effects[2].If.then[0]`).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::ability::Ability;
use crate::assemble::{PLAIN_MANA_ABILITY, plain_mana_ability};
use crate::card::{Card, Face, ImplStatus, TokenSpec};
use crate::cost::{Cost, CostItem};
use crate::effect::{Destination, Effect, Mode, Order, Place, Produce, TokenRef};
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable};
use crate::support::{PrintedCard, PrintedFace, oracle_fingerprint};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::{Amount, Condition};

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
            Pred::Compare { to, .. } => self.at("to", |w| w.amount(to)),
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

    fn amount(&mut self, amount: &Amount) {
        match amount {
            Amount::Fixed(_) => {}
            Amount::X => {
                if !self.x_free {
                    self.x_uses.push((self.face, self.current_path()));
                }
            }
            Amount::Count(selector) => self.selector(selector),
            Amount::Stat { of, .. } => self.at("of", |w| w.reference(of, Kind::Object)),
            Amount::Life(who) => self.reference(who, Kind::Player),
            Amount::Counters { on, .. } => self.at("on", |w| w.reference(on, Kind::Either)),
            Amount::Sum(parts) => {
                for (i, part) in parts.iter().enumerate() {
                    self.at(format!("[{i}]"), |w| w.amount(part));
                }
            }
            Amount::Divided => {
                if !self.scope.targets.iter().any(|t| t.1) {
                    self.problem("`Divided` needs a target specification with `divide`");
                }
            }
            Amount::EvenShare(total) => {
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
            Keyword::Ward(cost)
            | Keyword::Equip(cost)
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
            _ => {}
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
                ..
            } => {
                w.scope = Scope::new(&[], false);
                w.at("affected", |w| w.reference(affected, Kind::Object));
                w.at("mods", |w| w.mods(mods));
                if let Some(condition) = condition {
                    w.at("condition", |w| w.condition(condition));
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
            let modelled = card.abilities().any(
                |(_, a)| matches!(a, Ability::Keyword(k) if k.name().eq_ignore_ascii_case(keyword)),
            );
            if Keyword::is_known_name(keyword) && !modelled {
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
    fn how_often_from_where_and_by_what_chance_an_ability_works_counts_as_a_primitive(
        #[case] json: &str,
        #[case] expected: &[&str],
    ) {
        assert_eq!(problems(json), Vec::<String>::new());
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
