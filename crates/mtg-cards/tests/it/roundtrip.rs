//! Goal: for generated costs, effects, abilities and their parts, writing a value as JSON (compact or in the canonical layout) and reading it back gives the same value.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use mtg_cards::ability::{Ability, CostChange, Limit, LoyaltyCost, Timing};
use mtg_cards::card::{Span, TokenSpec};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::{Destination, Effect, Mode, Order, PhaseKind, Place, Produce, TokenRef};
use mtg_cards::keyword::Keyword;
use mtg_cards::load::{canonical, parse_json};
use mtg_cards::mana::ManaCostX;
use mtg_cards::modify::{CastPermission, Duration, GameRule, Modification};
use mtg_cards::refs::{AbilityKind, Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_cards::spend::{ManaLasts, SpendFact, SpendFilter, SpendOn};
use mtg_cards::trigger::{EntersChoice, ReplaceBy, ReplaceWhen, Step, TriggerPattern};
use mtg_cards::value::{Amount, CastWay, Cmp, Condition, CounterKind, StatKind};
use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaSymbol, ManaType};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;
use proptest::test_runner::TestRunner;

fn color() -> impl Strategy<Value = Color> {
    select(Color::ALL.to_vec())
}

fn card_type() -> impl Strategy<Value = CardType> {
    select(vec![
        CardType::Artifact,
        CardType::Battle,
        CardType::Creature,
        CardType::Enchantment,
        CardType::Instant,
        CardType::Kindred,
        CardType::Land,
        CardType::Planeswalker,
        CardType::Sorcery,
    ])
}

fn supertype() -> impl Strategy<Value = Supertype> {
    select(vec![
        Supertype::Basic,
        Supertype::Legendary,
        Supertype::Snow,
        Supertype::World,
    ])
}

fn bound_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{0,6}"
}

fn text() -> impl Strategy<Value = String> {
    "[A-Za-z ,'\u{2014}-]{1,12}"
}

fn symbol() -> impl Strategy<Value = ManaSymbol> {
    prop_oneof![
        (0u32..6).prop_map(ManaSymbol::Generic),
        color().prop_map(ManaSymbol::Colored),
        Just(ManaSymbol::Colorless),
        (color(), color()).prop_map(|(a, b)| ManaSymbol::Hybrid(a, b)),
        color().prop_map(ManaSymbol::MonoHybrid),
        color().prop_map(ManaSymbol::Phyrexian),
        (color(), color()).prop_map(|(a, b)| ManaSymbol::HybridPhyrexian(a, b)),
        color().prop_map(ManaSymbol::ColorlessHybrid),
        Just(ManaSymbol::ColorlessPhyrexian),
    ]
}

fn mana_cost() -> impl Strategy<Value = ManaCostX> {
    (0u8..3, vec(symbol(), 0..4))
        .prop_map(|(x, symbols)| ManaCostX::new(x, ManaCost::from_symbols(symbols)))
}

fn zone() -> impl Strategy<Value = ZoneKind> {
    select(vec![
        ZoneKind::Battlefield,
        ZoneKind::Hand,
        ZoneKind::Library,
        ZoneKind::Graveyard,
        ZoneKind::Stack,
        ZoneKind::Exile,
        ZoneKind::Command,
    ])
}

fn cmp() -> impl Strategy<Value = Cmp> {
    select(vec![Cmp::Eq, Cmp::Ne, Cmp::Lt, Cmp::Le, Cmp::Gt, Cmp::Ge])
}

fn stat() -> impl Strategy<Value = StatKind> {
    select(vec![
        StatKind::Power,
        StatKind::Toughness,
        StatKind::ManaValue,
        StatKind::Loyalty,
    ])
}

fn counter() -> impl Strategy<Value = CounterKind> {
    prop_oneof![
        Just(CounterKind::PlusOnePlusOne),
        Just(CounterKind::MinusOneMinusOne),
        Just(CounterKind::Loyalty),
        Just(CounterKind::Poison),
        Just(CounterKind::Charge),
        bound_name().prop_map(CounterKind::Other),
    ]
}

fn leaf_pred() -> impl Strategy<Value = Pred> {
    prop_oneof![
        Just(Pred::This),
        Just(Pred::Other),
        card_type().prop_map(Pred::Type),
        supertype().prop_map(Pred::Supertype),
        text().prop_map(|s| Pred::Subtype(Subtype(s))),
        color().prop_map(Pred::Color),
        Just(Pred::Colorless),
        Just(Pred::Token),
        Just(Pred::Spell),
        Just(Pred::Ability(AbilityKind::Activated)),
        Just(Pred::Ability(AbilityKind::Triggered)),
        Just(Pred::Tapped),
        Just(Pred::Untapped),
        Just(Pred::EnteredThisTurn),
        Just(Pred::Attacking),
        Just(Pred::Blocking),
        zone().prop_map(Pred::In),
        text().prop_map(Pred::Named),
        Just(Pred::ChosenColor),
        Just(Pred::ChosenSubtype),
    ]
}

fn leaf_selector() -> impl Strategy<Value = Selector> {
    vec(leaf_pred(), 0..3).prop_map(Selector)
}

fn leaf_ref() -> impl Strategy<Value = Ref> {
    prop_oneof![
        Just(Ref::You),
        Just(Ref::Opponents),
        Just(Ref::EachPlayer),
        Just(Ref::This),
        (0u8..3).prop_map(Ref::Target),
        Just(Ref::Triggering),
        Just(Ref::Attached),
        bound_name().prop_map(Ref::Bound),
    ]
}

fn simple_amount() -> impl Strategy<Value = Amount> {
    prop_oneof![
        (-50i32..50).prop_map(Amount::Fixed),
        Just(Amount::X),
        leaf_selector().prop_map(Amount::Count),
        (leaf_ref(), stat()).prop_map(|(of, stat)| Amount::Stat { of, stat }),
        leaf_ref().prop_map(Amount::Life),
        (leaf_ref(), counter()).prop_map(|(on, counter)| Amount::Counters { on, counter }),
        Just(Amount::Divided),
        (0u8..3).prop_map(Amount::TargetCount),
        vec(color(), 1..3).prop_map(Amount::Devotion),
        (leaf_selector(), stat()).prop_map(|(of, stat)| Amount::Greatest { of, stat }),
        (leaf_selector(), stat()).prop_map(|(of, stat)| Amount::Total { of, stat }),
        leaf_selector().prop_map(Amount::CardTypes),
        leaf_ref().prop_map(Amount::Players),
        leaf_ref().prop_map(Amount::LowestLife),
        Just(Amount::Kicks),
    ]
}

fn cost_item() -> impl Strategy<Value = CostItem> {
    prop_oneof![
        mana_cost().prop_map(CostItem::Mana),
        Just(CostItem::Tap),
        Just(CostItem::Untap),
        simple_amount().prop_map(CostItem::PayLife),
        Just(CostItem::SacrificeSelf),
        (leaf_selector(), simple_amount())
            .prop_map(|(filter, count)| CostItem::Sacrifice { filter, count }),
        (simple_amount(), proptest::option::of(leaf_selector()))
            .prop_map(|(count, filter)| CostItem::Discard { count, filter }),
        simple_amount().prop_map(CostItem::PayEnergy),
        (
            proptest::option::of(counter()),
            simple_amount(),
            proptest::option::of(leaf_selector())
        )
            .prop_map(|(counter, count, from)| CostItem::RemoveCounters {
                counter,
                count,
                from
            }),
        (leaf_selector(), simple_amount())
            .prop_map(|(filter, count)| CostItem::TapPermanents { filter, count }),
        (leaf_selector(), 0..20u32)
            .prop_map(|(filter, power)| CostItem::TapPower { filter, power }),
        (leaf_selector(), simple_amount())
            .prop_map(|(filter, count)| CostItem::ExileFromGraveyard { filter, count }),
        (leaf_selector(), simple_amount())
            .prop_map(|(filter, count)| CostItem::ReturnToHand { filter, count }),
    ]
}

fn cost() -> impl Strategy<Value = Cost> {
    vec(cost_item(), 0..4).prop_map(Cost)
}

fn keyword() -> impl Strategy<Value = Keyword> {
    prop_oneof![
        select(vec![
            Keyword::Flash,
            Keyword::Flying,
            Keyword::Reach,
            Keyword::FirstStrike,
            Keyword::DoubleStrike,
            Keyword::Deathtouch,
            Keyword::Trample,
            Keyword::Lifelink,
            Keyword::Vigilance,
            Keyword::Haste,
            Keyword::Menace,
            Keyword::Defender,
            Keyword::Indestructible,
            Keyword::Hexproof,
            Keyword::Prowess,
            Keyword::Convoke,
            Keyword::Delve,
            Keyword::Cascade,
            Keyword::Storm,
            Keyword::Partner,
            Keyword::companion(mtg_cards::companion::DeckCondition::ExtraCards(20)),
            Keyword::BattleCry,
            Keyword::Fuse,
            Keyword::Changeling,
            Keyword::Devoid,
            Keyword::Infect,
            Keyword::Myriad,
            Keyword::DoctorsCompanion,
            Keyword::ChooseABackground,
            Keyword::Rebound,
            Keyword::Aftermath,
            Keyword::JumpStart,
            Keyword::Retrace,
            Keyword::Undying,
            Keyword::Persist,
            Keyword::Exalted,
            Keyword::Melee,
            Keyword::Mentor,
            Keyword::Dethrone,
            Keyword::UmbraArmor,
            Keyword::Extort,
            Keyword::Evolve,
        ]),
        (1u32..10).prop_map(Keyword::Crew),
        (1u32..10).prop_map(Keyword::Annihilator),
        cost().prop_map(Keyword::Mutate),
        cost().prop_map(Keyword::Encore),
        cost().prop_map(Keyword::Squad),
        cost().prop_map(Keyword::Unearth),
        cost().prop_map(Keyword::Escape),
        cost().prop_map(Keyword::Embalm),
        cost().prop_map(Keyword::Eternalize),
        cost().prop_map(Keyword::Dash),
        cost().prop_map(Keyword::Bestow),
        cost().prop_map(Keyword::Blitz),
        cost().prop_map(Keyword::Prowl),
        cost().prop_map(Keyword::Mayhem),
        cost().prop_map(Keyword::Disturb),
        cost().prop_map(Keyword::Scavenge),
        cost().prop_map(Keyword::Transmute),
        (1u32..10, cost()).prop_map(|(count, cost)| Keyword::Awaken { count, cost }),
        Just(Keyword::Gravestorm),
        Just(Keyword::Sunburst),
        Just(Keyword::Demonstrate),
        (1u32..9).prop_map(Keyword::Hideaway),
        Just(Keyword::Cipher),
        cost().prop_map(Keyword::Ninjutsu),
        cost().prop_map(Keyword::Foretell),
        cost().prop_map(Keyword::Echo),
        cost().prop_map(Keyword::CumulativeUpkeep),
        cost().prop_map(Keyword::Ward),
        cost().prop_map(Keyword::Equip),
        (leaf_selector(), cost()).prop_map(|(to, cost)| Keyword::EquipTo {
            to: Box::new(to),
            cost
        }),
        cost().prop_map(Keyword::Reconfigure),
        leaf_selector().prop_map(Keyword::Enchant),
        cost().prop_map(Keyword::Cycling),
        cost().prop_map(Keyword::Kicker),
        cost().prop_map(Keyword::Multikicker),
        cost().prop_map(Keyword::Flashback),
        cost().prop_map(Keyword::Madness),
        cost().prop_map(Keyword::Miracle),
        cost().prop_map(Keyword::Morph),
        cost().prop_map(Keyword::Megamorph),
        cost().prop_map(Keyword::Disguise),
        (1u32..6, cost()).prop_map(|(time, cost)| Keyword::Suspend { time, cost }),
        cost().prop_map(Keyword::Evoke),
        cost().prop_map(Keyword::Overload),
        text().prop_map(Keyword::PartnerWith),
        text().prop_map(Keyword::PartnerGroup),
    ]
}

fn pred() -> impl Strategy<Value = Pred> {
    leaf_pred().prop_recursive(2, 12, 3, |inner| {
        prop_oneof![
            inner.clone().prop_map(|p| Pred::Not(Box::new(p))),
            vec(inner, 1..3).prop_map(Pred::AnyOf),
            leaf_ref().prop_map(Pred::ControlledBy),
            leaf_ref().prop_map(Pred::OwnedBy),
            leaf_ref().prop_map(Pred::Is),
            leaf_ref().prop_map(Pred::ExiledWith),
            leaf_ref().prop_map(Pred::EncodedWith),
            leaf_ref().prop_map(Pred::Crewed),
            leaf_ref().prop_map(Pred::Saddled),
            keyword().prop_map(Pred::HasKeyword),
            (stat(), cmp(), simple_amount()).prop_map(|(stat, op, to)| Pred::Compare {
                stat,
                op,
                to
            }),
        ]
    })
}

fn spend_filter() -> impl Strategy<Value = SpendFilter> {
    (
        vec(
            select(vec![
                CardType::Creature,
                CardType::Artifact,
                CardType::Instant,
            ]),
            0..3,
        ),
        vec(select(vec![CardType::Creature, CardType::Land]), 0..2),
        vec(select(vec![Supertype::Legendary, Supertype::Basic]), 0..2),
        vec(select(vec!["Dragon", "Elf"]), 0..2),
        (any::<bool>(), any::<bool>()),
        (
            0u32..8,
            vec(
                select(vec![
                    SpendFact::XInCost,
                    SpendFact::NotOwned,
                    SpendFact::FromGraveyard,
                ]),
                0..3,
            ),
        ),
    )
        .prop_map(
            |(
                types,
                without,
                supertypes,
                subtypes,
                (multicolored, chosen_type),
                (min_value, facts),
            )| SpendFilter {
                types,
                without,
                supertypes,
                subtypes: subtypes
                    .into_iter()
                    .map(|s| Subtype(s.to_owned()))
                    .collect(),
                multicolored,
                chosen_type,
                min_value,
                facts,
            },
        )
}

fn spend_on() -> impl Strategy<Value = SpendOn> {
    prop_oneof![
        spend_filter().prop_map(SpendOn::Spell),
        spend_filter().prop_map(SpendOn::Ability),
        Just(SpendOn::CostWithX),
    ]
}

fn selector() -> impl Strategy<Value = Selector> {
    vec(pred(), 0..3).prop_map(Selector)
}

fn reference() -> impl Strategy<Value = Ref> {
    leaf_ref().prop_recursive(2, 8, 2, |inner| {
        prop_oneof![
            inner.clone().prop_map(|r| Ref::ControllerOf(Box::new(r))),
            inner.prop_map(|r| Ref::OwnerOf(Box::new(r))),
            selector().prop_map(Ref::Each),
        ]
    })
}

fn amount() -> impl Strategy<Value = Amount> {
    simple_amount().prop_recursive(2, 10, 3, |inner| {
        prop_oneof![
            vec(inner.clone(), 1..3).prop_map(Amount::Sum),
            inner.prop_map(|a| Amount::EvenShare(Box::new(a))),
            selector().prop_map(Amount::Count),
            (reference(), stat()).prop_map(|(of, stat)| Amount::Stat { of, stat }),
        ]
    })
}

fn condition() -> impl Strategy<Value = Condition> {
    prop_oneof![
        Just(Condition::Kicked),
        Just(Condition::YourTurn),
        Just(Condition::Performed),
        leaf_ref().prop_map(Condition::Monarch),
        leaf_ref().prop_map(Condition::MonarchAtTurnStart),
        leaf_ref().prop_map(Condition::CitysBlessing),
        leaf_ref().prop_map(Condition::NewlyControlled),
        (leaf_ref(), 1u32..=3).prop_map(|(of, level)| Condition::Level {
            of,
            op: Cmp::Ge,
            level
        }),
        (leaf_ref(), zone()).prop_map(|(who, zone)| Condition::Searched { who, zone }),
    ]
    .prop_recursive(2, 8, 2, |inner| {
        prop_oneof![
            inner.clone().prop_map(|c| Condition::Not(Box::new(c))),
            vec(inner.clone(), 1..3).prop_map(Condition::All),
            vec(inner, 1..3).prop_map(Condition::Any),
            (amount(), cmp(), amount()).prop_map(|(left, op, right)| Condition::Compare {
                left,
                op,
                right
            }),
            (reference(), selector())
                .prop_map(|(subject, selector)| Condition::Matches { subject, selector }),
        ]
    })
}

fn modification() -> impl Strategy<Value = Modification> {
    prop_oneof![
        (amount(), amount())
            .prop_map(|(power, toughness)| Modification::ModifyPT { power, toughness }),
        (amount(), amount())
            .prop_map(|(power, toughness)| Modification::SetPT { power, toughness }),
        Just(Modification::SwitchPT),
        card_type().prop_map(Modification::AddType),
        text().prop_map(|s| Modification::AddSubtype(Subtype(s))),
        vec(color(), 0..3).prop_map(Modification::SetColors),
        keyword().prop_map(Modification::AddKeyword),
        keyword().prop_map(Modification::RemoveKeyword),
        card_type().prop_map(Modification::RemoveType),
        Just(Modification::LoseAbilities),
        granted().prop_map(|ability| Modification::GrantAbility(std::sync::Arc::new(ability))),
        reference().prop_map(Modification::SetController),
        Just(Modification::CantAttack),
        Just(Modification::CantBlock),
        Just(Modification::CantBeBlocked),
        selector().prop_map(Modification::CantBeBlockedBy),
    ]
}

/// An activated or a triggered ability built without the strategy of an effect, which a modification would call back.
fn granted() -> impl Strategy<Value = Ability> {
    let gain = || {
        vec![Effect::GainLife {
            who: Ref::You,
            amount: Amount::Fixed(1),
        }]
    };
    prop_oneof![
        (cost(), any::<bool>()).prop_map(move |(cost, sorcery)| Ability::Activated {
            cost,
            timing: if sorcery {
                Timing::Sorcery
            } else {
                Timing::Instant
            },
            condition: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: gain(),
        }),
        trigger_pattern().prop_map(move |when| Ability::Triggered {
            when,
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: gain(),
        }),
    ]
}

fn duration() -> impl Strategy<Value = Duration> {
    let counted = prop_oneof![
        Just(CounterKind::PlusOnePlusOne),
        Just(CounterKind::Other("vow".to_owned())),
    ]
    .prop_map(|kind| Duration::WhileHasCounter(Box::new(kind)));
    prop_oneof![
        select(vec![
            Duration::UntilEndOfTurn,
            Duration::UntilEndOfCombat,
            Duration::UntilYourNextTurn,
            Duration::UntilEndOfYourNextTurn,
            Duration::WhileSourceOnBattlefield,
            Duration::UntilOpponentBecomesMonarch,
            Duration::Forever,
        ]),
        counted,
    ]
}

fn game_rule() -> impl Strategy<Value = GameRule> {
    prop_oneof![
        (selector(), proptest::option::of(any::<bool>()))
            .prop_map(|(source, combat)| GameRule::DamageCantBePrevented { source, combat }),
        reference().prop_map(GameRule::MustAttack),
        reference().prop_map(GameRule::MustBlock),
        reference().prop_map(GameRule::CantBeCountered),
        reference().prop_map(GameRule::CantGainLife),
        reference().prop_map(GameRule::NoLifeLossFromDamage),
        reference().prop_map(GameRule::CantDraw),
        reference().prop_map(GameRule::CantSearch),
        reference().prop_map(GameRule::RevealTop),
        reference().prop_map(GameRule::LookAtTopAnyTime),
        reference().prop_map(GameRule::NoMaxHandSize),
        (reference(), amount())
            .prop_map(|(who, count)| GameRule::AdditionalLandPlays { who, count }),
        Just(GameRule::CanBeCommander),
        (
            reference(),
            selector(),
            proptest::option::of(zone()),
            any::<bool>(),
            any::<bool>(),
            proptest::option::of(reference()),
            any::<bool>(),
        )
            .prop_map(|(who, what, from, free, flash, only, play)| {
                GameRule::MayCast(CastPermission {
                    who,
                    what,
                    from,
                    free,
                    flash,
                    only,
                    play,
                    once: false,
                    additional: None,
                })
            }),
        trigger_pattern().prop_map(|cause| GameRule::TriggerAgain { cause }),
    ]
}

fn trigger_pattern() -> impl Strategy<Value = TriggerPattern> {
    let leaf = prop_oneof![
        (
            selector(),
            proptest::option::of(zone()),
            proptest::option::of(zone())
        )
            .prop_map(|(what, from, to)| TriggerPattern::ZoneChange { what, from, to }),
        selector().prop_map(|what| TriggerPattern::Attacks { what }),
        (selector(), selector()).prop_map(|(what, by)| TriggerPattern::BecomesTarget { what, by }),
        (selector(), reference()).prop_map(|(what, by)| TriggerPattern::SpellCast { what, by }),
        (selector(), reference()).prop_map(|(what, by)| TriggerPattern::SpellCopied { what, by }),
        (selector(), reference()).prop_map(|(what, by)| TriggerPattern::Sacrificed { what, by }),
        (
            select(vec![
                Step::Untap,
                Step::Upkeep,
                Step::Draw,
                Step::Main1,
                Step::End,
                Step::Cleanup
            ]),
            reference()
        )
            .prop_map(|(step, whose)| TriggerPattern::StepBegins { step, whose }),
        (
            selector(),
            proptest::option::of(reference()),
            proptest::option::of(any::<bool>())
        )
            .prop_map(|(source, to, combat)| TriggerPattern::DealsDamage {
                source,
                to,
                combat
            }),
        reference().prop_map(|who| TriggerPattern::LifeGained { who }),
        reference().prop_map(|who| TriggerPattern::LifeLost { who }),
        (reference(), selector()).prop_map(|(who, what)| TriggerPattern::Drew { who, what }),
        (selector(), reference()).prop_map(|(what, by)| TriggerPattern::Discarded { what, by }),
        selector().prop_map(|what| TriggerPattern::Tapped { what }),
        (selector(), reference()).prop_map(|(what, by)| TriggerPattern::TappedForMana { what, by }),
        selector().prop_map(|what| TriggerPattern::Blocks { what }),
        selector().prop_map(|what| TriggerPattern::BecomesBlocked { what }),
        selector().prop_map(|what| TriggerPattern::Unblocked { what }),
        (selector(), 1u32..5)
            .prop_map(|(what, at_least)| TriggerPattern::GroupAttacks { what, at_least }),
        (selector(), proptest::option::of(counter()))
            .prop_map(|(what, counter)| TriggerPattern::CountersPut { what, counter }),
        selector().prop_map(|what| TriggerPattern::Untapped { what }),
        (selector(), proptest::option::of(counter()))
            .prop_map(|(what, counter)| TriggerPattern::CountersRemoved { what, counter }),
        (1u32..=10).prop_map(|number| TriggerPattern::Chapter { number }),
        (selector(), 2u32..=3)
            .prop_map(|(what, level)| TriggerPattern::BecomesLevel { what, level }),
        (selector(), selector()).prop_map(|(what, to)| TriggerPattern::Attached { what, to }),
        (selector(), reference()).prop_map(|(what, by)| TriggerPattern::Cycled { what, by }),
    ];
    leaf.prop_recursive(2, 6, 2, |inner| {
        vec(inner, 2..4).prop_map(TriggerPattern::AnyOf)
    })
}

fn bounds() -> impl Strategy<Value = Bounds> {
    (amount(), proptest::option::of(amount())).prop_map(|(min, max)| Bounds { min, max })
}

fn target_spec() -> impl Strategy<Value = TargetSpec> {
    let what = prop_oneof![
        Just(Targetable::AnyTarget),
        Just(Targetable::Player),
        Just(Targetable::Opponent),
        selector().prop_map(Targetable::Object),
    ]
    .prop_recursive(1, 4, 2, |inner| {
        vec(inner, 2..3).prop_map(Targetable::Either)
    });
    (what, bounds(), proptest::option::of(amount())).prop_map(|(what, bounds, divide)| TargetSpec {
        what,
        bounds,
        divide,
    })
}

fn destination() -> impl Strategy<Value = Destination> {
    prop_oneof![
        Just(Destination::Hand),
        Just(Destination::Graveyard),
        (
            any::<bool>(),
            proptest::option::of(reference()),
            any::<bool>(),
            proptest::collection::vec((counter(), 1u32..4), 0..3),
        )
            .prop_map(
                |(tapped, under, transformed, counters)| Destination::Battlefield {
                    tapped,
                    under,
                    transformed,
                    counters,
                },
            ),
        (
            select(vec![Place::Top, Place::Bottom, Place::Shuffled]),
            proptest::option::of(select(vec![Order::Chosen, Order::Random])),
        )
            .prop_map(|(place, order)| Destination::Library { place, order }),
    ]
}

fn token_ref() -> impl Strategy<Value = TokenRef> {
    prop_oneof![
        bound_name().prop_map(TokenRef::Named),
        (
            text(),
            vec(color(), 0..2),
            vec(card_type(), 1..3),
            vec(keyword(), 0..2)
        )
            .prop_map(|(name, colors, types, keywords)| {
                TokenRef::Custom(Box::new(TokenSpec {
                    name,
                    colors,
                    supertypes: vec![],
                    types,
                    subtypes: vec![],
                    power: Some(1),
                    toughness: Some(1),
                    abilities: keywords.into_iter().map(Ability::Keyword).collect(),
                }))
            }),
        (reference(), vec(modification(), 0..2)).prop_map(|(of, except)| TokenRef::CopyOf {
            of,
            except,
            no_mana_cost: false,
        }),
        (reference(), any::<bool>()).prop_map(|(of, no_mana_cost)| TokenRef::CopyOf {
            of,
            except: vec![],
            no_mana_cost,
        }),
    ]
}

fn leaf_effect() -> impl Strategy<Value = Effect> {
    let who = || reference();
    let part1 = prop_oneof![
        (who(), amount()).prop_map(|(who, count)| Effect::Draw { who, count }),
        (who(), amount(), any::<bool>()).prop_map(|(who, count, random)| Effect::Discard {
            who,
            count,
            random,
            bind: None
        }),
        (who(), amount()).prop_map(|(who, count)| Effect::Mill { who, count }),
        (who(), amount()).prop_map(|(who, count)| Effect::Scry { who, count }),
        (who(), amount(), bound_name()).prop_map(|(who, count, bind)| Effect::LookAtTop {
            who,
            count,
            bind
        }),
        reference().prop_map(|what| Effect::Reveal { what }),
        (
            who(),
            zone(),
            proptest::collection::vec(zone(), 0..3),
            selector(),
            bounds(),
            any::<bool>(),
            bound_name()
        )
            .prop_map(
                |(who, zone, also, filter, count, reveal, bind)| Effect::Search {
                    who,
                    zone,
                    also,
                    filter,
                    count,
                    reveal,
                    bind
                }
            ),
        (
            who(),
            reference(),
            bounds(),
            bound_name(),
            proptest::option::of(bound_name())
        )
            .prop_map(|(who, from, count, bind, rest_as)| Effect::Choose {
                who,
                from,
                count,
                bind,
                rest_as
            }),
        who().prop_map(|who| Effect::Shuffle { who }),
        (reference(), destination()).prop_map(|(what, to)| Effect::Move { what, to }),
        (reference(), any::<bool>())
            .prop_map(|(what, regenerate)| Effect::Destroy { what, regenerate }),
        (
            reference(),
            proptest::option::of("[a-z]{1,6}"),
            proptest::option::of(duration())
        )
            .prop_map(|(what, bind, duration)| Effect::Exile {
                what,
                bind,
                duration
            }),
        reference().prop_map(|what| Effect::ExileHidden { what }),
        (reference(), reference()).prop_map(|(what, on)| Effect::Encode { what, on }),
        (who(), reference(), "[a-z]{1,6}").prop_map(|(who, what, bind)| Effect::CopyCard {
            who,
            what,
            bind
        }),
        (who(), selector(), amount()).prop_map(|(who, filter, count)| Effect::Sacrifice {
            who,
            filter,
            count
        }),
        reference().prop_map(|what| Effect::Counter { what }),
    ];
    let part2 = prop_oneof![
        (proptest::option::of(reference()), reference(), amount())
            .prop_map(|(source, to, amount)| Effect::DealDamage { source, to, amount }),
        (who(), amount()).prop_map(|(who, amount)| Effect::GainLife { who, amount }),
        (who(), amount()).prop_map(|(who, amount)| Effect::LoseLife { who, amount }),
        (
            who(),
            prop_oneof![
                vec(
                    select(vec![ManaType::White, ManaType::Colorless, ManaType::Green]),
                    1..3
                )
                .prop_map(Produce::Fixed),
                amount().prop_map(Produce::AnyColor),
                (
                    select(vec![ManaType::Green, ManaType::Colorless]),
                    condition(),
                    select(vec![ManaType::Black, ManaType::Colorless])
                )
                    .prop_map(|(base, when, instead)| Produce::Instead {
                        base: Box::new(Produce::Fixed(vec![base])),
                        when,
                        instead: Box::new(Produce::Fixed(vec![instead, instead])),
                    }),
                vec(
                    select(vec![ManaType::White, ManaType::Blue, ManaType::Colorless]),
                    2..4
                )
                .prop_map(Produce::OneOf),
                vec(
                    vec(
                        select(vec![ManaType::White, ManaType::Blue, ManaType::Colorless]),
                        1..3
                    ),
                    2..4
                )
                .prop_map(Produce::OneOfLists),
                (1u32..3).prop_map(Produce::ChosenColor),
                select(vec![ManaType::Red, ManaType::Colorless]).prop_map(Produce::OrChosenColor),
                (1u32..3).prop_map(Produce::CommanderIdentity),
                (selector(), 1u32..3, any::<bool>()).prop_map(|(among, count, colorless)| {
                    Produce::ProducedBy {
                        among,
                        count,
                        colorless,
                    }
                }),
            ],
            amount(),
            vec(spend_on(), 0..3),
            select(vec![ManaLasts::Step, ManaLasts::Combat, ManaLasts::Turn])
        )
            .prop_map(|(who, mana, times, only, lasts)| Effect::AddMana {
                who,
                mana,
                times,
                only,
                lasts
            }),
        (reference(), counter(), amount()).prop_map(|(on, counter, count)| Effect::PutCounters {
            on,
            counter,
            count
        }),
        (reference(), proptest::option::of(counter()), 2u32..6).prop_map(
            |(on, counter, factor)| Effect::MultiplyCounters {
                on,
                counter,
                factor
            }
        ),
        reference().prop_map(|what| Effect::Tap { what }),
        reference().prop_map(|what| Effect::Untap { what }),
        reference().prop_map(|what| Effect::TapOrUntap { what }),
        (reference(), reference()).prop_map(|(what, to)| Effect::Attach { what, to }),
        reference().prop_map(|what| Effect::Transform { what }),
        who().prop_map(|who| Effect::WinGame { who }),
        (who(), amount()).prop_map(|(who, count)| Effect::Surveil { who, count }),
        who().prop_map(|who| Effect::Proliferate { who }),
        reference().prop_map(|what| Effect::Goad { what }),
        (
            who(),
            reference(),
            amount(),
            any::<bool>(),
            vec(modification(), 0..2)
        )
            .prop_map(|(who, what, count, new_targets, except)| {
                Effect::CopySpell {
                    who,
                    what,
                    count,
                    new_targets,
                    except,
                }
            }),
        (reference(), amount(), any::<bool>()).prop_map(|(what, count, new_targets)| {
            Effect::CopyAbility {
                what,
                count,
                new_targets,
            }
        }),
        (
            reference(),
            reference(),
            vec(modification(), 0..2),
            duration()
        )
            .prop_map(|(what, of, except, duration)| Effect::BecomeCopy {
                what,
                of,
                except,
                duration
            }),
        (reference(), reference()).prop_map(|(what, with)| Effect::Fight { what, with }),
        who().prop_map(|who| Effect::BecomeMonarch { who }),
        (text(), amount()).prop_map(|(subtype, count)| Effect::Amass {
            subtype: Subtype(subtype),
            count
        }),
        amount().prop_map(|count| Effect::Monstrosity { count }),
        (2u32..=3).prop_map(|level| Effect::BecomeLevel { level }),
        amount().prop_map(|count| Effect::Adapt { count }),
        amount().prop_map(|count| Effect::Bolster { count }),
        reference().prop_map(|what| Effect::Explore { what }),
        reference().prop_map(|what| Effect::Connive { what }),
        (who(), selector(), bound_name(), bound_name()).prop_map(
            |(who, filter, bind, exiled_as)| Effect::ExileUntil {
                who,
                filter,
                bind,
                exiled_as
            }
        ),
        (who(), selector(), amount(), bound_name(), bound_name()).prop_map(
            |(who, filter, count, bind, revealed_as)| Effect::RevealUntil {
                who,
                filter,
                count,
                bind,
                revealed_as
            }
        ),
        (who(), amount(), bound_name()).prop_map(|(who, count, bind)| Effect::ExileTop {
            who,
            count,
            bind
        }),
        (
            who(),
            amount(),
            proptest::option::of(who()),
            proptest::option::of(bound_name()),
            any::<bool>()
        )
            .prop_map(|(who, count, from, bind, cloak)| if cloak {
                Effect::Cloak {
                    who,
                    count,
                    from,
                    bind,
                }
            } else {
                Effect::Manifest {
                    who,
                    count,
                    from,
                    bind,
                }
            }),
        who().prop_map(|who| Effect::TimeTravel { who }),
        (who(), amount(), any::<bool>()).prop_map(|(who, count, skip_untap)| Effect::ExtraTurn {
            who,
            count,
            skip_untap
        }),
        prop::collection::vec(
            prop_oneof![
                Just(PhaseKind::Beginning),
                Just(PhaseKind::Combat),
                Just(PhaseKind::Main)
            ],
            1..4
        )
        .prop_map(|phases| Effect::AddPhase { phases }),
        (who(), token_ref(), amount(), any::<bool>(), any::<bool>()).prop_map(
            |(who, token, count, tapped, attacking)| Effect::CreateToken {
                who,
                token,
                count,
                tapped,
                attacking,
                bind: None
            }
        ),
        (reference(), vec(modification(), 1..3), duration()).prop_map(
            |(affected, mods, duration)| Effect::Continuous {
                affected,
                mods,
                duration
            }
        ),
        (game_rule(), duration()).prop_map(|(rule, duration)| Effect::Rule { rule, duration }),
        reference().prop_map(|what| Effect::Regenerate { what }),
        (
            proptest::option::of(reference()),
            selector(),
            proptest::option::of(any::<bool>()),
            proptest::option::of(1u32..6),
            duration()
        )
            .prop_map(|(to, source, combat, most, duration)| Effect::Shield {
                when: ReplaceWhen::Damage { to, source, combat },
                by: most.map_or(ReplaceBy::Prevent, ReplaceBy::PreventUpTo),
                duration
            }),
    ];
    prop_oneof![part1, part2]
}

fn effect() -> impl Strategy<Value = Effect> {
    leaf_effect().prop_recursive(3, 24, 3, |inner| {
        let list = || vec(inner.clone(), 0..3);
        prop_oneof![
            (condition(), list(), list()).prop_map(|(condition, then, otherwise)| Effect::If {
                condition,
                then,
                otherwise
            }),
            (reference(), list()).prop_map(|(who, effects)| Effect::May { who, effects }),
            (
                reference(),
                reference(),
                any::<bool>(),
                any::<bool>(),
                list(),
                proptest::option::of(select(vec![
                    CastWay::Madness,
                    CastWay::Miracle,
                    CastWay::Suspend
                ])),
            )
                .prop_map(|(who, what, free, play, otherwise, way)| {
                    Effect::CastFromZone {
                        who,
                        what,
                        free,
                        play,
                        way,
                        bind: None,
                        otherwise,
                    }
                }),
            (reference(), cost(), amount(), list(), list()).prop_map(
                |(who, cost, times, paid, unpaid)| {
                    Effect::Pay {
                        who,
                        cost,
                        times,
                        paid,
                        unpaid,
                    }
                }
            ),
            (trigger_pattern(), any::<bool>(), list()).prop_map(|(when, once, effects)| {
                Effect::DelayedTrigger {
                    when,
                    once,
                    this_turn: false,
                    effects,
                }
            }),
            (
                bounds(),
                any::<bool>(),
                vec(
                    (
                        proptest::option::of(text()),
                        vec(target_spec(), 0..2),
                        list()
                    ),
                    1..3
                )
            )
                .prop_map(|(choose, repeat, modes)| Effect::Modal {
                    choose,
                    repeat,
                    at_random: false,
                    once_each_turn: false,
                    once_ever: false,
                    all_if: None,
                    modes: modes
                        .into_iter()
                        .map(|(label, targets, effects)| Mode {
                            label,
                            targets,
                            effects
                        })
                        .collect(),
                }),
        ]
    })
}

fn ability() -> impl Strategy<Value = Ability> {
    let targets = || vec(target_spec(), 0..2);
    let effects = || vec(effect(), 1..3);
    let limit = || proptest::option::of(Just(Limit::OncePerTurn));
    let tail = || (proptest::option::of(condition()), zone());
    prop_oneof![
        (targets(), effects()).prop_map(|(targets, effects)| Ability::Spell { targets, effects }),
        (cost(), any::<bool>(), limit(), zone(), targets(), effects()).prop_map(
            |(cost, sorcery, limit, from, targets, effects)| Ability::Activated {
                cost,
                timing: if sorcery {
                    Timing::Sorcery
                } else {
                    Timing::Instant
                },
                condition: None,
                limit,
                from,
                targets,
                effects
            }
        ),
        (
            prop_oneof![
                (0u32..4).prop_map(LoyaltyCost::Plus),
                (0u32..13).prop_map(LoyaltyCost::Minus),
                Just(LoyaltyCost::Zero),
                Just(LoyaltyCost::MinusX),
            ],
            targets(),
            effects()
        )
            .prop_map(|(cost, targets, effects)| Ability::Loyalty {
                cost,
                targets,
                effects
            }),
        (trigger_pattern(), tail(), limit(), targets(), effects()).prop_map(
            |(when, (intervening_if, from), limit, targets, effects)| Ability::Triggered {
                when,
                intervening_if,
                limit,
                from,
                targets,
                effects
            }
        ),
        (game_rule(), tail()).prop_map(|(rule, (condition, from))| Ability::Rule {
            rule,
            condition,
            from
        }),
        (
            reference(),
            vec(modification(), 1..3),
            tail(),
            any::<bool>()
        )
            .prop_map(
                |(affected, mods, (condition, from), defining)| Ability::Static {
                    affected,
                    mods,
                    condition,
                    defining,
                    from
                }
            ),
        (
            prop_oneof![
                reference().prop_map(|controller| ReplaceWhen::TokenCreation {
                    controller,
                    what: Selector::default()
                }),
                (
                    proptest::option::of(reference()),
                    selector(),
                    proptest::option::of(any::<bool>())
                )
                    .prop_map(|(to, source, combat)| ReplaceWhen::Damage {
                        to,
                        source,
                        combat
                    }),
                (
                    proptest::option::of(reference()),
                    proptest::option::of(counter())
                )
                    .prop_map(|(on, counter)| ReplaceWhen::CountersPut { on, counter }),
                reference().prop_map(|who| ReplaceWhen::LifeGain { who }),
                (reference(), 0..3u32).prop_map(|(who, except_draw_step)| ReplaceWhen::Draw {
                    who,
                    except_draw_step
                }),
                selector().prop_map(|what| ReplaceWhen::Enters { what }),
                selector().prop_map(|what| ReplaceWhen::Discarded { what }),
                selector().prop_map(|what| ReplaceWhen::Destroyed { what }),
                (
                    selector(),
                    proptest::option::of(zone()),
                    proptest::option::of(zone())
                )
                    .prop_map(|(what, from, to)| ReplaceWhen::ZoneChange {
                        what,
                        from,
                        to
                    }),
            ],
            prop_oneof![
                (1u32..4).prop_map(ReplaceBy::Multiply),
                (1u32..4).prop_map(ReplaceBy::Add),
                Just(ReplaceBy::Prevent),
                (1u32..6).prop_map(ReplaceBy::PreventUpTo),
                effects().prop_map(ReplaceBy::Instead),
                zone().prop_map(ReplaceBy::MoveTo),
                Just(ReplaceBy::Tapped),
                (counter(), amount(), any::<bool>()).prop_map(|(counter, count, tapped)| {
                    ReplaceBy::WithCounters {
                        counter,
                        count,
                        tapped,
                    }
                }),
                select(vec![
                    EntersChoice::Color,
                    EntersChoice::CreatureType,
                    EntersChoice::Player,
                    EntersChoice::Opponent
                ])
                .prop_map(ReplaceBy::Choose),
                color().prop_map(|other| ReplaceBy::Choose(EntersChoice::ColorOtherThan(other))),
                (1u32..5).prop_map(ReplaceBy::PayLifeOrTapped),
                Just(ReplaceBy::UmbraArmor),
                (
                    selector(),
                    proptest::collection::vec(modification(), 0..3),
                    any::<bool>(),
                    any::<bool>()
                )
                    .prop_map(|(of, except, tapped, until_end_of_turn)| {
                        ReplaceBy::AsCopy {
                            of,
                            except,
                            tapped,
                            until_end_of_turn,
                        }
                    }),
            ],
            tail()
        )
            .prop_map(|(when, by, (condition, from))| Ability::Replacement {
                when,
                by,
                condition,
                from
            }),
        (
            selector(),
            select(vec![CostChange::Increase, CostChange::Reduce]),
            mana_cost(),
            amount(),
            proptest::option::of(condition())
        )
            .prop_map(
                |(applies_to, change, mana, per, condition)| Ability::CostModifier {
                    applies_to,
                    change,
                    mana,
                    per,
                    condition
                }
            ),
        keyword().prop_map(Ability::Keyword),
        (text(), 0u32..40, 1u32..40).prop_map(|(reason, start, length)| Ability::Manual {
            reason,
            span: Span {
                start,
                end: start + length
            }
        }),
    ]
}

/// Compact JSON and the canonical layout both read back to the value.
fn assert_round_trips<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let compact = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&compact).unwrap();
    assert_eq!(&back, value, "compact: {compact}");
    let laid_out = canonical(value);
    let back: T =
        parse_json("generated.json", &laid_out).unwrap_or_else(|e| panic!("{e}\n{laid_out}"));
    assert_eq!(&back, value, "canonical: {laid_out}");
    assert_eq!(
        canonical(&back),
        laid_out,
        "the canonical layout is a fixed point"
    );
}

/// Runs a property on a thread with a large stack: generated values nest deeply and debug builds use big frames.
fn check<S, F>(cases: u32, make: fn() -> S, test: F)
where
    S: Strategy + 'static,
    S::Value: std::fmt::Debug,
    F: Fn(S::Value) + Send + 'static,
{
    let worker = std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || {
            let mut runner = TestRunner::new(ProptestConfig::with_cases(cases));
            if let Err(failure) = runner.run(&make(), |value| {
                test(value);
                Ok(())
            }) {
                panic!("{failure}");
            }
        })
        .unwrap();
    if let Err(panic) = worker.join() {
        std::panic::resume_unwind(panic);
    }
}

#[test]
fn mana_costs_with_x_round_trip() {
    check(256, mana_cost, |cost| {
        assert_round_trips(&cost);
        assert_eq!(cost.to_string().parse::<ManaCostX>().unwrap(), cost);
    });
}

#[test]
fn costs_round_trip() {
    check(256, cost, |cost| assert_round_trips(&cost));
}

#[test]
fn amounts_selectors_and_conditions_round_trip() {
    check(
        128,
        || {
            (
                amount(),
                selector(),
                condition(),
                modification(),
                trigger_pattern(),
            )
        },
        |(amount, selector, condition, modification, pattern)| {
            assert_round_trips(&amount);
            assert_round_trips(&selector);
            assert_round_trips(&condition);
            assert_round_trips(&modification);
            assert_round_trips(&pattern);
        },
    );
}

#[test]
fn effects_round_trip() {
    check(256, effect, |effect| assert_round_trips(&effect));
}

#[test]
fn abilities_round_trip() {
    check(128, ability, |ability| assert_round_trips(&ability));
}
