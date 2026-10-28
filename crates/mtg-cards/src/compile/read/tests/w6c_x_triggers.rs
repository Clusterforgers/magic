//! Goal: an X that a trigger reads without an `{X}` in the mana cost reads and writes back where an action announced it: the `{X}` of a payment ("you may pay {X}{R}. If you do, it deals X damage"), the `{X}` of the card's cycling cost ("when you cycle ~") and of its monstrosity ("when ~ becomes monstrous", also as a bound of its targets); without an `{X}` in the cycling cost the X stays with the table.
use super::*;

fn trigger(text: &str, cost_x: bool) -> Option<Ability> {
    let own = Own {
        w6c_cost_x: cost_x,
        ..Own::default()
    };
    clause_with(text, ClauseKind::Triggered, false, &own)
}

#[rstest]
#[case::damage(
    "Whenever ~ attacks, you may pay {X}{R}. If you do, ~ deals X damage to any target.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":"AnyTarget"}],"effects":[{"Pay":{"cost":"{X}{R}","paid":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}]}}"#
)]
#[case::cards(
    "Whenever ~ attacks, you may pay {X}{W}{U}{B}. If you do, draw X cards.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":"{X}{W}{U}{B}","paid":[{"Draw":{"count":"X"}}]}}]}}"#
)]
#[case::nothing_that_reads_it(
    "Whenever ~ attacks, you may pay {X}. If you do, draw a card.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":"{X}","paid":[{"Draw":{}}]}}]}}"#
)]
fn w6c_the_x_of_a_payment_is_the_x_of_what_follows_it(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::a_shark(
    "When you cycle ~, create an X/X blue Shark creature token with flying.",
    "CreateToken",
    true
)]
#[case::up_to_x_targets("When you cycle ~, destroy up to X target artifacts.", "Destroy", true)]
#[case::lifelink_counters(
    "When ~ becomes monstrous, put a lifelink counter on each of up to X target creatures.",
    "PutCounters",
    false
)]
fn w6c_the_x_of_the_cards_own_cost_is_read_by_its_trigger(
    #[case] text: &str,
    #[case] kind: &str,
    #[case] needs_the_cost: bool,
) {
    let read = trigger(text, true).unwrap_or_else(|| panic!("not read: {text}"));
    assert!(
        render_variants(&read)
            .iter()
            .any(|written| same_text(written, text)),
        "{text}: {:?}",
        render_variants(&read)
    );
    let json = serde_json::to_string(&read).unwrap();
    assert!(json.contains(kind) && json.contains("\"X\""), "{json}");
    // A monstrosity X is read whatever the cost (`c10_pa_monstrous`); a cycling X only with an `{X}` in the cost.
    assert_eq!(trigger(text, false).is_none(), needs_the_cost, "{text}");
}

#[rstest]
#[case::two_parts("{X} and 2 life")]
#[case::x_life("X life")]
fn w6c_a_payment_with_another_x_stays_with_the_table(#[case] cost: &str) {
    let text = format!("Whenever ~ attacks, you may pay {cost}. If you do, draw X cards.");
    assert_eq!(clause(&text, ClauseKind::Triggered, false), None, "{text}");
}

#[test]
fn w6c_a_payment_of_x_on_a_card_with_its_own_x_reads_the_x_paid() {
    reads_as(
        "When you cycle ~, you may pay {X}. If you do, create X 1/1 white Soldier creature tokens.",
        ClauseKind::Triggered,
        true,
        r#"{"Triggered":{"when":{"Cycled":{"what":["This"],"by":"You"}},"from":"Graveyard","effects":[{"Pay":{"cost":"{X}","paid":[{"CreateToken":{"count":"X","token":{"Custom":{"name":"Soldier","types":["Creature"],"subtypes":["Soldier"],"colors":["White"],"power":1,"toughness":1}}}}]}}]}}"#,
    );
}
