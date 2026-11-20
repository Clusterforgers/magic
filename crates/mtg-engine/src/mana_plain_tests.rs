//! Goal: the game pays with plain mana whenever plain mana can (hybrid and Phyrexian symbols with mana, `{S}` with snow mana, a source's plain ability beside its paid one, restricted pool mana where it may be spent) and otherwise names what the payment needs beyond it.

use mtg_cards::spend::{ManaLasts, SpendOn};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_core::mana::{ManaCost, ManaSymbol, ManaType};
use rstest::rstest;

use crate::help::{ConvokeOption, Offered};
use crate::mana::{AskedWhy, ManaOption, ManaPool, ManaSource, plain_payment, propose};
use crate::spend::Spend;

const LIFE: i32 = 20;

/// A source the payer controls, by what its mana ability does beside adding mana.
#[derive(Debug, Clone, Copy)]
enum Kind {
    /// Taps for the mana and does nothing else.
    Plain,
    /// A snow land.
    Snow,
    /// Deals damage to its controller too.
    Painland,
    /// Sacrificed for the mana.
    Treasure,
    /// Costs `{1}` of its own.
    Signet,
    /// Pays 1 life.
    LifeLand,
    /// Pays 1 energy.
    Battery,
    /// Ability 0 sacrifices it, ability 1 only taps it.
    SacOrTap,
}

fn ability(id: u64, ability: usize, mana: ManaType) -> ManaOption {
    ManaOption::new(
        ManaSource {
            object: ObjectId(id),
            ability,
        },
        mana,
    )
}

/// The options of a source at `id` of `kind` that adds `mana`.
fn source(id: u64, kind: Kind, mana: ManaType) -> Vec<ManaOption> {
    let mut option = ability(id, 0, mana);
    match kind {
        Kind::Plain => {}
        Kind::Snow => option.only = vec![SpendOn::Snow],
        Kind::Painland => option.extra = true,
        Kind::Treasure => option.sacrifice = true,
        Kind::Signet => option.cost = ManaCost::parse("{1}").unwrap(),
        Kind::LifeLand => option.life = 1,
        Kind::Battery => {
            option.energy = 1;
            option.energy_held = 3;
        }
        Kind::SacOrTap => {
            option.sacrifice = true;
            return vec![option, ability(id, 1, mana)];
        }
    }
    vec![option]
}

/// A Forest (object 30) and the sources listed, from object 31 on.
fn table(rest: &[(Kind, ManaType)]) -> Vec<ManaOption> {
    let mut sources = source(30, Kind::Plain, ManaType::Green);
    for (n, (kind, mana)) in (31..).zip(rest) {
        sources.extend(source(n, *kind, *mana));
    }
    sources
}

const R: ManaType = ManaType::Red;
const G: ManaType = ManaType::Green;

/// The payment for `cost` with `sources` and no pool: the plain taps, or why it is asked.
fn proposed(cost: &str, sources: &[ManaOption], help: &Offered) -> Result<Vec<u64>, AskedWhy> {
    let cost = ManaCost::parse(cost).unwrap();
    let proposal = propose(
        &cost,
        &ManaPool::default(),
        sources,
        help.help(),
        LIFE,
        &Spend::other(),
    );
    if let Some(why) = proposal.asked {
        return Err(why);
    }
    let payment = proposal.payment.unwrap();
    assert_eq!(payment.life, 0);
    assert!(payment.convoke.is_empty() && payment.delve.is_empty());
    Ok(payment.taps.iter().map(|t| t.object.0).collect())
}

fn convoke(id: u64) -> Offered {
    Offered {
        convoke: vec![ConvokeOption {
            creature: ObjectId(id),
            colors: Vec::new(),
        }],
        ..Offered::default()
    }
}

#[rstest]
#[case::hybrid("{R/G}", vec![], Ok(vec![30]))]
#[case::mono_hybrid("{2/R}", vec![(Kind::Plain, G)], Ok(vec![30, 31]))]
#[case::phyrexian_with_mana("{G/P}", vec![], Ok(vec![30]))]
#[case::phyrexian_with_life_only("{B/P}", vec![], Err(AskedWhy::Life(ManaSymbol::Phyrexian(Color::Black))))]
#[case::snow_from_a_snow_land("{S}{G}", vec![(Kind::Snow, R)], Ok(vec![31, 30]))]
#[case::snow_without_one("{S}", vec![(Kind::Plain, R)], Err(AskedWhy::NoPayment))]
#[case::plain_ability_beside_a_sacrifice("{R}", vec![(Kind::SacOrTap, R)], Ok(vec![31]))]
#[case::treasure_only("{R}", vec![(Kind::Treasure, R)], Err(AskedWhy::GivesUp(vec![ObjectId(31)])))]
#[case::unused_treasure("{G}", vec![(Kind::Treasure, R)], Ok(vec![30]))]
#[case::painland("{R}", vec![(Kind::Painland, R)], Err(AskedWhy::DoesMore(vec![ObjectId(31)])))]
#[case::signet("{R}", vec![(Kind::Signet, R)], Err(AskedWhy::PaidAbility(vec![ObjectId(31)])))]
#[case::life_land("{R}", vec![(Kind::LifeLand, R)], Err(AskedWhy::PaysLife(vec![ObjectId(31)])))]
#[case::energy("{R}", vec![(Kind::Battery, R)], Err(AskedWhy::Energy(vec![ObjectId(31)])))]
#[case::x_paid("{3}{G}", vec![(Kind::Plain, R), (Kind::Plain, R), (Kind::Plain, G)], Ok(vec![30, 31, 32, 33]))]
#[case::too_much("{3}", vec![(Kind::Plain, R)], Err(AskedWhy::NoPayment))]
fn plain_mana_pays_whenever_it_can(
    #[case] cost: &str,
    #[case] rest: Vec<(Kind, ManaType)>,
    #[case] expected: Result<Vec<u64>, AskedWhy>,
) {
    let mut got = proposed(cost, &table(&rest), &Offered::default());
    if let Ok(taps) = &mut got {
        taps.sort_unstable();
    }
    let mut expected = expected;
    if let Ok(taps) = &mut expected {
        taps.sort_unstable();
    }
    assert_eq!(got, expected);
}

#[rstest]
#[case::mana_suffices("{1}", Ok(vec![30]))]
#[case::needs_the_creature("{2}", Err(AskedWhy::Convoke))]
fn offered_help_is_left_unused(#[case] cost: &str, #[case] expected: Result<Vec<u64>, AskedWhy>) {
    assert_eq!(proposed(cost, &table(&[]), &convoke(60)), expected);
}

#[rstest]
#[case::spent_where_allowed(SpendOn::OtherCosts, true)]
#[case::kept_where_not(SpendOn::CumulativeUpkeep, false)]
fn restricted_pool_mana_follows_the_spend_rules(#[case] only: SpendOn, #[case] paid: bool) {
    let mut pool = ManaPool::default();
    pool.add_restricted(ManaType::Colorless, vec![only], ManaLasts::Step);
    let payment = plain_payment(
        &ManaCost::parse("{1}").unwrap(),
        &pool,
        &[],
        &Spend::other(),
    );
    assert_eq!(payment.is_some(), paid);
    if let Some(payment) = payment {
        assert_eq!(payment.restricted.len(), 1);
    }
}

/// A land at `id` with one ability that adds one mana of any of `kinds`.
fn land_of(id: u64, kinds: &[ManaType]) -> Vec<ManaOption> {
    kinds.iter().map(|mana| ability(id, 0, *mana)).collect()
}

/// The Reaper, King No More (`{2/B}{2/R}{2/G}`) from a land of any colour, Spire Garden, Cinder Glade and two Swamps:
/// each monocolored hybrid symbol takes its colour, so three lands pay it.
#[rstest]
#[case::all_lands(5, 3)]
#[case::without_the_swamps(3, 3)]
#[case::two_dual_lands_only(2, 4)]
fn monocolored_hybrid_takes_the_colour_when_it_pays(#[case] lands: usize, #[case] taps: usize) {
    use ManaType::{Black, Blue, Green, Red, White};
    let all = [
        land_of(30, &[White, Blue, Black, Red, Green]),
        land_of(31, &[Red, Green]),
        land_of(32, &[Red, Green]),
        land_of(33, &[Black]),
        land_of(34, &[Black]),
    ];
    let mut sources: Vec<ManaOption> = all[..lands.min(3)].concat();
    if lands == 2 {
        sources = [land_of(31, &[Red, Green]), land_of(32, &[Red, Green])].concat();
        for id in 40..42 {
            sources.extend(land_of(id, &[Green]));
        }
    } else if lands == 5 {
        sources.extend(all[3..].concat());
    }
    let cost = ManaCost::parse("{2/B}{2/R}{2/G}").unwrap();
    let proposal = propose(
        &cost,
        &ManaPool::default(),
        &sources,
        Offered::default().help(),
        LIFE,
        &Spend::other(),
    );
    assert_eq!(proposal.asked, None);
    assert_eq!(proposal.payment.unwrap().taps.len(), taps);
}
