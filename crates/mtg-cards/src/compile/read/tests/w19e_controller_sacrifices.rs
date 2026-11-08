//! Goal: the controller of the spell or ability that targeted the card sacrifices a permanent, written back as the card prints it; the same sacrifice by the controller of another source reads differently or not at all.
use super::*;

#[test]
fn w19e_the_controller_of_the_targeting_spell_or_ability_sacrifices_a_land() {
    let rule = ability(
        r#"{"Triggered":{"when":{"BecomesTarget":{"what":["This"]}},"effects":[{"Sacrifice":{"who":{"ControllerOf":"Dealer"},"filter":[{"Type":"Land"}]}}]}}"#,
    );
    let wordings = render_variants(&rule);
    assert!(
        wordings.iter().any(|written| same_text(
            written,
            "Whenever this creature becomes the target of a spell or ability, that spell or ability's controller sacrifices a land of their choice."
        )),
        "{wordings:?}"
    );
}

#[test]
fn w19e_a_sacrifice_by_the_controller_of_the_card_itself_is_not_that_spells_controller() {
    let rule = ability(
        r#"{"Triggered":{"when":{"BecomesTarget":{"what":["This"]}},"effects":[{"Sacrifice":{"who":"You","filter":[{"Type":"Land"}]}}]}}"#,
    );
    let wordings = render_variants(&rule);
    assert!(
        !wordings
            .iter()
            .any(|written| written.contains("that spell or ability's controller")),
        "{wordings:?}"
    );
}
