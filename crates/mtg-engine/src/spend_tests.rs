//! Goal: restricted mana counts toward a payment only where its restriction allows it, and `can_afford`, `auto_payment`, `check_payment` and `complete_payment` agree with a brute-force check of every payment for pools of unrestricted and restricted mana, sources of restricted mana, plain, hybrid and Phyrexian costs, and every kind of spend.

use mtg_cards::spend::{SpendFilter, SpendOn, SpendTraits};
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaType};
use proptest::prelude::*;
use rstest::rstest;

use crate::error::PaymentError;
use crate::mana::{
    ManaOption, ManaPool, ManaSource, Payment, auto_payment, can_afford, check_payment,
    complete_payment,
};
use crate::spend::{Restricted, Spend, SpendKind};

const LIFE: i32 = 20;

fn spell_of(types: &[CardType], has_x: bool) -> Spend {
    Spend {
        kind: SpendKind::Spell,
        traits: SpendTraits {
            types: types.iter().copied().collect(),
            ..SpendTraits::default()
        },
        has_x,
    }
}

fn ability(has_x: bool) -> Spend {
    Spend {
        kind: SpendKind::Ability,
        traits: SpendTraits::default(),
        has_x,
    }
}

fn only_for(card_type: CardType) -> Vec<SpendOn> {
    vec![SpendOn::Spell(SpendFilter::of_type(card_type))]
}

fn cost(text: &str) -> ManaCost {
    ManaCost::parse(text).unwrap()
}

fn pool(plain: &[ManaType], restricted: &[(ManaType, Vec<SpendOn>)]) -> ManaPool {
    let mut pool = ManaPool::default();
    for mana in plain {
        pool.add(*mana);
    }
    for (mana, only) in restricted {
        pool.add_restricted(*mana, only.clone());
    }
    pool
}

fn tap(object: u64) -> ManaSource {
    ManaSource {
        object: ObjectId(object),
        ability: 0,
    }
}

#[rstest]
#[case::unrestricted_anywhere(&[], spell_of(&[CardType::Instant], false), true)]
#[case::unrestricted_for_neither(&[], Spend::other(), true)]
#[case::the_kind_of_spell(&[SpendOn::Spell(SpendFilter::of_type(CardType::Creature))], spell_of(&[CardType::Creature], false), true)]
#[case::another_kind_of_spell(&[SpendOn::Spell(SpendFilter::of_type(CardType::Creature))], spell_of(&[CardType::Artifact], false), false)]
#[case::a_spell_is_not_an_ability(&[SpendOn::Spell(SpendFilter::default())], ability(false), false)]
#[case::an_ability_is_not_a_spell(&[SpendOn::abilities()], spell_of(&[CardType::Creature], false), false)]
#[case::an_ability(&[SpendOn::abilities()], ability(false), true)]
#[case::either_of_two_uses(&[SpendOn::abilities(), SpendOn::Spell(SpendFilter::of_type(CardType::Land))], spell_of(&[CardType::Land], false), true)]
#[case::a_cost_with_x_of_a_spell(&[SpendOn::CostWithX], spell_of(&[CardType::Instant], true), true)]
#[case::a_cost_with_x_of_an_ability(&[SpendOn::CostWithX], ability(true), true)]
#[case::a_cost_without_x(&[SpendOn::CostWithX], spell_of(&[CardType::Instant], false), false)]
#[case::x_of_neither(&[SpendOn::CostWithX], Spend { has_x: true, ..Spend::other() }, false)]
#[case::restricted_for_neither(&[SpendOn::Spell(SpendFilter::default())], Spend::other(), false)]
fn a_spend_allows_what_a_restriction_lists(
    #[case] only: &[SpendOn],
    #[case] spend: Spend,
    #[case] allowed: bool,
) {
    assert_eq!(spend.allows(only), allowed);
}

#[rstest]
#[case::the_kind(only_for(CardType::Creature), CardType::Creature, true)]
#[case::another_kind(only_for(CardType::Creature), CardType::Instant, false)]
fn restricted_mana_in_the_pool_pays_only_what_it_allows(
    #[case] only: Vec<SpendOn>,
    #[case] spell: CardType,
    #[case] pays: bool,
) {
    let held = pool(&[], &[(ManaType::Red, only.clone())]);
    let spend = spell_of(&[spell], false);
    assert_eq!(can_afford(&cost("{R}"), &held, &[], LIFE, &spend), pays);
    assert_eq!(
        auto_payment(&cost("{R}"), &held, &[], LIFE, &spend).is_some(),
        pays
    );
    let unit = Restricted {
        mana: ManaType::Red,
        only,
    };
    let by_hand = Payment {
        restricted: vec![unit.clone()],
        ..Payment::default()
    };
    let checked = check_payment(&cost("{R}"), &held, &[], &[], LIFE, &spend, &by_hand);
    if pays {
        assert_eq!(checked.unwrap().restricted, [unit]);
    } else {
        assert_eq!(checked, Err(PaymentError::Restricted(ManaType::Red)));
    }
}

#[test]
fn restricted_mana_is_spent_before_mana_without_a_restriction() {
    let held = pool(
        &[ManaType::Green, ManaType::Red],
        &[(ManaType::Green, only_for(CardType::Creature))],
    );
    let creature = spell_of(&[CardType::Creature], false);
    let payment = auto_payment(&cost("{G}"), &held, &[], LIFE, &creature).unwrap();
    assert_eq!(payment.pool, Vec::<ManaType>::new());
    assert_eq!(payment.restricted.len(), 1);
    // Generic mana is taken from the kind the restricted mana is.
    let held = pool(
        &[ManaType::White],
        &[(ManaType::Blue, only_for(CardType::Creature))],
    );
    let payment = auto_payment(&cost("{1}"), &held, &[], LIFE, &creature).unwrap();
    assert_eq!((payment.pool.len(), payment.restricted.len()), (0, 1));
}

#[test]
fn a_source_of_restricted_mana_is_tapped_before_one_without() {
    let mut ziggurat = ManaOption::new(tap(1), ManaType::Green);
    ziggurat.only = only_for(CardType::Creature);
    let forest = ManaOption::new(tap(2), ManaType::Green);
    let sources = [forest, ziggurat];
    let none = ManaPool::default();
    let creature = spell_of(&[CardType::Creature], false);
    let payment = auto_payment(&cost("{G}"), &none, &sources, LIFE, &creature).unwrap();
    assert_eq!(payment.taps, [tap(1)]);
    let instant = spell_of(&[CardType::Instant], false);
    let payment = auto_payment(&cost("{G}"), &none, &sources, LIFE, &instant).unwrap();
    assert_eq!(payment.taps, [tap(2)]);
    let named = Payment {
        taps: vec![tap(1)],
        ..Payment::default()
    };
    assert_eq!(
        check_payment(&cost("{G}"), &none, &sources, &[], LIFE, &instant, &named),
        Err(PaymentError::Restricted(ManaType::Green))
    );
}

#[test]
fn a_source_of_three_restricted_mana_leaves_the_surplus_restricted() {
    let mut workshop = ManaOption::new(tap(1), ManaType::Colorless);
    workshop.count = 3;
    workshop.only = only_for(CardType::Artifact);
    let by_hand = Payment {
        taps: vec![tap(1)],
        ..Payment::default()
    };
    let spent = check_payment(
        &cost("{1}"),
        &ManaPool::default(),
        &[workshop],
        &[],
        LIFE,
        &spell_of(&[CardType::Artifact], false),
        &by_hand,
    )
    .unwrap();
    assert_eq!(spent.mana, [ManaType::Colorless]);
    assert_eq!(spent.restricted.len(), 1);
}

#[test]
fn restricted_mana_beside_hybrid_and_phyrexian_symbols_wastes_least() {
    let held = pool(
        &[ManaType::White],
        &[(ManaType::Blue, only_for(CardType::Creature))],
    );
    let creature = spell_of(&[CardType::Creature], false);
    let instant = spell_of(&[CardType::Instant], false);
    let hybrid = cost("{W/U}{W/U}");
    let payment = auto_payment(&hybrid, &held, &[], LIFE, &creature).unwrap();
    assert_eq!((payment.pool.len(), payment.restricted.len()), (1, 1));
    assert!(!can_afford(&hybrid, &held, &[], LIFE, &instant));
    // A Phyrexian symbol is paid with restricted mana where it is allowed, and with life where it is not.
    let held = pool(&[], &[(ManaType::Blue, only_for(CardType::Creature))]);
    let phyrexian = cost("{U/P}");
    let payment = auto_payment(&phyrexian, &held, &[], LIFE, &creature).unwrap();
    assert_eq!((payment.life, payment.restricted.len()), (0, 1));
    let payment = auto_payment(&phyrexian, &held, &[], LIFE, &instant).unwrap();
    assert_eq!((payment.life, payment.restricted.len()), (2, 0));
}

#[test]
fn a_cost_with_x_is_paid_with_mana_restricted_to_costs_with_x() {
    let unit = (ManaType::Colorless, vec![SpendOn::CostWithX]);
    let held = pool(&[], &[unit.clone(), unit.clone(), unit.clone(), unit]);
    let with_x = spell_of(&[CardType::Instant], true);
    let without = spell_of(&[CardType::Instant], false);
    assert!(can_afford(&cost("{4}"), &held, &[], LIFE, &with_x));
    assert!(!can_afford(&cost("{1}"), &held, &[], LIFE, &without));
    let partial = Payment {
        restricted: vec![Restricted {
            mana: ManaType::Colorless,
            only: vec![SpendOn::CostWithX],
        }],
        ..Payment::default()
    };
    let whole = complete_payment(&cost("{3}"), &held, &[], &[], LIFE, &with_x, &partial).unwrap();
    assert_eq!(whole.restricted.len(), 3);
    assert_eq!(whole.restricted[0], partial.restricted[0]);
}

/// One mana a payment may name, told apart the way the brute-force check does.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Plain(usize),
    Restricted(usize),
    Tap(usize),
}

const TYPES: [ManaType; 4] = [
    ManaType::White,
    ManaType::Blue,
    ManaType::Red,
    ManaType::Colorless,
];

fn restrictions() -> [Vec<SpendOn>; 3] {
    [
        only_for(CardType::Creature),
        vec![SpendOn::abilities()],
        vec![SpendOn::CostWithX],
    ]
}

fn spends() -> [Spend; 5] {
    [
        spell_of(&[CardType::Creature], false),
        spell_of(&[CardType::Instant], true),
        ability(false),
        ability(true),
        Spend::other(),
    ]
}

/// Floating mana without a restriction (by type), restricted floating mana (type, restriction), sources (type,
/// restriction or none), a cost and a spend.
type Case = (
    Vec<usize>,
    Vec<(usize, usize)>,
    Vec<(usize, Option<usize>)>,
    &'static str,
    usize,
);

fn case() -> impl Strategy<Value = Case> {
    (
        prop::collection::vec(0usize..4, 0..3),
        prop::collection::vec((0usize..4, 0usize..3), 0..3),
        prop::collection::vec((0usize..4, prop::option::of(0usize..3)), 0..3),
        prop::sample::select(vec![
            "{1}",
            "{2}",
            "{W}",
            "{U}{1}",
            "{R}{R}",
            "{W}{U}",
            "{2}{C}",
            "{W/U}",
            "{W/U}{R}",
            "{U/P}",
            "{R/P}{1}",
            "{2/W}{U}",
            "{W/U}{W/U}{1}",
            "{C}",
        ]),
        0usize..5,
    )
}

struct World {
    pool: ManaPool,
    sources: Vec<ManaOption>,
    spend: Spend,
    cost: ManaCost,
    /// What a payment may name, with what each makes.
    parts: Vec<(Part, ManaType)>,
}

fn world((plain, restricted, taps, cost_text, spend): &Case) -> World {
    let restrictions = restrictions();
    let spend = spends()[*spend].clone();
    let mut held = ManaPool::default();
    let mut parts = Vec::new();
    for (at, mana) in plain.iter().enumerate() {
        held.add(TYPES[*mana]);
        parts.push((Part::Plain(at), TYPES[*mana]));
    }
    for (at, (mana, only)) in restricted.iter().enumerate() {
        held.add_restricted(TYPES[*mana], restrictions[*only].clone());
        if spend.allows(&restrictions[*only]) {
            parts.push((Part::Restricted(at), TYPES[*mana]));
        }
    }
    let mut sources = Vec::new();
    for (at, (mana, only)) in taps.iter().enumerate() {
        let mut option = ManaOption::new(tap(at as u64), TYPES[*mana]);
        option.only = only.map(|i| restrictions[i].clone()).unwrap_or_default();
        if spend.allows(&option.only) {
            parts.push((Part::Tap(at), TYPES[*mana]));
        }
        sources.push(option);
    }
    World {
        pool: held,
        sources,
        spend,
        cost: cost(cost_text),
        parts,
    }
}

/// The payment that names these parts.
fn payment_of(world: &World, case: &Case, chosen: &[&Part], life: u32) -> Payment {
    let restrictions = restrictions();
    let mut payment = Payment {
        life,
        ..Payment::default()
    };
    for part in chosen {
        match part {
            Part::Plain(at) => payment.pool.push(TYPES[case.0[*at]]),
            Part::Restricted(at) => {
                let (mana, only) = case.1[*at];
                payment.restricted.push(Restricted {
                    mana: TYPES[mana],
                    only: restrictions[only].clone(),
                });
            }
            Part::Tap(at) => payment.taps.push(world.sources[*at].source),
        }
    }
    payment
}

/// Whether the mana made pays the cost exactly, with `life` paid for Phyrexian symbols: some way to pay each symbol takes
/// that life, and the mana is the rest of the cost's value and covers each colour it needs.
fn pays(world: &World, made: &[ManaType], life: u32) -> bool {
    world.cost.resolutions().iter().any(|r| {
        r.life == life
            && made.len() == r.cost.mana_value() as usize
            && ManaType::ALL.into_iter().all(|mana| {
                made.iter().filter(|m| **m == mana).count() >= r.cost.required(mana) as usize
            })
    })
}

/// Every subset of the parts a payment may name.
fn subsets(world: &World) -> Vec<Vec<usize>> {
    (0..1usize << world.parts.len())
        .map(|mask| {
            (0..world.parts.len())
                .filter(|bit| mask >> bit & 1 == 1)
                .collect()
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn the_four_functions_agree_with_a_check_of_every_payment(case in case()) {
        let world = world(&case);
        let lives: Vec<u32> = (0..=world.cost.life_symbols()).map(|n| n * 2).collect();
        let mut valid: Vec<(Vec<usize>, u32)> = Vec::new();
        for subset in subsets(&world) {
            let made: Vec<ManaType> = subset.iter().map(|at| world.parts[*at].1).collect();
            let chosen: Vec<&Part> = subset.iter().map(|at| &world.parts[*at].0).collect();
            for life in &lives {
                let payment = payment_of(&world, &case, &chosen, *life);
                let engine = check_payment(&world.cost, &world.pool, &world.sources, &[], LIFE, &world.spend, &payment);
                let expected = pays(&world, &made, *life);
                prop_assert_eq!(engine.is_ok(), expected, "{:?} for life {}", chosen, life);
                if expected {
                    valid.push((subset.clone(), *life));
                }
            }
        }
        let affordable = can_afford(&world.cost, &world.pool, &world.sources, LIFE, &world.spend);
        prop_assert_eq!(affordable, !valid.is_empty());
        let auto = auto_payment(&world.cost, &world.pool, &world.sources, LIFE, &world.spend);
        prop_assert_eq!(auto.is_some(), !valid.is_empty());
        if let Some(payment) = &auto {
            prop_assert!(check_payment(&world.cost, &world.pool, &world.sources, &[], LIFE, &world.spend, payment).is_ok());
            let least = valid.iter().map(|(_, life)| *life).min().unwrap_or(0);
            prop_assert_eq!(payment.life, least, "mana before life");
        }
        // Any part of a valid payment is completed to a valid one that holds it.
        for (subset, _) in valid.iter().take(6) {
            for keep in 0..=subset.len() {
                let kept: Vec<&Part> = subset[..keep].iter().map(|at| &world.parts[*at].0).collect();
                let partial = payment_of(&world, &case, &kept, 0);
                let done = complete_payment(&world.cost, &world.pool, &world.sources, &[], LIFE, &world.spend, &partial);
                prop_assert!(done.is_ok(), "{:?} of {:?}: {:?}", kept, subset, done);
                if let Ok(whole) = done {
                    prop_assert!(check_payment(&world.cost, &world.pool, &world.sources, &[], LIFE, &world.spend, &whole).is_ok());
                    for mana in &partial.pool {
                        prop_assert!(whole.pool.contains(mana));
                    }
                    for unit in &partial.restricted {
                        prop_assert!(whole.restricted.contains(unit));
                    }
                    for source in &partial.taps {
                        prop_assert!(whole.taps.contains(source));
                    }
                }
            }
        }
    }

    #[test]
    fn a_payment_never_spends_restricted_mana_its_spend_forbids(case in case()) {
        let world = world(&case);
        if let Some(payment) = auto_payment(&world.cost, &world.pool, &world.sources, LIFE, &world.spend) {
            for unit in &payment.restricted {
                prop_assert!(world.spend.allows(&unit.only));
            }
            for tap in &payment.taps {
                let option = world.sources.iter().find(|o| o.source == *tap).unwrap();
                prop_assert!(world.spend.allows(&option.only));
            }
        }
    }
}

/// A Signet (`{1}, {T}: Add {W}{U}`) and, if `land`, a Forest.
fn signet_table(land: bool) -> Vec<ManaOption> {
    let mut signet = ManaOption::new(tap(1), ManaType::White);
    signet.also = vec![ManaType::Blue];
    signet.cost = cost("{1}");
    let mut sources = vec![signet];
    if land {
        sources.push(ManaOption::new(tap(2), ManaType::Green));
    }
    sources
}

/// The restricted mana pays for a spell it is allowed on, never for the cost of a mana ability.
#[rstest]
#[case::the_land_pays_the_signet_and_the_spell_keeps_the_mana("{W}{U}", true, &[1, 2], 0)]
#[case::restricted_mana_pays_the_generic_part_of_the_spell("{1}{W}{U}", true, &[1, 2], 1)]
#[case::restricted_mana_alone_cannot_pay_the_signet("{W}{U}", false, &[], 0)]
fn restricted_mana_pays_for_the_spell_and_never_for_a_mana_ability(
    #[case] spell: &str,
    #[case] land: bool,
    #[case] tapped: &[u64],
    #[case] restricted_used: usize,
) {
    let cost = cost(spell);
    let spend = spell_of(&[CardType::Creature], false);
    let pool = pool(&[], &[(ManaType::Green, only_for(CardType::Creature))]);
    let sources = signet_table(land);
    let auto = auto_payment(&cost, &pool, &sources, LIFE, &spend);
    assert_eq!(auto.is_some(), land, "{auto:?}");
    assert_eq!(can_afford(&cost, &pool, &sources, LIFE, &spend), land);
    let Some(payment) = auto else {
        return;
    };
    let mut objects: Vec<u64> = payment.taps.iter().map(|t| t.object.0).collect();
    objects.sort_unstable();
    assert_eq!(objects, tapped);
    assert_eq!(payment.restricted.len(), restricted_used);
    let spent = check_payment(&cost, &pool, &sources, &[], LIFE, &spend, &payment).unwrap();
    assert_eq!(spent.steps.len(), payment.taps.len());
    let signet = spent.steps.iter().find(|s| s.source == tap(1)).unwrap();
    assert_eq!(
        signet.paid,
        [ManaType::Green],
        "from the land, not the restricted mana"
    );
}

#[test]
fn a_payment_that_spends_restricted_mana_on_a_signets_cost_is_refused() {
    let spend = spell_of(&[CardType::Creature], false);
    let pool = pool(&[], &[(ManaType::Green, only_for(CardType::Creature))]);
    let sources = signet_table(false);
    let payment = Payment {
        restricted: vec![Restricted {
            mana: ManaType::Green,
            only: only_for(CardType::Creature),
        }],
        taps: vec![tap(1)],
        ..Payment::default()
    };
    assert!(
        check_payment(
            &cost("{W}{U}"),
            &pool,
            &sources,
            &[],
            LIFE,
            &spend,
            &payment
        )
        .is_err()
    );
}

#[rstest]
#[case::creature_mana(CardType::Creature, true)]
#[case::artifact_mana(CardType::Artifact, false)]
#[case::enchantment_mana(CardType::Enchantment, false)]
fn a_card_cast_face_down_is_a_plain_creature_spell_for_the_mana_restrictions(
    #[case] restricted_to: CardType,
    #[case] allowed: bool,
) {
    let spend = spell_of(&[CardType::Artifact, CardType::Creature], false);
    assert!(spend.allows(&only_for(CardType::Artifact)));
    let face_down = spend.clone().face_down();
    assert_eq!(face_down.allows(&only_for(restricted_to)), allowed);
    assert_eq!(
        spend
            .clone()
            .cast_as(Some(mtg_cards::value::CastWay::FaceDown)),
        face_down
    );
    assert_eq!(spend.clone().cast_as(None), spend);
}

#[rstest]
#[case::creature_mana(CardType::Creature, false)]
#[case::enchantment_mana(CardType::Enchantment, true)]
fn a_bestowed_creature_spell_is_an_aura_spell_for_the_mana_restrictions(
    #[case] restricted_to: CardType,
    #[case] allowed: bool,
) {
    let spend = spell_of(&[CardType::Enchantment, CardType::Creature], false);
    assert!(spend.allows(&only_for(CardType::Creature)));
    let bestowed = spend.bestowed();
    assert_eq!(bestowed.allows(&only_for(restricted_to)), allowed);
    assert!(bestowed.traits.subtypes.iter().any(|s| s.0 == "Aura"));
}
