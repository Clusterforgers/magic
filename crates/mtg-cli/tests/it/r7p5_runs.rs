//! Goal: references the grammar widened in round 7 piece 5 play through: the cards a permanent exiled come back only for it, and the top card of the library is the one a step moves.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::characteristics;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, forest, island, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::r3p3_runs::{settle, translated};

fn ring(name: &str) -> String {
    format!(
        "Name:{name}\nManaCost:1 W\nTypes:Enchantment\n\
         T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigExile | TriggerDescription$ When CARDNAME enters, exile another target nonland permanent.\n\
         T:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Any | ValidCard$ Card.Self | Execute$ TrigReturn | TriggerDescription$ When CARDNAME leaves the battlefield, return the exiled card to the battlefield under its owner's control.\n\
         SVar:TrigExile:DB$ ChangeZone | IsCurse$ True | ValidTgts$ Creature.OppCtrl | Origin$ Battlefield | Destination$ Exile\n\
         SVar:TrigReturn:DB$ ChangeZone | Defined$ ExiledWith | Origin$ Exile | Destination$ Battlefield\n\
         A:AB$ GainLife | Cost$ Sac<1/CARDNAME> | LifeAmount$ 1\n"
    )
}

fn count(driver: &Driver, zone: Zone, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .filter(|id| state.object(**id).is_some_and(|o| o.card().name == name))
        .count()
}

fn grower(name: &str) -> String {
    format!(
        "Name:{name}\nManaCost:W\nTypes:Creature Rogue\nPT:1/1\n\
         A:AB$ Pump | Cost$ 1 | Defined$ Self | NumAtt$ +X | SpellDescription$ This gets +X/+0 until end of turn, where X is the number of times this ability has resolved this turn.\n\
         SVar:X:Count$ResolvedThisTurn\n"
    )
}

fn power(driver: &Driver, name: &str) -> i32 {
    characteristics(driver.state(), driver.id_of(name).unwrap())
        .unwrap()
        .power
        .unwrap()
}

#[rstest]
#[case::once(&["One"], 2, 1)]
#[case::twice_counts_each_resolution(&["One", "One"], 4, 1)]
#[case::each_ability_counts_for_itself(&["One", "Two", "One"], 4, 2)]
fn an_ability_counts_only_its_own_resolutions_this_turn(
    #[case] activations: &[&str],
    #[case] one: i32,
    #[case] two: i32,
) {
    let scenario = Scenario::new(2).battlefield(
        0,
        [
            ready(translated(&grower("One"), "One", "Creature", "{W}")),
            ready(translated(&grower("Two"), "Two", "Creature", "{W}")),
            ready(plains()),
            ready(plains()),
            ready(plains()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for name in activations {
        driver.begin_ability(name, 0).unwrap();
        driver.pay_tapping(&["Plains"], 0).unwrap_or_default();
        settle(&mut driver);
    }
    assert_eq!((power(&driver, "One"), power(&driver, "Two")), (one, two));
}

#[rstest]
#[case::the_first_ring("Ring One")]
#[case::the_second_ring("Ring Two")]
fn only_the_card_a_permanent_exiled_comes_back_when_it_leaves(#[case] leaving: &str) {
    let first = translated(&ring("Ring One"), "Ring One", "Enchantment", "{1}{W}");
    let second = translated(&ring("Ring Two"), "Ring Two", "Enchantment", "{1}{W}");
    let scenario = Scenario::new(2)
        .hand(0, [first, second])
        .battlefield(0, (0..4).map(|_| ready(plains())))
        .battlefield(1, [ready(bear()), ready(bear())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Ring One").unwrap();
    settle(&mut driver);
    driver.cast("Ring Two").unwrap();
    settle(&mut driver);
    assert_eq!(count(&driver, Zone::Battlefield, "Grizzly Bears"), 0);
    assert_eq!(count(&driver, Zone::Exile, "Grizzly Bears"), 2);
    driver.begin_activation(leaving).unwrap();
    settle(&mut driver);
    assert_eq!(count(&driver, Zone::Battlefield, "Grizzly Bears"), 1);
    assert_eq!(count(&driver, Zone::Exile, "Grizzly Bears"), 1);
}

const PEEK: &str = "Name:Seeker\nManaCost:G\nTypes:Creature Dryad\nPT:1/1\n\
    A:AB$ PeekAndReveal | Cost$ T | PeekAmount$ 1 | RevealValid$ Land | RevealOptional$ True | RememberRevealed$ True | SubAbility$ DBChangeZone\n\
    SVar:DBChangeZone:DB$ ChangeZone | Defined$ TopOfLibrary | Origin$ Library | Destination$ Hand | ConditionDefined$ Remembered | ConditionPresent$ Land | ConditionCompare$ EQ1 | SubAbility$ DBCleanup\n\
    SVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";

#[rstest]
#[case::a_land_goes_to_the_hand(forest(), 1, 4)]
#[case::a_nonland_stays_on_top(bear(), 0, 5)]
fn the_top_card_moves_only_when_the_reveal_held(
    #[case] top: std::sync::Arc<mtg_cards::CardDefinition>,
    #[case] hand: usize,
    #[case] library: usize,
) {
    let seeker = translated(PEEK, "Seeker", "Creature", "{G}");
    let scenario = Scenario::new(2)
        .battlefield(0, [ready(seeker)])
        .library(0, [top, island(), island(), island(), island()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Seeker").unwrap();
    settle(&mut driver);
    let me = PlayerId::from_index(0);
    assert_eq!(driver.state().zone(Zone::Hand(me)).len(), hand);
    assert_eq!(driver.state().zone(Zone::Library(me)).len(), library);
}
