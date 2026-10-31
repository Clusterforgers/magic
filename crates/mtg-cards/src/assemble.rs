//! Card assembly: the printed side from the imported record and the rules from the DSL become the
//! definition the engine plays, with the support record that says how far to trust it.
//!
//! Design record: `docs/cards-dsl.md`, "Printed side and rules". The definition is that of the front
//! face, or the whole card with its faces for the layouts the engine plays; any other card with several faces is `Manual`.

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::{Color, ColorSet};
use mtg_core::mana::{ManaCost, ManaType, ParseManaCostError};

use crate::ability::{Ability, Limit, Timing};
use crate::card::{Card, TokenSpec};
use crate::cost::CostItem;
use crate::definition::{AbilityDef, CardDefinition, Layout, ManaAbility, ManualClause};
use crate::effect::{Effect, Produce};
use crate::keyword::Keyword;
use crate::mana::ManaCostX;
use crate::refs::{Ref, Selector, ZoneKind};
use crate::spend::{SpendOn, WhenSpent};
use crate::support::{
    Caps, Clause, PrintedCard, PrintedFace, Support, SupportRecord, support, whole_text,
};
use crate::validate::used_primitives;
use crate::value::{Amount, Condition};

/// The most ways one mana ability may make its mana (a list of `OneOfLists`, a type of `OneOf`): the engine keeps a
/// source for each way, and the validator refuses a choice of more.
pub const MANA_WAYS: usize = 6;

/// The most ways one mana ability may make its mana when it picks a combination: five mana of five types is 126. The
/// engine keeps a source for each way (`mana_ability.rs`), so an ability with more is no mana ability it runs.
pub const MANA_COMBINATIONS: usize = 128;

/// How many ways there are to pick `count` mana among `types` types (a multiset: the order of the mana is no choice).
pub fn combinations(types: usize, count: u32) -> usize {
    let (n, k) = (count as usize + types.max(1) - 1, types.max(1) - 1);
    (1..=k).fold(1_usize, |ways, step| {
        ways.saturating_mul(n - k + step) / step
    })
}

/// The one primitive a plain `{T}: Add {X}` stands for: the engine runs that exact shape, one fixed
/// mana for a tap, not the general `Ability.Activated`, `Cost.Tap` and `Effect.AddMana`.
pub const PLAIN_MANA_ABILITY: &str = "Ability.ManaAbility";
/// The engine pays `{X}` in the mana cost of a spell.
pub const X_COST: &str = "Card.XCost";
/// The engine plays the faces of a card that has more than one, for the layouts it knows (split, adventure, modal double-faced).
pub const FACES: &str = "Card.Faces";
/// The engine casts the spell face of a prepared permanent (the `prepare` layout).
pub const PREPARE: &str = "Card.Prepare";
/// The engine knows a power, toughness, loyalty or defense that is not a number (`*`).
pub const VARIABLE_STATS: &str = "Card.VariableStats";

/// The basic land types in the order of CR 305.6, each with the mana its intrinsic ability adds.
const BASIC_LAND_TYPES: [(&str, ManaType); 5] = [
    ("Plains", ManaType::White),
    ("Island", ManaType::Blue),
    ("Swamp", ManaType::Black),
    ("Mountain", ManaType::Red),
    ("Forest", ManaType::Green),
];

/// A card as the engine plays it, and how far the engine can be trusted with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assembled {
    /// The front face's printed characteristics and the abilities the engine runs.
    pub definition: CardDefinition,
    /// The state the card is in and the clauses left to the table.
    pub support: SupportRecord,
}

/// Builds the definition of one oracle card from its imported record and its rules, if it has any.
///
/// The printed side always comes from `printed`. The abilities of the rules are attached only when
/// the support record says the engine runs them (`Verified` or `Partial`), and then only those the
/// engine runs; every other card has the printed side alone. A card with no file is `Verified` when it
/// has no rules text (a vanilla creature, a basic land) and `Manual` otherwise. The mana abilities the
/// basic land types give are part of the printed side (CR 305.6), so they are always there.
pub fn assemble(printed: &PrintedCard, rules: Option<&Card>, caps: &Caps) -> Assembled {
    let record = support(rules, printed, caps);
    let attach = matches!(record.support, Support::Verified | Support::Partial);
    let face_of = |index: usize| -> CardDefinition {
        let mut definition = printed_side(&printed.faces[index]);
        definition
            .abilities
            .extend(intrinsic_abilities(&definition));
        if attach && let Some(face) = rules.and_then(|card| card.faces.get(index)) {
            let runs = |ability: &&Ability| caps.missing(&used_primitives(ability)).is_empty();
            for lowered in face.abilities.iter().filter(runs).filter_map(lower) {
                match lowered {
                    Lowered::Mana(mana) => definition.abilities.push(AbilityDef::Mana(mana)),
                    Lowered::Rules(rules) => definition.rules.push(rules),
                }
            }
        }
        definition.manual = manual_clauses(&record, attach, index, printed);
        definition
    };
    let definition = match (played_layout(printed, caps), printed.faces.len()) {
        (Some(layout), count) => whole_card(printed, layout, (0..count).map(face_of).collect()),
        (None, 0) => CardDefinition::new(printed.name.clone()),
        (None, _) => face_of(0),
    };
    Assembled {
        definition,
        support: record,
    }
}

/// The clauses of face `index` the table follows by hand. A face the engine attaches rules to has the clauses the
/// support record names, read out of its oracle text; a face it attaches none to (a `Manual` card) is one clause of its
/// whole rules text.
fn manual_clauses(
    record: &SupportRecord,
    attach: bool,
    index: usize,
    printed: &PrintedCard,
) -> Vec<ManualClause> {
    let text = printed
        .faces
        .get(index)
        .map_or("", |face| face.oracle_text.as_str());
    let named = || record.clauses.iter().filter(|clause| clause.face == index);
    if record.support == Support::Verified {
        return Vec::new();
    }
    if !attach {
        let reason = named().next().map_or_else(
            || "the engine runs none of this card's rules".to_owned(),
            |clause| clause.reason.clone(),
        );
        return crate::support::has_rules_text(text)
            .then(|| ManualClause {
                text: text.to_owned(),
                reason,
            })
            .into_iter()
            .collect();
    }
    named()
        .filter_map(|clause| {
            clause.span.slice(text).map(|clause_text| ManualClause {
                text: clause_text.to_owned(),
                reason: clause.reason.clone(),
            })
        })
        .collect()
}

/// The layout of a card with several faces when the engine plays it: two faces of a shape it knows.
pub(crate) fn played_layout(printed: &PrintedCard, caps: &Caps) -> Option<Layout> {
    (printed.faces.len() == 2 && caps.runs(FACES))
        .then(|| Layout::from_scryfall(&printed.layout))
        .flatten()
        .filter(|layout| *layout != Layout::Prepare || caps.runs(PREPARE))
}

/// A card with several faces as the engine plays it: the card outside the stack and the battlefield, and its faces.
///
/// A split card is both halves there (CR 709.4): two names, the combined colors and types and the mana symbols
/// of both costs, so its mana value is their sum. The other layouts are their front face.
fn whole_card(
    printed: &PrintedCard,
    layout: Layout,
    mut faces: Vec<CardDefinition>,
) -> CardDefinition {
    if layout == Layout::Flip {
        flipped_keeps_cost(&mut faces);
    }
    let mut whole = match layout {
        Layout::Split => {
            let mut whole = CardDefinition::new(printed.name.clone());
            let symbols = faces
                .iter()
                .filter_map(|face| face.mana_cost.as_ref())
                .flat_map(|cost| cost.symbols().iter().copied());
            whole.mana_cost = Some(ManaCost::from_symbols(symbols));
            for face in &faces {
                whole.types.extend(face.types.iter().copied());
                whole.supertypes.extend(face.supertypes.iter().copied());
            }
            let union: ColorSet = faces.iter().flat_map(|face| face.colors().iter()).collect();
            let from_cost = whole
                .mana_cost
                .as_ref()
                .map(ManaCost::colors)
                .unwrap_or_default();
            whole.color_indicator = (union != from_cost).then_some(union);
            whole.manual = faces.iter().flat_map(|face| face.manual.clone()).collect();
            // The card as a whole is what a fused split spell is (CR 702.102c), so it carries that spell's rules.
            whole.rules.extend(fused_spell(&faces).map(Arc::new));
            whole
        }
        _ => faces[0].clone(),
    };
    whole.layout = layout;
    whole.faces = faces;
    crate::dk1c5_rooms::dk1c5_doors(&mut whole);
    whole
}

/// The bottom half of a flip card has the mana cost and the colour of the top half (CR 710.2), so a flipped permanent
/// keeps its mana value and colours.
fn flipped_keeps_cost(faces: &mut [CardDefinition]) {
    if let [top, bottom] = faces {
        bottom.mana_cost.clone_from(&top.mana_cost);
        bottom.x_in_cost = top.x_in_cost;
        bottom.color_indicator = top.color_indicator;
    }
}

/// The spell a split card with fuse is when both halves are cast (CR 702.102c): the targets of the left half and then
/// those of the right, and the effects of the left half and then those of the right with their targets renumbered after
/// the left half's. `None` when a half has no spell or the card has no fuse, and when an amount is divided among targets
/// (the announcement divides one specification only).
fn fused_spell(halves: &[CardDefinition]) -> Option<Ability> {
    let spells: Vec<&Ability> = halves
        .iter()
        .map(|half| {
            half.rules
                .iter()
                .map(|rules| &**rules)
                .find(|rules| matches!(rules, Ability::Spell { .. }))
        })
        .collect::<Option<_>>()?;
    let fuse = halves.iter().any(|half| {
        half.rules
            .iter()
            .any(|rules| matches!(**rules, Ability::Keyword(Keyword::Fuse)))
    });
    let [
        Ability::Spell {
            targets: left_targets,
            effects: left_effects,
        },
        Ability::Spell {
            targets: right_targets,
            effects: right_effects,
        },
    ] = spells.as_slice()
    else {
        return None;
    };
    if !fuse
        || left_targets
            .iter()
            .chain(right_targets)
            .any(|spec| spec.divide.is_some())
    {
        return None;
    }
    let mut effects = serde_json::to_value(right_effects).ok()?;
    shift_targets(&mut effects, u8::try_from(left_targets.len()).ok()?);
    let mut fused = left_effects.clone();
    fused.extend(serde_json::from_value::<Vec<Effect>>(effects).ok()?);
    Some(Ability::Spell {
        targets: left_targets.iter().chain(right_targets).cloned().collect(),
        effects: fused,
    })
}

/// Adds `by` to every target index a reference or an amount names in the JSON of some effects.
fn shift_targets(value: &mut serde_json::Value, by: u8) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, inner) in map.iter_mut() {
                match (key.as_str(), inner.as_u64()) {
                    ("Target" | "TargetCount", Some(index)) => {
                        *inner = serde_json::Value::from(index + u64::from(by));
                    }
                    _ => shift_targets(inner, by),
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                shift_targets(item, by);
            }
        }
        _ => {}
    }
}

/// What a bare `{T}: Add ...` adds: no targets, no limit, usable at instant speed from the battlefield, cost exactly
/// `{T}`, and mana added once to the controller's pool that is restricted in no way.
pub(crate) fn bare_tap_mana(ability: &Ability) -> Option<&Produce> {
    let Ability::Activated {
        cost,
        timing: Timing::Instant,
        condition: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    } = ability
    else {
        return None;
    };
    let [CostItem::Tap] = cost.0.as_slice() else {
        return None;
    };
    let [
        Effect::AddMana {
            who: Ref::You,
            mana,
            times,
            only,
            lasts,
            when_spent,
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    (targets.is_empty()
        && times.is_one()
        && only.is_empty()
        && lasts.is_step()
        && when_spent.is_none())
    .then_some(mana)
}

/// The mana a plain `{T}: Add {X}` adds: a bare tap ability (`bare_tap_mana`) of one fixed mana.
pub fn plain_mana_ability(ability: &Ability) -> Option<ManaType> {
    let Produce::Fixed(mana) = bare_tap_mana(ability)? else {
        return None;
    };
    let [one] = mana.as_slice() else {
        return None;
    };
    Some(*one)
}

/// What a mana ability the engine runs without the stack adds each time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManaProduced<'a> {
    /// This many mana of one type.
    Fixed {
        /// The type.
        mana: ManaType,
        /// How many.
        count: u32,
    },
    /// This many mana, each of a type its controller picks from these (all five colors when none are listed) as the
    /// ability is activated.
    AnyCombination {
        /// How many mana: a number, the X of the cost, or an amount counted when it is offered.
        count: &'a Amount,
        /// The types to pick from; none means the five colors.
        among: &'a [ManaType],
    },
    /// Mana of several types, one of each listed (`{W}{U}`, `{C}{G}`).
    Mixed(&'a [ManaType]),
    /// This many mana of one colour its controller picks when the ability is activated: a number, or an amount counted
    /// when it is offered.
    AnyColor(&'a Amount),
    /// What `base` adds, or what `instead` adds when the condition holds as the ability is offered.
    Instead {
        /// Added when the condition does not hold.
        base: &'a Produce,
        /// The condition.
        when: &'a Condition,
        /// Added in its place when the condition holds.
        instead: &'a Produce,
    },
    /// One mana of a type its controller picks from these when the ability is activated.
    OneOf(&'a [ManaType]),
    /// All the mana of one list its controller picks from these when the ability is activated.
    OneOfLists(&'a [Vec<ManaType>]),
    /// This many mana of the colour the source chose as it entered.
    ChosenColor(u32),
    /// One mana of this type or of the colour the source chose as it entered, picked when the ability is activated.
    OrChosenColor(ManaType),
    /// This many mana of one colour of its controller's commander's colour identity, picked when activated.
    CommanderIdentity(u32),
    /// This many mana of one colour (or type) that an object matching `among` could produce, picked when activated.
    ProducedBy {
        /// The objects whose mana abilities are looked at.
        among: &'a Selector,
        /// How many.
        count: u32,
        /// Colorless counts as a type.
        colorless: bool,
    },
}

impl<'a> ManaProduced<'a> {
    /// What an effect that adds `mana` adds each time; none for a list of nothing or more mana than fit in a count.
    pub fn of(mana: &'a Produce) -> Option<Self> {
        Some(match mana {
            Produce::Fixed(list) => {
                let (&first, rest) = list.split_first()?;
                if rest.iter().all(|other| *other == first) {
                    ManaProduced::Fixed {
                        mana: first,
                        count: u32::try_from(list.len()).ok()?,
                    }
                } else {
                    ManaProduced::Mixed(list)
                }
            }
            Produce::AnyColor(count) => ManaProduced::AnyColor(count),
            Produce::Instead {
                base,
                when,
                instead,
            } => ManaProduced::Instead {
                base,
                when,
                instead,
            },
            Produce::OneOf(types) => ManaProduced::OneOf(types),
            Produce::OneOfLists(lists) => ManaProduced::OneOfLists(lists),
            Produce::ChosenColor(count) => ManaProduced::ChosenColor(*count),
            Produce::OrChosenColor(mana) => ManaProduced::OrChosenColor(*mana),
            Produce::CommanderIdentity(count) => ManaProduced::CommanderIdentity(*count),
            Produce::ProducedBy {
                among,
                count,
                colorless,
            } => ManaProduced::ProducedBy {
                among,
                count: *count,
                colorless: *colorless,
            },
            Produce::AnyCombination { count, among } => {
                ManaProduced::AnyCombination { count, among }
            }
            Produce::OfTapped => return None,
        })
    }
}

impl ManaProduced<'_> {
    /// How many mana one addition makes.
    pub fn each(&self) -> u32 {
        match self {
            ManaProduced::Fixed { count, .. }
            | ManaProduced::ChosenColor(count)
            | ManaProduced::CommanderIdentity(count)
            | ManaProduced::ProducedBy { count, .. } => *count,
            // The smaller of the two, as the condition is not known here.
            ManaProduced::Instead { base, instead, .. } => [base, instead]
                .into_iter()
                .filter_map(|produce| ManaProduced::of(produce))
                .map(|produced| produced.each())
                .min()
                .unwrap_or(0),
            // A counted amount adds mana unless it counts nothing, which only the game can tell.
            ManaProduced::AnyColor(count) | ManaProduced::AnyCombination { count, .. } => {
                count.fixed().unwrap_or(1)
            }
            ManaProduced::OneOf(_) | ManaProduced::OrChosenColor(_) => 1,
            ManaProduced::OneOfLists(lists) => lists
                .iter()
                .map(|list| u32::try_from(list.len()).unwrap_or(u32::MAX))
                .min()
                .unwrap_or(0),
            ManaProduced::Mixed(list) => u32::try_from(list.len()).unwrap_or(u32::MAX),
        }
    }
}

/// A mana ability of the shape the engine activates: no targets, instant speed from the battlefield, an optional
/// condition ("Activate only if you control a Swamp"), a cost of `{T}`, sacrificing the source, mana and fixed
/// amounts of life, energy and counters of the source, mana added to its controller's pool (some number of times) and
/// then effects that need no answer from anyone (CR 605.3b). The mana of its cost is paid from the pool when it is
/// activated, so an ability with one is a source of a payment only (a Signet).
///
/// The plain `{T}: Add {X}` has this shape and is carried as an [`AbilityDef`]; the others (Treasure, Sol Ring, Gaea's
/// Cradle, the painlands, the bouncelands) stay in the rules of the card.
#[derive(Debug, Clone, PartialEq)]
pub struct ManaShape<'a> {
    /// What it adds each time.
    pub produces: ManaProduced<'a>,
    /// How many times.
    pub times: &'a Amount,
    /// The source is tapped.
    pub tap: bool,
    /// The source is sacrificed.
    pub sacrifice: bool,
    /// The mana of its cost (`{1}` for a Signet), paid from mana already in the pool; empty when it costs none.
    pub mana: ManaCost,
    /// The other parts of its cost: life, energy, counters of the source.
    pub pays: Vec<&'a CostItem>,
    /// What must hold to activate it.
    pub condition: Option<&'a Condition>,
    /// It can be activated once each turn ("Activate only once each turn").
    pub once: bool,
    /// The parts of its cost that take other permanents the payer controls: tapping or sacrificing some number of them.
    pub picks: Vec<&'a CostItem>,
    /// What it does before adding mana: a counter put on the source, which the amount of mana may count.
    pub before: &'a [Effect],
    /// What it does besides adding mana.
    pub then: &'a [Effect],
    /// What the mana may be spent on; none listed means anything.
    pub only: &'a [SpendOn],
    /// What spending the mana to cast a spell sets off.
    pub when_spent: Option<&'a WhenSpent>,
}

impl ManaShape<'_> {
    /// Whether it adds one mana of a fixed type or of any colour, once, and does nothing else (a Treasure, a Lotus
    /// Petal); any other shape is the primitive `Ability.ManaAbility.Complex`.
    pub fn is_simple(&self) -> bool {
        (matches!(self.produces, ManaProduced::Fixed { count: 1, .. })
            || matches!(self.produces, ManaProduced::AnyColor(count) if count.is_one()))
            && self.times.is_one()
            && self.then.is_empty()
            && self.before.is_empty()
            && self.pays.is_empty()
            && self.mana.symbols().is_empty()
            && self.condition.is_none()
            && !self.once
            && self.picks.is_empty()
    }
}

/// Whether `effect` is one a mana ability may run after its mana: it asks nothing of anyone and touches the
/// controller or the source alone ("deals 1 damage to you", "draw a card", "you lose 1 life", "put a charge counter on
/// this artifact").
fn needs_no_answer(effect: &Effect) -> bool {
    match effect {
        Effect::Draw { who, .. } | Effect::LoseLife { who, .. } | Effect::GainLife { who, .. } => {
            *who == Ref::You
        }
        Effect::DealDamage { source, to, .. } => {
            *to == Ref::You && source.as_ref().is_none_or(|source| *source == Ref::This)
        }
        Effect::PutCounters { on, .. } => *on == Ref::This,
        _ => false,
    }
}

/// Whether `item` is a part of a mana ability's cost the engine pays with the activation: a fixed amount of life,
/// energy or counters of the source, or X counters of the source (the X of "Remove X storage counters from this land:
/// Add X mana in any combination of {R} and/or {G}").
fn pays_with_activation(item: &CostItem) -> bool {
    let fixed = match item {
        CostItem::PayLife(amount)
        | CostItem::Mill(amount)
        | CostItem::PayEnergy(amount)
        | CostItem::RemoveCounters { count: amount, .. } => {
            matches!(amount, Amount::Fixed(number) if *number > 0)
        }
        _ => false,
    };
    fixed || is_x_counters(item)
}

/// Whether `item` removes X counters from the source.
fn is_x_counters(item: &CostItem) -> bool {
    matches!(
        item,
        CostItem::RemoveCounters {
            count: Amount::X,
            ..
        }
    )
}

/// Whether `item` is a part of a mana ability's cost that takes other permanents or cards of the payer, which the payment
/// picks: tapping some number of untapped permanents, sacrificing some number of permanents or exiling some number of
/// cards from the hand.
fn picked_by_the_planner(item: &CostItem) -> bool {
    match item {
        CostItem::TapPermanents { count, .. }
        | CostItem::Sacrifice { count, .. }
        | CostItem::ExileFromHand { count, .. } => {
            matches!(count, Amount::Fixed(number) if *number > 0)
        }
        _ => false,
    }
}

/// The shape of a mana ability the engine activates at priority, if `ability` has it.
///
/// The first effect adds mana to the controller's pool and the rest each ask nothing; one effect of a kind.
pub fn mana_shape(ability: &Ability) -> Option<ManaShape<'_>> {
    let Ability::Activated {
        cost,
        timing: Timing::Instant,
        condition,
        limit,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    } = ability
    else {
        return None;
    };
    let (mut tap, mut sacrifice) = (false, false);
    let mut pays = Vec::new();
    let mut picks = Vec::new();
    let mut cost_mana = None;
    for item in &cost.0 {
        let seen = match item {
            CostItem::Tap => &mut tap,
            CostItem::SacrificeSelf => &mut sacrifice,
            // A cost with an X or a Phyrexian symbol is not paid from the pool alone.
            CostItem::Mana(paid) if !paid.has_x() && paid.rest().life_symbols() == 0 => {
                if cost_mana.replace(paid.rest().clone()).is_some() {
                    return None;
                }
                continue;
            }
            other if pays_with_activation(other) => {
                pays.push(other);
                continue;
            }
            other if picked_by_the_planner(other) => {
                picks.push(other);
                continue;
            }
            _ => return None,
        };
        if std::mem::replace(seen, true) {
            return None;
        }
    }
    let (before, effects) = c8_time_before(effects);
    let [
        Effect::AddMana {
            who: Ref::You,
            mana,
            times,
            only,
            lasts,
            when_spent,
        },
        then @ ..,
    ] = effects
    else {
        return None;
    };
    if !lasts.is_step() {
        return None;
    }
    let produces = ManaProduced::of(mana)?;
    // A combination keeps one source for each way to pick it, so it is added once and there are not too many ways.
    if let ManaProduced::AnyCombination { count, among } = &produces {
        let types = if among.is_empty() { 5 } else { among.len() };
        let fits = count
            .fixed()
            .is_none_or(|count| combinations(types, count) <= MANA_COMBINATIONS);
        if !times.is_one() || !fits {
            return None;
        }
    }
    // The X of the cost is the X of the mana and nothing else binds it.
    let x_paid = pays.iter().filter(|item| is_x_counters(item)).count();
    let x_added = usize::from(matches!(
        produces,
        ManaProduced::AnyCombination {
            count: Amount::X,
            ..
        } | ManaProduced::AnyColor(Amount::X)
    ));
    if x_paid != x_added {
        return None;
    }
    let distinct = c8_time_distinct(then);
    (targets.is_empty()
        && (tap || sacrifice || !pays.is_empty() || !picks.is_empty() || limit.is_some())
        && then.iter().all(needs_no_answer)
        && distinct
        && produces.each() > 0)
        .then_some(ManaShape {
            produces,
            times,
            tap,
            sacrifice,
            mana: cost_mana.unwrap_or_default(),
            pays,
            condition: condition.as_ref(),
            once: *limit == Some(Limit::OncePerTurn),
            picks,
            before,
            then,
            only,
            when_spent: when_spent.as_deref(),
        })
}

/// The effects of a mana ability split before its mana: "Put a charge counter on this artifact. Add X mana ..., where
/// X is the number of charge counters on this artifact." puts one counter on the source first and the mana counts it.
fn c8_time_before(effects: &[Effect]) -> (&[Effect], &[Effect]) {
    let first = usize::from(matches!(
        effects.first(),
        Some(Effect::PutCounters { on: Ref::This, .. })
    ));
    effects.split_at(first)
}

/// Whether no two of the effects are of one kind.
fn c8_time_distinct(then: &[Effect]) -> bool {
    then.iter()
        .enumerate()
        .all(|(i, effect)| then[..i].iter().all(|before| !same_kind(before, effect)))
}

/// Whether two effects are of one kind (the same variant).
fn same_kind(a: &Effect, b: &Effect) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// The definition of a token from the description of what creates it.
///
/// Its colours are a colour indicator (a token has no mana cost), and it keeps the abilities `caps` says the engine
/// runs, as a card does ([`assemble()`]).
pub fn token_definition(spec: &TokenSpec, caps: &Caps) -> CardDefinition {
    let mut definition = CardDefinition::new(spec.name.clone());
    if !spec.colors.is_empty() {
        definition.color_indicator = Some(spec.colors.iter().copied().collect());
    }
    definition.supertypes = spec.supertypes.iter().copied().collect();
    definition.types = spec.types.iter().copied().collect();
    definition.subtypes.clone_from(&spec.subtypes);
    definition.power = spec.power;
    definition.toughness = spec.toughness;
    let runs = |ability: &&Ability| caps.missing(&used_primitives(ability)).is_empty();
    for lowered in spec.abilities.iter().filter(runs).filter_map(lower) {
        match lowered {
            Lowered::Mana(mana) => definition.abilities.push(AbilityDef::Mana(mana)),
            Lowered::Rules(rules) => definition.rules.push(rules),
        }
    }
    definition
}

/// The form an ability takes in a card definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lowered {
    /// The plain `{T}: Add {X}`, which the engine runs without the stack.
    Mana(ManaAbility),
    /// Any other ability, run by the effect pipeline.
    Rules(Arc<Ability>),
}

/// The engine-facing form of an ability: the plain mana ability, or the rules of any other; a clause
/// left to the table has none.
///
/// What the engine does not run is kept out by its `Caps`, not here: a `Verified` card must not lose
/// an ability on its way into the game, and one it cannot run must not reach the game at all.
pub fn lower(ability: &Ability) -> Option<Lowered> {
    if let Some(produces) = plain_mana_ability(ability) {
        return Some(Lowered::Mana(ManaAbility { produces }));
    }
    (!ability.is_manual()).then(|| Lowered::Rules(Arc::new(ability.clone())))
}

/// The mana abilities the basic land types of a land give.
fn intrinsic_abilities(definition: &CardDefinition) -> Vec<AbilityDef> {
    if !definition.is_type(CardType::Land) {
        return Vec::new();
    }
    BASIC_LAND_TYPES
        .iter()
        .filter(|(name, _)| definition.subtypes.iter().any(|subtype| subtype.0 == *name))
        .map(|&(_, produces)| AbilityDef::Mana(ManaAbility { produces }))
        .collect()
}

/// The mana types the basic land types on the type line of a printed face give (CR 305.6), in the order of the rules;
/// none for a face that is no land or has no basic land type.
pub(crate) fn intrinsic_mana(face: &PrintedFace) -> Vec<ManaType> {
    intrinsic_abilities(&printed_side(face))
        .into_iter()
        .map(|AbilityDef::Mana(mana)| mana.produces)
        .collect()
}

/// The clauses for printed characteristics the engine cannot play with, on the front face and on
/// every face the engine does not play.
pub(crate) fn printed_gaps(printed: &PrintedCard, caps: &Caps) -> Vec<Clause> {
    let mut clauses = Vec::new();
    let played = played_layout(printed, caps).is_some();
    for (face, shown) in printed.faces.iter().enumerate() {
        if face > 0 && !played {
            clauses.push(Clause {
                face,
                span: whole_text(&shown.oracle_text),
                reason: format!(
                    "the engine plays only the front face of a card, so {} cannot be cast or turned over",
                    shown.name
                ),
            });
            continue;
        }
        for gap in read_face(shown).gaps {
            if gap.needs.is_none_or(|name| !caps.runs(name)) {
                clauses.push(Clause {
                    face,
                    span: whole_text(&shown.oracle_text),
                    reason: gap.reason,
                });
            }
        }
    }
    clauses
}

/// A printed characteristic the definition cannot hold or the engine cannot play with.
struct Gap {
    /// The capability that lifts the gap once the engine has it; `None` when the definition cannot hold the value.
    needs: Option<&'static str>,
    reason: String,
}

/// What a face's strings say, read once for the definition and for the gaps.
struct Read {
    cost: Option<ManaCostX>,
    types: TypeLine,
    power: Option<i32>,
    toughness: Option<i32>,
    loyalty: Option<u32>,
    defense: Option<u32>,
    gaps: Vec<Gap>,
}

/// The words of a type line.
#[derive(Default)]
struct TypeLine {
    supertypes: BTreeSet<Supertype>,
    types: BTreeSet<CardType>,
    subtypes: Vec<Subtype>,
    unknown: Vec<String>,
}

/// A printed power, toughness, loyalty or defense.
enum Stat {
    Absent,
    Number(i64),
    /// Not a number: `*`, `1+*`, `X`.
    Variable,
}

fn read_stat(text: Option<&str>) -> Stat {
    match text.map(str::trim) {
        None | Some("") => Stat::Absent,
        // Newer data may print the sign of a negative number as U+2212.
        Some(text) => text
            .replace('\u{2212}', "-")
            .parse()
            .map_or(Stat::Variable, Stat::Number),
    }
}

fn supertype_of(word: &str) -> Option<Supertype> {
    Some(match word {
        "Basic" => Supertype::Basic,
        "Legendary" => Supertype::Legendary,
        "Snow" => Supertype::Snow,
        "World" => Supertype::World,
        _ => return None,
    })
}

fn card_type_of(word: &str) -> Option<CardType> {
    Some(match word {
        "Artifact" => CardType::Artifact,
        "Battle" => CardType::Battle,
        "Creature" => CardType::Creature,
        "Enchantment" => CardType::Enchantment,
        "Instant" => CardType::Instant,
        // The old name of the type, kept by cards printed before the rename.
        "Kindred" | "Tribal" => CardType::Kindred,
        "Land" => CardType::Land,
        "Planeswalker" => CardType::Planeswalker,
        "Sorcery" => CardType::Sorcery,
        _ => return None,
    })
}

fn read_type_line(line: &str) -> TypeLine {
    let (head, tail) = line.split_once('\u{2014}').unwrap_or((line, ""));
    let mut read = TypeLine::default();
    for word in head.split_whitespace() {
        if let Some(supertype) = supertype_of(word) {
            read.supertypes.insert(supertype);
        } else if let Some(card_type) = card_type_of(word) {
            read.types.insert(card_type);
        } else {
            read.unknown.push(word.to_owned());
        }
    }
    let words: Vec<&str> = tail.split_whitespace().collect();
    let mut at = 0;
    while let Some(&word) = words.get(at) {
        // The one creature type of two words (CR 205.3m).
        if word == "Time" && words.get(at + 1) == Some(&"Lord") {
            read.subtypes.push(Subtype("Time Lord".to_owned()));
            at += 2;
        } else {
            read.subtypes.push(Subtype(word.to_owned()));
            at += 1;
        }
    }
    read
}

/// The Scryfall letters of a face as a color set; letters that are no color are skipped.
fn letters(colors: &[String]) -> ColorSet {
    colors
        .iter()
        .filter_map(|text| text.chars().next())
        .filter_map(Color::from_symbol)
        .collect()
}

fn read_face(face: &PrintedFace) -> Read {
    let mut gaps = Vec::new();
    let cost = if face.mana_cost.trim().is_empty() {
        None
    } else {
        match face.mana_cost.parse::<ManaCostX>() {
            Ok(cost) => Some(cost),
            Err(error) => {
                gaps.push(Gap {
                    needs: None,
                    reason: match error {
                        ParseManaCostError::Unsupported(symbol) => format!(
                            "the mana cost has the symbol {{{symbol}}}, which the engine does not know yet"
                        ),
                        ParseManaCostError::Malformed(_) => {
                            format!("the mana cost {:?} cannot be read", face.mana_cost)
                        }
                    },
                });
                None
            }
        }
    };
    if cost.as_ref().is_some_and(ManaCostX::has_x) {
        gaps.push(Gap {
            needs: Some(X_COST),
            reason: "the engine cannot pay {X} in a mana cost yet".to_owned(),
        });
    }
    let types = read_type_line(&face.type_line);
    if !types.unknown.is_empty() {
        gaps.push(Gap {
            needs: None,
            reason: format!(
                "the type line has words the engine does not know: {}",
                types.unknown.join(", ")
            ),
        });
    }
    let mut variable = Vec::new();
    let mut number = |name: &'static str, text: Option<&str>| -> Option<i64> {
        match read_stat(text) {
            Stat::Absent => None,
            Stat::Number(value) => Some(value),
            Stat::Variable => {
                variable.push(name);
                None
            }
        }
    };
    let power = number("power", face.power.as_deref()).and_then(|n| i32::try_from(n).ok());
    let toughness =
        number("toughness", face.toughness.as_deref()).and_then(|n| i32::try_from(n).ok());
    let loyalty = number("loyalty", face.loyalty.as_deref()).and_then(|n| u32::try_from(n).ok());
    let defense = number("defense", face.defense.as_deref()).and_then(|n| u32::try_from(n).ok());
    if !variable.is_empty() {
        let (verb, what, source) = if variable.len() == 1 {
            ("is", "a number", "it comes")
        } else {
            ("are", "numbers", "they come")
        };
        gaps.push(Gap {
            needs: Some(VARIABLE_STATS),
            reason: format!(
                "the printed {} {verb} not {what}, so {source} from an ability the engine does not run yet",
                variable.join(" and "),
            ),
        });
    }
    Read {
        cost,
        types,
        power,
        toughness,
        loyalty,
        defense,
        gaps,
    }
}

/// The printed characteristics of a face; its colors come from the cost unless the record gives
/// other ones, which only a color indicator does.
fn printed_side(face: &PrintedFace) -> CardDefinition {
    let read = read_face(face);
    let (mana_cost, x_in_cost) = read
        .cost
        .as_ref()
        .map_or((None, 0), |cost| (Some(cost.rest().clone()), cost.x()));
    let from_cost = mana_cost.as_ref().map(ManaCost::colors).unwrap_or_default();
    // An empty list is no evidence: records of split and adventure faces often leave colors out.
    let shown = letters(&face.colors);
    CardDefinition {
        name: face.name.clone(),
        mana_cost,
        x_in_cost,
        supertypes: read.types.supertypes,
        types: read.types.types,
        subtypes: read.types.subtypes,
        power: read.power,
        toughness: read.toughness,
        loyalty: read.loyalty,
        defense: read.defense,
        color_indicator: (!shown.is_empty() && shown != from_cost).then_some(shown),
        abilities: Vec::new(),
        rules: Vec::new(),
        layout: Layout::Normal,
        manual: Vec::new(),
        faces: Vec::new(),
        identity: None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the type line, the stats and the cost of a face read into a definition, the plain mana ability is the one shape the engine carries, and characteristics the definition cannot hold make the card `Manual`.
    use super::*;
    use rstest::rstest;

    use crate::card::{Face, ImplStatus};
    use crate::support::oracle_fingerprint;

    fn face(name: &str, cost: &str, type_line: &str) -> PrintedFace {
        PrintedFace {
            name: name.into(),
            mana_cost: cost.into(),
            type_line: type_line.into(),
            ..PrintedFace::default()
        }
    }

    fn card(front: PrintedFace) -> PrintedCard {
        PrintedCard {
            layout: String::new(),
            oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
            name: front.name.clone(),
            keywords: vec![],
            faces: vec![front],
        }
    }

    fn ability(json: &str) -> Ability {
        serde_json::from_str(json).unwrap()
    }

    #[rstest]
    #[case::creature("Creature \u{2014} Elf Druid", &[], &["Creature"], &["Elf", "Druid"])]
    #[case::legendary("Legendary Creature \u{2014} Beast", &["Legendary"], &["Creature"], &["Beast"])]
    #[case::basic_land("Basic Land \u{2014} Forest", &["Basic"], &["Land"], &["Forest"])]
    #[case::several_types("Artifact Creature \u{2014} Golem", &[], &["Artifact", "Creature"], &["Golem"])]
    #[case::no_subtypes("Instant", &[], &["Instant"], &[])]
    #[case::snow("Snow Land", &["Snow"], &["Land"], &[])]
    #[case::old_tribal("Tribal Instant \u{2014} Goblin", &[], &["Kindred", "Instant"], &["Goblin"])]
    #[case::two_word_type("Legendary Creature \u{2014} Time Lord Doctor", &["Legendary"], &["Creature"], &["Time Lord", "Doctor"])]
    #[case::empty("", &[], &[], &[])]
    fn type_lines_read_into_supertypes_types_and_subtypes(
        #[case] line: &str,
        #[case] supertypes: &[&str],
        #[case] types: &[&str],
        #[case] subtypes: &[&str],
    ) {
        let read = read_type_line(line);
        let names = |set: Vec<String>| -> BTreeSet<String> { set.into_iter().collect() };
        let expected = |words: &[&str]| -> BTreeSet<String> {
            words.iter().map(|w| (*w).to_owned()).collect()
        };
        assert_eq!(
            names(read.supertypes.iter().map(|s| format!("{s:?}")).collect()),
            expected(supertypes)
        );
        assert_eq!(
            names(read.types.iter().map(|t| format!("{t:?}")).collect()),
            expected(types)
        );
        let subtypes_read: Vec<&str> = read.subtypes.iter().map(|s| s.0.as_str()).collect();
        assert_eq!(subtypes_read, subtypes);
        assert!(read.unknown.is_empty(), "{line}");
    }

    #[test]
    fn a_word_of_no_known_kind_is_kept_apart() {
        assert_eq!(
            read_type_line("Plane \u{2014} Dominaria").unknown,
            ["Plane"]
        );
    }

    #[rstest]
    #[case::none(None, "absent")]
    #[case::blank(Some(" "), "absent")]
    #[case::number(Some("3"), "3")]
    #[case::zero(Some("0"), "0")]
    #[case::negative(Some("-1"), "-1")]
    #[case::unicode_minus(Some("\u{2212}2"), "-2")]
    #[case::star(Some("*"), "variable")]
    #[case::star_plus(Some("1+*"), "variable")]
    #[case::x(Some("X"), "variable")]
    fn stats_are_numbers_or_variable(#[case] text: Option<&str>, #[case] expected: &str) {
        let shown = match read_stat(text) {
            Stat::Absent => "absent".to_owned(),
            Stat::Number(n) => n.to_string(),
            Stat::Variable => "variable".to_owned(),
        };
        assert_eq!(shown, expected);
    }

    #[test]
    fn a_face_reads_into_the_definition_it_prints() {
        let mut front = face(
            "Jace, the Mind Sculptor",
            "{2}{U}{U}",
            "Legendary Planeswalker \u{2014} Jace",
        );
        front.loyalty = Some("3".into());
        let shown = assemble(&card(front), None, &Caps::none()).definition;
        assert_eq!(shown.name, "Jace, the Mind Sculptor");
        assert_eq!(shown.mana_cost, Some(ManaCost::parse("{2}{U}{U}").unwrap()));
        assert_eq!(
            (shown.loyalty, shown.defense, shown.x_in_cost),
            (Some(3), None, 0)
        );
        assert!(shown.supertypes.contains(&Supertype::Legendary));
        assert!(shown.is_type(CardType::Planeswalker));
        assert_eq!(shown.subtypes, [Subtype("Jace".into())]);
        assert_eq!((shown.power, shown.toughness), (None, None));
    }

    #[rstest]
    #[case::no_cost("", None, 0)]
    #[case::free("{0}", Some("{0}"), 0)]
    #[case::plain("{2}{R}", Some("{2}{R}"), 0)]
    #[case::x("{X}{R}", Some("{R}"), 1)]
    #[case::two_x("{X}{X}", Some(""), 2)]
    #[case::hybrid("{R/W}{R/W}", Some("{R/W}{R/W}"), 0)]
    fn a_cost_keeps_its_x_beside_the_symbols_the_engine_pays(
        #[case] text: &str,
        #[case] paid: Option<&str>,
        #[case] x: u8,
    ) {
        let shown = assemble(&card(face("C", text, "Sorcery")), None, &Caps::none()).definition;
        assert_eq!(shown.mana_cost, paid.map(|p| ManaCost::parse(p).unwrap()));
        assert_eq!(shown.x_in_cost, x);
    }

    #[rstest]
    #[case::the_cost_says_it("{1}{R}", &["R"], false)]
    #[case::no_evidence("{1}{R}", &[], false)]
    #[case::indicator_without_a_cost("", &["U"], true)]
    #[case::indicator_over_a_cost("{4}", &["W", "U", "B", "R", "G"], true)]
    #[case::two_colors_in_either_order("{U}{R}", &["R", "U"], false)]
    fn a_color_indicator_is_kept_only_where_the_cost_does_not_say_the_colors(
        #[case] cost: &str,
        #[case] colors: &[&str],
        #[case] indicator: bool,
    ) {
        let mut front = face("C", cost, "Creature \u{2014} Elf");
        front.colors = colors.iter().map(|c| (*c).to_owned()).collect();
        let shown = assemble(&card(front.clone()), None, &Caps::none()).definition;
        assert_eq!(shown.color_indicator.is_some(), indicator);
        if !colors.is_empty() {
            assert_eq!(shown.colors(), letters(&front.colors));
        }
    }

    #[rstest]
    #[case::forest("Basic Land \u{2014} Forest", &[ManaType::Green])]
    #[case::plains("Basic Land \u{2014} Plains", &[ManaType::White])]
    #[case::dual("Land \u{2014} Mountain Forest", &[ManaType::Red, ManaType::Green])]
    #[case::listed_in_the_order_of_the_rules("Land \u{2014} Forest Island", &[ManaType::Blue, ManaType::Green])]
    #[case::no_land_type("Land", &[])]
    #[case::wastes("Basic Land", &[])]
    #[case::not_a_land("Creature \u{2014} Forest Dryad", &[])]
    fn land_types_give_their_mana_abilities_to_every_card_with_them(
        #[case] type_line: &str,
        #[case] mana: &[ManaType],
    ) {
        let shown = assemble(&card(face("L", "", type_line)), None, &Caps::none()).definition;
        let expected: Vec<AbilityDef> = mana
            .iter()
            .map(|&produces| AbilityDef::Mana(ManaAbility { produces }))
            .collect();
        assert_eq!(shown.abilities, expected);
    }

    #[rstest]
    #[case::llanowar(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        Some(ManaType::Green)
    )]
    #[case::colorless(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#, Some(ManaType::Colorless))]
    #[case::sol_ring_adds_two(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}]}}"#, None)]
    #[case::any_color(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        None
    )]
    #[case::cradle_adds_many_times(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Count":[{"Type":"Creature"}]}}}]}}"#, None)]
    #[case::adds_twice(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":2}}]}}"#, None)]
    #[case::for_an_opponent(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"who":"Opponents","mana":{"Fixed":["Green"]}}}]}}"#, None)]
    #[case::costs_more(r#"{"Activated":{"cost":["Tap",{"PayLife":1}],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#, None)]
    #[case::costs_mana(
        r#"{"Activated":{"cost":"{1}","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        None
    )]
    #[case::at_sorcery_speed(r#"{"Activated":{"cost":["Tap"],"timing":"Sorcery","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#, None)]
    #[case::once_a_turn(r#"{"Activated":{"cost":["Tap"],"limit":"OncePerTurn","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#, None)]
    #[case::from_the_graveyard(r#"{"Activated":{"cost":["Tap"],"from":"Graveyard","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#, None)]
    #[case::and_draws(r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}},{"Draw":{}}]}}"#, None)]
    #[case::not_activated(r#"{"Keyword":"Flying"}"#, None)]
    fn only_a_plain_tap_for_one_mana_is_a_mana_ability_the_engine_carries(
        #[case] json: &str,
        #[case] expected: Option<ManaType>,
    ) {
        let parsed = ability(json);
        assert_eq!(plain_mana_ability(&parsed), expected);
        match (expected, lower(&parsed)) {
            (Some(produces), lowered) => {
                assert_eq!(lowered, Some(Lowered::Mana(ManaAbility { produces })));
            }
            (None, Some(Lowered::Rules(rules))) => {
                assert_eq!(*rules, parsed, "any other ability keeps its rules whole");
            }
            (None, other) => panic!("{other:?}"),
        }
        let used = used_primitives(&parsed);
        assert_eq!(used.contains(PLAIN_MANA_ABILITY), expected.is_some());
        if expected.is_some() {
            assert_eq!(used.len(), 1, "{used:?}");
        }
    }

    /// What `mana_shape` reads of `json`: what is added each time, whether it taps and sacrifices, and how many effects follow.
    fn shape_of(json: &str) -> Option<(ManaProduced<'static>, bool, bool, usize)> {
        let parsed: &'static Ability = Box::leak(Box::new(ability(json)));
        mana_shape(parsed).map(|s| (s.produces, s.tap, s.sacrifice, s.then.len()))
    }

    static ONE: Amount = Amount::Fixed(1);
    static THREE: Amount = Amount::Fixed(3);

    const GREEN: ManaProduced<'static> = ManaProduced::Fixed {
        mana: ManaType::Green,
        count: 1,
    };

    #[rstest]
    #[case::treasure(
        r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some((ManaProduced::AnyColor(&ONE), true, true, 0))
    )]
    #[case::lotus_petal_without_the_tap(
        r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}}]}}"#,
        Some((ManaProduced::Fixed { mana: ManaType::Red, count: 1 }, false, true, 0))
    )]
    #[case::a_limit_and_a_mana_cost(
        r#"{"Activated":{"cost":"{1}","limit":"OncePerTurn","effects":[{"AddMana":{"mana":{"OneOf":["Red","White"]}}}]}}"#,
        Some((ManaProduced::OneOf(&[ManaType::Red, ManaType::White]), false, false, 0))
    )]
    #[case::a_limit_alone_has_no_cost_to_pay_for_it(
        r#"{"Activated":{"cost":"{1}","effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}}]}}"#,
        None
    )]
    #[case::the_plain_ability_has_the_shape_too(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        Some((GREEN, true, false, 0))
    )]
    #[case::sol_ring_adds_two_of_one_type(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}]}}"#,
        Some((ManaProduced::Fixed { mana: ManaType::Colorless, count: 2 }, true, false, 0))
    )]
    #[case::three_of_one_color(
        r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":3}}}]}}"#,
        Some((ManaProduced::AnyColor(&THREE), true, true, 0))
    )]
    #[case::counted_times(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Count":[{"Type":"Creature"}]}}}]}}"#,
        Some((GREEN, true, false, 0))
    )]
    #[case::a_choice_of_two(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["White","Blue"]}}}]}}"#,
        Some((ManaProduced::OneOf(&[ManaType::White, ManaType::Blue]), true, false, 0))
    )]
    #[case::the_chosen_color(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"ChosenColor":1}}}]}}"#,
        Some((ManaProduced::ChosenColor(1), true, false, 0))
    )]
    #[case::painland_damage(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["White","Blue"]}}},{"DealDamage":{"to":"You","amount":1}}]}}"#,
        Some((ManaProduced::OneOf(&[ManaType::White, ManaType::Blue]), true, false, 1))
    )]
    #[case::egg_draws(
        r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}},{"Draw":{}}]}}"#,
        Some((ManaProduced::AnyColor(&ONE), false, true, 1))
    )]
    #[case::damage_and_a_draw(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}},{"DealDamage":{"to":"You","amount":1}},{"Draw":{}}]}}"#,
        Some((ManaProduced::Fixed { mana: ManaType::Red, count: 1 }, true, false, 2))
    )]
    #[case::two_draws(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}},{"Draw":{}},{"Draw":{}}]}}"#,
        None
    )]
    #[case::damage_to_an_opponent(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}},{"DealDamage":{"to":"Opponents","amount":1}}]}}"#,
        None
    )]
    #[case::a_discard_asks_something(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}},{"Discard":{}}]}}"#,
        None
    )]
    #[case::mixed_mana(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["White","Blue"]}}}]}}"#,
        Some((ManaProduced::Mixed(&[ManaType::White, ManaType::Blue]), true, false, 0))
    )]
    #[case::the_commanders_colors(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"CommanderIdentity":1}}}]}}"#,
        Some((ManaProduced::CommanderIdentity(1), true, false, 0))
    )]
    #[case::for_an_opponent(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"who":"Opponents","mana":{"Fixed":["Green"]}}}]}}"#,
        None
    )]
    #[case::costs_nothing(
        r#"{"Activated":{"cost":[],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        None
    )]
    #[case::tap_twice(
        r#"{"Activated":{"cost":["Tap","Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        None
    )]
    #[case::sacrifices_another_without_the_tap(
        r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some((ManaProduced::AnyColor(&ONE), false, false, 0))
    )]
    #[case::costs_mana(
        r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some((ManaProduced::AnyColor(&ONE), true, false, 0))
    )]
    #[case::costs_mana_with_a_phyrexian_symbol(
        r#"{"Activated":{"cost":[{"Mana":"{B/P}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        None
    )]
    #[case::costs_mana_twice(
        r#"{"Activated":{"cost":[{"Mana":"{1}"},{"Mana":"{W}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        None
    )]
    #[case::costs_life(
        r#"{"Activated":{"cost":["Tap",{"PayLife":1}],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        Some((GREEN, true, false, 0))
    )]
    #[case::costs_energy(
        r#"{"Activated":{"cost":["Tap",{"PayEnergy":1}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some((ManaProduced::AnyColor(&ONE), true, false, 0))
    )]
    #[case::removes_counters_from_the_source(
        r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"counter":"charge"}}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some((ManaProduced::AnyColor(&ONE), true, false, 0))
    )]
    #[case::removes_counters_without_a_tap(
        r#"{"Activated":{"cost":[{"RemoveCounters":{"counter":"charge"}}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some((ManaProduced::AnyColor(&ONE), false, false, 0))
    )]
    #[case::pays_life_you_count(
        r#"{"Activated":{"cost":["Tap",{"PayLife":"X"}],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        None
    )]
    #[case::pays_no_life(
        r#"{"Activated":{"cost":["Tap",{"PayLife":0}],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        None
    )]
    #[case::only_if(
        r#"{"Activated":{"cost":["Tap"],"condition":{"Compare":{"left":{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"effects":[{"AddMana":{"mana":{"OneOf":["White","Black"]}}}]}}"#,
        Some((ManaProduced::OneOf(&[ManaType::White, ManaType::Black]), true, false, 0))
    )]
    #[case::a_counter_on_the_source(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}},{"PutCounters":{"on":"This","counter":"charge"}}]}}"#,
        Some((ManaProduced::Fixed { mana: ManaType::Red, count: 1 }, true, false, 1))
    )]
    #[case::a_counter_on_the_controller(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}},{"PutCounters":{"on":"You","counter":"charge"}}]}}"#,
        None
    )]
    #[case::at_sorcery_speed(
        r#"{"Activated":{"cost":["Tap"],"timing":"Sorcery","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        None
    )]
    #[case::once_a_turn(
        r#"{"Activated":{"cost":["Tap"],"limit":"OncePerTurn","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        Some((GREEN, true, false, 0))
    )]
    #[case::from_the_graveyard(
        r#"{"Activated":{"cost":["Tap"],"from":"Graveyard","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        None
    )]
    #[case::draws_without_mana(
        r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"Draw":{}}]}}"#,
        None
    )]
    fn a_mana_ability_the_engine_activates_at_priority_has_one_type_of_mana_and_costs_of_tap_and_sacrifice(
        #[case] json: &str,
        #[case] expected: Option<(ManaProduced<'static>, bool, bool, usize)>,
    ) {
        assert_eq!(shape_of(json), expected);
    }

    #[test]
    fn a_choice_of_lists_is_one_shape_that_keeps_every_list_and_adds_the_shortest_each_time() {
        let json = r#"{"Activated":{"cost":[{"Mana":"{W/U}"},"Tap"],"effects":[{"AddMana":{"mana":{"OneOfLists":[["White","White"],["White","Blue"],["Blue"]]}}}]}}"#;
        let (produces, tap, sacrifice, then) = shape_of(json).unwrap();
        let ManaProduced::OneOfLists(lists) = produces else {
            panic!("a choice of lists, got {produces:?}");
        };
        assert_eq!(
            lists,
            [
                vec![ManaType::White, ManaType::White],
                vec![ManaType::White, ManaType::Blue],
                vec![ManaType::Blue]
            ]
        );
        assert_eq!((produces.each(), tap, sacrifice, then), (1, true, false, 0));
    }

    #[rstest]
    #[case::treasure(
        r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        None
    )]
    #[case::sol_ring(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::gaeas_cradle(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Count":[{"Type":"Creature"}]}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::black_lotus(
        r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":3}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::a_painland(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["White","Blue"]}}},{"DealDamage":{"to":"You","amount":1}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::a_filter_land(
        r#"{"Activated":{"cost":[{"Mana":"{W/U}"},"Tap"],"effects":[{"AddMana":{"mana":{"OneOfLists":[["White","White"],["White","Blue"],["Blue","Blue"]]}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::a_mana_cost(
        r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::c8_time_a_counter_before_the_mana(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"PutCounters":{"on":"This","counter":"charge"}},{"AddMana":{"mana":{"AnyColor":{"Counters":{"on":"This","counter":"charge"}}}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::c8_time_two_counters_before_the_mana(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"PutCounters":{"on":"This","counter":"charge"}},{"PutCounters":{"on":"This","counter":"charge"}},{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some("Ability.ManaAbility.Other")
    )]
    #[case::c8_time_a_counter_on_another_before_the_mana(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"PutCounters":{"on":"You","counter":"charge"}},{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some("Ability.ManaAbility.Other")
    )]
    #[case::a_mana_cost_with_an_x(
        r#"{"Activated":{"cost":[{"Mana":"{X}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some("Ability.ManaAbility.Other")
    )]
    #[case::a_mana_cost_without_a_tap(
        r#"{"Activated":{"cost":[{"Mana":"{1}"}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some("Ability.ManaAbility.Other")
    )]
    #[case::once_each_turn_with_a_mana_cost(
        r#"{"Activated":{"cost":"{1}","limit":"OncePerTurn","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::once_each_turn_paying_life(
        r#"{"Activated":{"cost":[{"PayLife":1}],"limit":"OncePerTurn","effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::mixed_mana(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["White","Blue"]}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::the_commanders_colors(
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"CommanderIdentity":1}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::paying_life(
        r#"{"Activated":{"cost":["Tap",{"PayLife":1}],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::only_with_a_condition(
        r#"{"Activated":{"cost":["Tap"],"condition":"YourTurn","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::tapping_another_permanent(
        r#"{"Activated":{"cost":["Tap",{"TapPermanents":{"filter":[{"Type":"Creature"}]}}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::exiling_a_card_from_the_hand(
        r#"{"Activated":{"cost":[{"ExileFromHand":{"filter":[]}}],"effects":[{"AddMana":{"mana":{"OneOfLists":[["Black","Black"],["Green","Green"]]}}}]}}"#,
        Some("Ability.ManaAbility.Complex")
    )]
    #[case::exiling_an_uncounted_number_of_cards_from_the_hand(
        r#"{"Activated":{"cost":[{"ExileFromHand":{"filter":[],"count":{"Count":[]}}}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
        Some("Ability.ManaAbility.Other")
    )]
    #[case::not_a_mana_ability(r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#, None)]
    #[case::a_spell_that_adds_mana(
        r#"{"Spell":{"effects":[{"AddMana":{"mana":{"Fixed":["Black","Black","Black"]}}}]}}"#,
        None
    )]
    fn a_mana_ability_that_is_not_one_mana_is_named_complex_and_one_the_engine_does_not_activate_other(
        #[case] json: &str,
        #[case] named: Option<&str>,
    ) {
        let used = used_primitives(&ability(json));
        for name in ["Ability.ManaAbility.Complex", "Ability.ManaAbility.Other"] {
            assert_eq!(used.contains(name), Some(name) == named, "{name}: {used:?}");
        }
    }

    #[test]
    fn a_token_has_its_characteristics_and_the_abilities_the_engine_runs() {
        let spec: TokenSpec = serde_json::from_str(
            r#"{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],
                "power":1,"toughness":1,"abilities":[{"Keyword":"Flying"},
                {"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}},
                {"Activated":{"cost":"{2}","effects":[{"Draw":{}}]}}]}"#,
        )
        .unwrap();
        let runs = Caps::from_names([PLAIN_MANA_ABILITY, "Keyword.Flying"]);
        let made = token_definition(&spec, &runs);
        assert_eq!(made.name, "Soldier");
        assert!(made.is_type(CardType::Creature));
        assert_eq!(made.subtypes, [Subtype("Soldier".into())]);
        assert_eq!((made.power, made.toughness), (Some(1), Some(1)));
        assert_eq!(made.mana_cost, None);
        assert_eq!(made.colors(), ColorSet::EMPTY.with(Color::White));
        assert_eq!(
            made.abilities,
            [AbilityDef::Mana(ManaAbility {
                produces: ManaType::Green
            })]
        );
        assert_eq!(
            made.rules.len(),
            1,
            "the flying; the {{2}} ability is left out"
        );
        let none = token_definition(&spec, &Caps::none());
        assert!(none.abilities.is_empty() && none.rules.is_empty());
    }

    #[test]
    fn a_colorless_token_has_no_color_indicator() {
        let spec: TokenSpec = serde_json::from_str(
            r#"{"name":"Treasure","types":["Artifact"],"subtypes":["Treasure"]}"#,
        )
        .unwrap();
        let made = token_definition(&spec, &Caps::none());
        assert_eq!(made.color_indicator, None);
        assert!(made.colors().is_empty());
    }

    #[test]
    fn a_clause_left_to_the_table_has_no_form_in_the_definition() {
        let clause = Ability::Manual {
            reason: "needs a rule the data lacks".into(),
            span: crate::card::Span { start: 0, end: 4 },
        };
        assert_eq!(lower(&clause), None);
    }

    #[test]
    fn the_rules_of_a_spell_the_engine_runs_reach_the_definition_whole() {
        let bolt = r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#;
        let text = "Lightning Bolt deals 3 damage to any target.";
        let mut rules = elf_rules(ImplStatus::Full, vec![ability(bolt)]);
        rules.faces[0].oracle_fingerprint = oracle_fingerprint(text);
        let mut printed = elf();
        printed.faces[0].oracle_text = text.into();
        let runs = Caps::from_names(["Ability.Spell", "Effect.DealDamage"]);
        let assembled = assemble(&printed, Some(&rules), &runs);
        assert_eq!(assembled.support.support, Support::Verified);
        assert_eq!(assembled.definition.abilities, []);
        assert_eq!(
            assembled.definition.rules.len(),
            1,
            "a spell is not a mana ability"
        );
        assert_eq!(*assembled.definition.rules[0], ability(bolt));
        let idle = assemble(&printed, Some(&rules), &Caps::none());
        assert!(
            idle.definition.rules.is_empty(),
            "no rules for a Manual card"
        );
    }

    const MANA_TEXT: &str = "{T}: Add {G}.";
    const MANA_ABILITY: &str =
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#;

    fn elf_rules(status: ImplStatus, abilities: Vec<Ability>) -> Card {
        Card {
            oracle_id: "4457ed35-7c10-48c8-9776-456485fdf070".into(),
            name: "Elf".into(),
            impl_status: status,
            faces: vec![Face {
                name: "Elf".into(),
                oracle_fingerprint: oracle_fingerprint(MANA_TEXT),
                abilities,
            }],
            notes: None,
        }
    }

    fn elf() -> PrintedCard {
        let mut front = face("Elf", "{G}", "Creature \u{2014} Elf Druid");
        front.oracle_text = MANA_TEXT.into();
        front.power = Some("1".into());
        front.toughness = Some("1".into());
        card(front)
    }

    #[rstest]
    #[case::the_engine_runs_it(Caps::from_names([PLAIN_MANA_ABILITY]), Support::Verified, 1)]
    #[case::the_engine_runs_everything(Caps::everything(), Support::Verified, 1)]
    #[case::the_engine_runs_nothing(Caps::none(), Support::Manual, 0)]
    fn rules_are_attached_only_where_the_support_says_the_engine_runs_them(
        #[case] caps: Caps,
        #[case] expected: Support,
        #[case] attached: usize,
    ) {
        let rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
        let assembled = assemble(&elf(), Some(&rules), &caps);
        assert_eq!(assembled.support.support, expected);
        assert_eq!(assembled.definition.abilities.len(), attached);
        assert_eq!(
            assembled.definition.power,
            Some(1),
            "the printed side is always there"
        );
    }

    #[test]
    fn rules_without_a_current_text_are_not_attached() {
        let rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
        let mut errata = elf();
        errata.faces[0].oracle_text = "{T}: Add {R}.".into();
        let assembled = assemble(&errata, Some(&rules), &Caps::everything());
        assert_eq!(assembled.support.support, Support::Stale);
        assert!(
            assembled.definition.abilities.is_empty(),
            "no rules for a Manual card"
        );
        let no_file = assemble(&elf(), None, &Caps::everything());
        assert_eq!(no_file.support.support, Support::Manual);
        assert!(no_file.definition.abilities.is_empty(), "no file, no rules");
    }

    #[test]
    fn a_partial_card_carries_what_the_engine_runs_and_leaves_the_clauses() {
        let clause = Ability::Manual {
            reason: "needs a rule the data lacks".into(),
            span: crate::card::Span { start: 0, end: 4 },
        };
        let rules = elf_rules(ImplStatus::Partial, vec![ability(MANA_ABILITY), clause]);
        let caps = Caps::from_names([PLAIN_MANA_ABILITY]);
        let assembled = assemble(&elf(), Some(&rules), &caps);
        assert_eq!(assembled.support.support, Support::Partial);
        assert_eq!(assembled.support.clauses.len(), 1);
        assert_eq!(assembled.definition.abilities.len(), 1);
    }

    #[rstest]
    #[case::star_power(Some("*"), Some("2"), "power is not a number")]
    #[case::star_toughness(Some("2"), Some("1+*"), "toughness is not a number")]
    #[case::both(Some("*"), Some("*"), "power and toughness are not numbers")]
    fn a_stat_that_is_not_a_number_makes_the_card_manual(
        #[case] power: Option<&str>,
        #[case] toughness: Option<&str>,
        #[case] reason: &str,
    ) {
        let mut shown = elf();
        shown.faces[0].power = power.map(str::to_owned);
        shown.faces[0].toughness = toughness.map(str::to_owned);
        let rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
        let caps = Caps::from_names([PLAIN_MANA_ABILITY]);
        let assembled = assemble(&shown, Some(&rules), &caps);
        assert_eq!(assembled.support.support, Support::Manual);
        assert_eq!(assembled.support.clauses.len(), 1);
        assert!(
            assembled.support.clauses[0].reason.contains(reason),
            "{:?}",
            assembled.support.clauses
        );
        assert!(
            assembled.definition.abilities.is_empty(),
            "no rules for a Manual card"
        );
        // Once the engine knows variable stats the gap is gone.
        let lifted = Caps::from_names([PLAIN_MANA_ABILITY, VARIABLE_STATS]);
        assert_eq!(
            assemble(&shown, Some(&rules), &lifted).support.support,
            Support::Verified
        );
    }

    #[rstest]
    #[case::x_in_the_cost("{X}{G}", "Creature \u{2014} Elf", X_COST, "{X}")]
    #[case::snow_in_the_cost("{1}{S}", "Creature \u{2014} Elf", "", "{S}")]
    #[case::garbage_cost("1G", "Creature \u{2014} Elf", "", "cannot be read")]
    #[case::a_type_of_no_game_here("{G}", "Plane \u{2014} Dominaria", "", "Plane")]
    fn what_the_definition_cannot_hold_or_play_makes_the_card_manual(
        #[case] cost: &str,
        #[case] type_line: &str,
        #[case] lifted_by: &'static str,
        #[case] reason: &str,
    ) {
        let mut shown = elf();
        shown.faces[0].mana_cost = cost.into();
        shown.faces[0].type_line = type_line.into();
        let rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
        let caps = Caps::from_names([PLAIN_MANA_ABILITY]);
        let assembled = assemble(&shown, Some(&rules), &caps);
        assert_eq!(assembled.support.support, Support::Manual);
        assert!(
            assembled
                .support
                .clauses
                .iter()
                .any(|c| c.reason.contains(reason)),
            "{:?}",
            assembled.support.clauses
        );
        assert!(
            assembled.definition.abilities.is_empty(),
            "no rules for a Manual card"
        );
        let can_be_lifted = !lifted_by.is_empty();
        let lifted = Caps::from_names([PLAIN_MANA_ABILITY, lifted_by]);
        let state = assemble(&shown, Some(&rules), &lifted).support.support;
        assert_eq!(state == Support::Verified, can_be_lifted, "{state:?}");
    }

    #[test]
    fn faces_the_engine_does_not_play_make_the_card_manual_and_each_gets_a_clause() {
        let mut shown = elf();
        shown
            .faces
            .push(face("Back", "", "Creature \u{2014} Elf Wolf"));
        shown
            .faces
            .push(face("Third", "", "Creature \u{2014} Elf Bear"));
        let mut rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
        for name in ["Back", "Third"] {
            rules.faces.push(Face {
                name: name.into(),
                oracle_fingerprint: oracle_fingerprint(""),
                abilities: vec![],
            });
        }
        let caps = Caps::from_names([PLAIN_MANA_ABILITY]);
        let assembled = assemble(&shown, Some(&rules), &caps);
        assert_eq!(assembled.support.support, Support::Manual);
        let faces: Vec<usize> = assembled.support.clauses.iter().map(|c| c.face).collect();
        assert_eq!(faces, [1, 2]);
        assert_eq!(
            assembled.definition.name, "Elf",
            "the front face is the card"
        );
        assert!(
            assembled.definition.abilities.is_empty(),
            "no rules for a Manual card"
        );
        let lifted = Caps::from_names([PLAIN_MANA_ABILITY, FACES]);
        shown.layout = "modal_dfc".into();
        let three = assemble(&shown, Some(&rules), &lifted);
        assert_eq!(
            three.support.support,
            Support::Manual,
            "three faces are no shape the engine plays"
        );
        shown.faces.truncate(2);
        rules.faces.truncate(2);
        let state = assemble(&shown, Some(&rules), &lifted);
        assert_eq!(state.support.support, Support::Verified);
        assert_eq!(state.definition.abilities.len(), 1);
        assert_eq!(state.definition.layout, Layout::ModalDfc);
        assert_eq!(state.definition.faces.len(), 2);
    }

    #[test]
    fn a_record_without_faces_still_makes_a_named_definition() {
        let bare = PrintedCard {
            layout: String::new(),
            oracle_id: "x".into(),
            name: "Nothing".into(),
            ..PrintedCard::default()
        };
        let assembled = assemble(&bare, None, &Caps::none());
        assert_eq!(assembled.definition, CardDefinition::new("Nothing"));
        assert_eq!(assembled.support.support, Support::Manual);
    }

    mod generated {
        //! Goal: over generated faces, a card with rules the engine runs is Verified exactly when nothing printed on it is out of reach, and the definition keeps the X of a readable cost and the numbers of the stats.
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn verified_exactly_when_nothing_printed_is_out_of_reach(
                symbols in prop::collection::vec(
                    prop::sample::select(vec!["{1}", "{G}", "{W/U}", "{2/R}", "{G/P}", "{C}", "{0}"]),
                    0..5,
                ),
                x in 0usize..3,
                unknown_symbol in prop::option::of(prop::sample::select(vec!["{S}", "{Z}", "{HW}"])),
                power in prop::sample::select(vec![None, Some("0"), Some("7"), Some("-1"), Some("*"), Some("1+*")]),
                toughness in prop::sample::select(vec![None, Some("1"), Some("12"), Some("X")]),
                type_word in prop::sample::select(vec!["Creature", "Artifact", "Land", "Plane", "Scheme"]),
            ) {
                let cost = format!("{}{}{}", "{X}".repeat(x), symbols.concat(), unknown_symbol.unwrap_or(""));
                let mut shown = elf();
                shown.faces[0].mana_cost = cost;
                shown.faces[0].type_line = format!("{type_word} \u{2014} Elf");
                shown.faces[0].power = power.map(str::to_owned);
                shown.faces[0].toughness = toughness.map(str::to_owned);
                let rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
                let caps = Caps::from_names([PLAIN_MANA_ABILITY]);

                let assembled = assemble(&shown, Some(&rules), &caps);

                let reachable = x == 0
                    && unknown_symbol.is_none()
                    && matches!(type_word, "Creature" | "Artifact" | "Land")
                    && matches!(power, None | Some("0" | "7" | "-1"))
                    && matches!(toughness, None | Some("1" | "12"));
                prop_assert_eq!(assembled.support.support == Support::Verified, reachable);
                prop_assert_eq!(assembled.definition.abilities.is_empty(), !reachable);
                if unknown_symbol.is_none() {
                    prop_assert_eq!(usize::from(assembled.definition.x_in_cost), x);
                    prop_assert_eq!(assembled.definition.mana_cost.is_some(), x > 0 || !symbols.is_empty());
                } else {
                    prop_assert_eq!(assembled.definition.mana_cost, None);
                }
                prop_assert_eq!(assembled.definition.power, power.and_then(|p| p.parse().ok()));
                prop_assert_eq!(assembled.definition.toughness, toughness.and_then(|t| t.parse().ok()));
            }

            #[test]
            fn a_number_reads_back_as_the_number(n in -100_000i64..100_000) {
                let text = n.to_string();
                prop_assert!(matches!(read_stat(Some(&text)), Stat::Number(m) if m == n));
            }
        }
    }

    /// The elf with a second line the rules leave to the table, and the rules file that says so.
    fn elf_with_a_clause(status: ImplStatus) -> (PrintedCard, Card) {
        let text = format!("{MANA_TEXT} Draw a card.");
        let mut printed = elf();
        printed.faces[0].oracle_text = text.clone();
        let mut rules = elf_rules(
            status,
            vec![
                ability(MANA_ABILITY),
                Ability::Manual {
                    reason: "draws are not modelled".into(),
                    span: crate::card::Span { start: 14, end: 26 },
                },
            ],
        );
        rules.faces[0].oracle_fingerprint = oracle_fingerprint(&text);
        (printed, rules)
    }

    #[rstest]
    #[case::a_clause_the_rules_name(ImplStatus::Partial, "Draw a card.", "draws are not modelled")]
    #[case::the_whole_text_of_a_card_with_no_rules(
        ImplStatus::Manual,
        "{T}: Add {G}. Draw a card.",
        "the engine runs none of this card's rules"
    )]
    fn the_clauses_the_engine_does_not_run_stay_on_the_definition_as_the_card_prints_them(
        #[case] status: ImplStatus,
        #[case] text: &str,
        #[case] reason: &str,
    ) {
        let (printed, rules) = elf_with_a_clause(status);
        let assembled = assemble(
            &printed,
            Some(&rules),
            &Caps::from_names([PLAIN_MANA_ABILITY]),
        );
        assert_eq!(
            assembled.definition.manual,
            [ManualClause {
                text: text.into(),
                reason: reason.into()
            }]
        );
        let ran = usize::from(status == ImplStatus::Partial);
        assert_eq!(assembled.definition.abilities.len(), ran);
    }

    #[rstest]
    #[case::a_card_the_engine_plays_whole(false)]
    #[case::a_card_with_nothing_but_reminder_text(true)]
    fn a_card_with_no_clause_left_to_the_table_shows_none(#[case] reminder_only: bool) {
        let mut printed = elf();
        let mut rules = elf_rules(ImplStatus::Full, vec![ability(MANA_ABILITY)]);
        if reminder_only {
            let text = "(This is a reminder.)";
            printed.faces[0].oracle_text = text.into();
            rules.faces[0].oracle_fingerprint = oracle_fingerprint(text);
        }
        let assembled = assemble(&printed, None, &Caps::everything());
        let filed = assemble(&printed, Some(&rules), &Caps::everything());
        assert_eq!(filed.definition.manual, Vec::<ManualClause>::new());
        assert_eq!(
            assembled.definition.manual.is_empty(),
            reminder_only,
            "a card with no file and rules text is left to the table whole"
        );
    }

    #[test]
    fn a_definition_with_no_clause_serializes_as_it_always_did() {
        let json = serde_json::to_string(&CardDefinition::new("Elf")).unwrap();
        assert!(!json.contains("manual"), "{json}");
        let clause = CardDefinition::new("Elf").with_manual("Draw a card.", "not modelled");
        assert!(
            serde_json::to_string(&clause)
                .unwrap()
                .contains("Draw a card.")
        );
    }
}
