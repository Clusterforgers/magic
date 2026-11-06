//! Goal: fix pass 9's read-back wordings accept exactly their rule: "permanent cards" for all six permanent types, the excess damage sentence for damage whose excess goes to the target's controller, and a gendered pronoun never for the "its" of a target.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const WARGATE: &str = r#"{"Spell":{"effects":[{"Search":{"as":"found","filter":[{"Compare":{"op":"Le","stat":"ManaValue","to":"X"}},{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}]}},{"Move":{"to":{"Battlefield":{}},"what":{"Bound":"found"}}},{"Shuffle":{}}]}}"#;
const WARGATE_TEXT: &str = "Search your library for a permanent card with mana value X or less, put it onto the battlefield, then shuffle.";

#[rstest]
#[case::six_types(WARGATE, true)]
#[case::no_land(&WARGATE.replace(r#"{"Type":"Land"},"#, ""), false)]
fn fix9_a_searched_permanent_card_takes_its_article(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(written(json, WARGATE_TEXT), reads);
}

const DESCEND: &str = r#"{"Static":{"affected":"This","condition":{"Compare":{"left":{"Count":[{"OwnedBy":"You"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]},{"In":"Graveyard"}]},"op":"Ge","right":4}},"mods":[{"AddKeyword":"Flying"}]}}"#;
const DESCEND_TEXT: &str =
    "As long as there are 4 or more permanent cards in your graveyard, ~ has flying.";

#[rstest]
#[case::six_types(DESCEND, true)]
// Five types are not "permanent".
#[case::no_battle(&DESCEND.replace(r#"{"Type":"Battle"},"#, ""), false)]
#[case::other_zone(&DESCEND.replace(r#"{"In":"Graveyard"}"#, r#"{"In":"Exile"}"#), false)]
fn fix9_permanent_cards_name_the_six_permanent_types(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(
        written(json, DESCEND_TEXT),
        reads,
        "{:?}",
        written_variants(&ability(json))
    );
}

const SPILL: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":4,"excess":{"to":{"ControllerOf":{"Target":0}}},"to":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const SPILL_TEXT: &str = "~ deals 4 damage to target creature. Excess damage is dealt to that creature's controller instead.";

#[rstest]
#[case::excess(SPILL, true)]
// Damage dealt as it is has no excess sentence.
#[case::no_excess(&SPILL.replace(r#""excess":{"to":{"ControllerOf":{"Target":0}}},"#, ""), false)]
#[case::other_amount(&SPILL.replace(r#""amount":4"#, r#""amount":5"#), false)]
fn fix9_excess_damage_goes_to_the_controller(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(written(json, SPILL_TEXT), reads);
}

const RETURN_TARGET: &str = r#"{"Triggered":{"effects":[{"Move":{"to":"Hand","what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
const RETURN_SELF: &str = r#"{"Triggered":{"effects":[{"Move":{"to":"Hand","what":"This"}}],"when":{"StepBegins":{"step":"End","whose":"You"}}}}"#;

#[rstest]
#[case::target_its(
    RETURN_TARGET,
    "When ~ enters, return target nonland permanent to its owner's hand.",
    true
)]
// "Her owner's hand" after a target names the card's owner, not the target's.
#[case::target_her(
    RETURN_TARGET,
    "When ~ enters, return target nonland permanent to her owner's hand.",
    false
)]
#[case::self_her(
    RETURN_SELF,
    "At the beginning of your end step, return ~ to her owner's hand.",
    true
)]
fn fix9_a_pronoun_of_the_card_is_not_the_targets(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        written(json, clause),
        reads,
        "{:?}",
        written_variants(&ability(json))
    );
}
