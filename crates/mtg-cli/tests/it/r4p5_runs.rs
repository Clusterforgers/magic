//! Goal: an activated ability that costs less for each object counted is offered once the reduced cost is payable, and only then, in a played game.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Supertype};
use mtg_testkit::cards::{forest, swamp};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use std::sync::Arc;

use crate::r3p3_runs::translated;

const BOSEIJU: &str = "Name:Boseiju, Who Endures\nManaCost:no cost\nTypes:Legendary Land\nA:AB$ Mana | Cost$ T | Produced$ G | SpellDescription$ Add {G}.\nA:AB$ Destroy | PrecostDesc$ Channel — | Cost$ 1 G Discard<1/CARDNAME> | ValidTgts$ Artifact.OppCtrl,Enchantment.OppCtrl,Land.nonBasic+OppCtrl | TgtPrompt$ Select target | ReduceCost$ X | ActivationZone$ Hand | SpellDescription$ Destroy target artifact.\nSVar:DBChangeZone:DB$ ChangeZone | Optional$ True | Origin$ Library | Destination$ Battlefield | ChangeType$ Land.hasABasicLandType | ChangeNum$ 1 | DefinedPlayer$ TargetedController | ShuffleNonMandatory$ True\nSVar:X:Count$Valid Creature.Legendary+YouCtrl\n";
const TAKENUMA: &str = "Name:Takenuma, Abandoned Mire\nManaCost:no cost\nTypes:Legendary Land\nA:AB$ Mana | Cost$ T | Produced$ B | SpellDescription$ Add {B}.\nA:AB$ Mill | PrecostDesc$ Channel — | Cost$ 3 B Discard<1/CARDNAME> | ActivationZone$ Hand | NumCards$ 3 | Defined$ You | SubAbility$ DBChangeZone | ReduceCost$ Count$Valid Creature.Legendary+YouCtrl | SpellDescription$ Mill three cards,\nSVar:DBChangeZone:DB$ ChangeZone | Origin$ Graveyard | Destination$ Hand | ChangeType$ Creature.YouOwn,Planeswalker.YouOwn | ChangeNum$ 1 | Mandatory$ True | Hidden$ True | SpellDescription$ then return a creature.\n";

fn creature(name: &str, legendary: bool) -> Arc<CardDefinition> {
    let card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(1, 1);
    Arc::new(if legendary {
        card.with_supertype(Supertype::Legendary)
    } else {
        card
    })
}

/// Whether the channel ability (the only one offered from the hand) of the translated land is offered with `lands` untapped lands in play and
/// `legends` legendary and `others` other creatures.
fn offered(
    script: &str,
    name: &str,
    land: &Arc<CardDefinition>,
    lands: usize,
    legends: usize,
    others: usize,
) -> bool {
    let card = translated(script, name, "Legendary Land", "");
    let mut field: Vec<_> = (0..lands).map(|_| ready(land.clone())).collect();
    field.extend((0..legends).map(|i| ready(creature(&format!("Legend {i}"), true))));
    field.extend((0..others).map(|i| ready(creature(&format!("Plain {i}"), false))));
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, field)
        .battlefield(
            1,
            [ready(Arc::new(
                CardDefinition::new("Trinket").with_type(CardType::Artifact),
            ))],
        )
        .library(0, (0..9).map(|_| forest()).collect::<Vec<_>>())
        .library(1, (0..9).map(|_| forest()).collect::<Vec<_>>());
    let driver = Driver::from_scenario(scenario).unwrap();
    !driver.offered_abilities(name).unwrap().is_empty()
}

/// Boseiju's channel (without its search half, a separate gap) costs {1}{G}: a legendary creature takes the generic mana off, never the green, and creatures that are
/// not legendary take nothing. (A Forest taps for {G}, so one land pays {G} only.)
#[rstest]
#[case::none_one_land(0, 0, 1, false)]
#[case::one_legend_one_land(1, 0, 1, true)]
#[case::two_legends_one_land(2, 0, 1, true)]
#[case::plain_creatures_one_land(0, 3, 1, false)]
#[case::none_two_lands(0, 0, 2, true)]
fn boseiju_channel_costs_one_less_for_each_legendary_creature(
    #[case] legends: usize,
    #[case] others: usize,
    #[case] lands: usize,
    #[case] offered_now: bool,
) {
    assert_eq!(
        offered(
            BOSEIJU,
            "Boseiju, Who Endures",
            &forest(),
            lands,
            legends,
            others
        ),
        offered_now
    );
}

/// Takenuma's channel costs {3}{B}.
#[rstest]
#[case::none_three_lands(0, 3, false)]
#[case::one_legend_three_lands(1, 3, true)]
#[case::three_legends_one_land(3, 1, true)]
#[case::two_legends_two_lands(2, 2, true)]
#[case::one_legend_two_lands(1, 2, false)]
fn takenuma_channel_costs_one_less_for_each_legendary_creature(
    #[case] legends: usize,
    #[case] lands: usize,
    #[case] offered_now: bool,
) {
    assert_eq!(
        offered(
            TAKENUMA,
            "Takenuma, Abandoned Mire",
            &swamp(),
            lands,
            legends,
            0
        ),
        offered_now
    );
}
