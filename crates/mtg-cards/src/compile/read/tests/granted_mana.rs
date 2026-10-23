//! Goal: a mana ability in quotes given to a group, to what an Aura or Equipment is attached to or to a subtype reads into a granted activated mana ability and writes back as the same text, whatever the group (lands, creatures, tokens, Treasures, Slivers, artifacts), the mana (a color, any color, two of one color) and the restriction on spending it.
use super::*;
use crate::modify::Modification;

fn grant(text: &str) -> Ability {
    golden(text, ClauseKind::Static, false)
}

#[rstest]
#[case::lands("Lands you control have \"{T}: Add one mana of any color.\"")]
#[case::creatures("Creatures you control have \"{T}: Add {G}.\"")]
#[case::tokens("Creature tokens you control have \"{T}: Add one mana of any color.\"")]
#[case::slivers("All Slivers have \"{T}: Add one mana of any color.\"")]
#[case::sliver_creatures("Sliver creatures you control have \"{T}: Add one mana of any color.\"")]
#[case::treasures(
    "Treasures you control have \"{T}, Sacrifice this artifact: Add two mana of any one color.\""
)]
#[case::foods(
    "Foods you control have \"{T}, Sacrifice this artifact: Add one mana of any color.\""
)]
#[case::artifacts(
    "Artifacts you control have \"{T}: Add one mana of any color. Spend this mana only to cast an instant or sorcery spell.\""
)]
#[case::an_enchanted_land("Enchanted land has \"{T}: Add two mana of any one color.\"")]
#[case::an_enchanted_creature("Enchanted creature has \"{T}: Add one mana of any color.\"")]
#[case::a_group_with_a_counter(
    "Each creature you control with a +1/+1 counter on it has \"{T}: Add {G}.\""
)]
fn a_granted_mana_ability_reads_whole_and_writes_back(#[case] text: &str) {
    let Ability::Static { mods, .. } = grant(text) else {
        panic!("{text}: not a static");
    };
    let [Modification::GrantAbility(given)] = mods.as_slice() else {
        panic!("{text}: not a granted ability: {mods:?}");
    };
    assert!(given.is_mana_ability(), "{text}");
}

#[rstest]
#[case::a_cost_with_no_mana("Lands you control have \"{T}: Draw a card.\"")]
#[case::unclosed("Lands you control have \"{T}: Add one mana of any color.")]
#[case::two_abilities("Lands you control have \"{T}: Add {G}.\" and \"{T}: Add {R}.\"")]
fn a_quote_that_is_no_mana_ability_is_not_one(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, false);
    let mana = matches!(
        read,
        Some(Ability::Static { ref mods, .. })
            if matches!(mods.as_slice(), [Modification::GrantAbility(given)] if given.is_mana_ability())
    );
    assert!(!mana, "{text}: {read:?}");
}
