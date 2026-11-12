//! Goal: the l31-b writers word a payment of any amount of mana, life or energy, the amount paid said as the card says it ("that much", "that many", "for each {E} paid this way", the amount kept per player), and nothing near them: a fixed payment, an amount that is not the one paid, or two payments.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::written_variants;
use rstest::rstest;

fn says(json: &str, text: &str) -> bool {
    let ability: Ability = serde_json::from_str(json).unwrap();
    written_variants(&ability)
        .iter()
        .any(|written| written == text)
}

const SQUIRREL: &str = r#"{"Custom":{"name":"Squirrel","colors":["Green"],"types":["Creature"],"subtypes":["Squirrel"],"power":1,"toughness":1}}"#;

/// Liege of the Hollows with the payment and the count given.
fn liege(cost: &str, per_pay: &str, per_count: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"from":"Battlefield","to":"Graveyard"}}}},"effects":[{{"RepeatEach":{{"over":"EachPlayer","as":"player","effects":[{{"Pay":{{"who":{{"Bound":"player"}},"cost":{cost},"paid":[{{"RememberValue":{{"value":"X","slot":"paid"{per_pay}}}}}]}}}}]}}}},{{"Together":{{"effects":[{{"RepeatEach":{{"over":"EachPlayer","as":"creator","effects":[{{"CreateToken":{{"who":{{"Bound":"creator"}},"token":{SQUIRREL},"count":{{"Remembered":{{"on":"This","slot":"paid"{per_count}}}}}}}}}]}}}}]}}}}]}}}}"#
    )
}

const PER: &str = r#","per":{"Bound":"player"}"#;
const PER_CREATOR: &str = r#","per":{"Bound":"creator"}"#;
const LIEGE_TEXT: &str = "When ~ dies, each player may pay any amount of mana. Then each player creates a number of 1/1 green Squirrel creature tokens equal to the amount of mana they paid this way.";

#[rstest]
#[case::energy_damage(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":3}},{"Pay":{"cost":[{"PayEnergy":"X"}],"paid":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}]}}"#,
    "Choose target creature. You get {E}{E}{E}, then you may pay any amount of {E}. ~ deals that much damage to that creature."
)]
#[case::energy_shrink(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":2}},{"Pay":{"cost":[{"PayEnergy":"X"}],"paid":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Times":{"by":-1,"of":"X"}},"toughness":{"Times":{"by":-1,"of":"X"}}}}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
    "Choose target creature. You get {E}{E}, then you may pay any amount of {E}. The creature gets -1/-1 until end of turn for each {E} paid this way."
)]
#[case::energy_destroy(
    r#"{"Spell":{"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":"X"}},{"Pay":{"cost":[{"PayEnergy":"X"}],"paid":[{"Destroy":{"what":{"Each":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"}]},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}]}}}]}}]}}"#,
    "You get X {E}, then you may pay any amount of {E}. Destroy each artifact, creature, and enchantment with mana value less than or equal to the amount of {E} paid this way."
)]
#[case::life_draw(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Pay":{"cost":[{"PayLife":"X"}],"paid":[{"Draw":{"count":"X"}}]}}]}}"#,
    "At the beginning of your end step, you may pay any amount of life. If you do, draw that many cards."
)]
#[case::mana_draw(
    r#"{"Spell":{"effects":[{"Pay":{"cost":"{X}","paid":[{"Draw":{"count":"X"}}]}}]}}"#,
    "You may pay any amount of mana. If you do, draw that many cards."
)]
fn l31b_any_amount_is_worded_as_the_card_says_it(#[case] json: &str, #[case] text: &str) {
    assert!(says(json, text), "{text}");
}

#[test]
fn l31b_each_player_pays_and_gets_what_they_paid() {
    assert!(says(&liege(r#""{X}""#, PER, PER_CREATOR), LIEGE_TEXT));
}

#[rstest]
#[case::fixed_energy(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":3}},{"Pay":{"cost":[{"PayEnergy":2}],"paid":[{"DealDamage":{"to":{"Target":0},"amount":2}}]}}]}}"#,
    "Choose target creature. You get {E}{E}{E}, then you may pay any amount of {E}. ~ deals that much damage to that creature."
)]
#[case::damage_not_paid(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":3}},{"Pay":{"cost":[{"PayEnergy":"X"}],"paid":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}}"#,
    "Choose target creature. You get {E}{E}{E}, then you may pay any amount of {E}. ~ deals that much damage to that creature."
)]
#[case::life_is_not_mana(
    r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Pay":{"cost":[{"PayLife":"X"}],"paid":[{"Draw":{"count":"X"}}]}}]}}"#,
    "At the beginning of your end step, you may pay any amount of mana. If you do, draw that many cards."
)]
#[case::two_payments(
    r#"{"Spell":{"effects":[{"Pay":{"cost":[{"PayLife":"X"}],"paid":[{"Draw":{"count":"X"}}]}},{"Pay":{"cost":[{"PayLife":"X"}],"paid":[{"Draw":{"count":"X"}}]}}]}}"#,
    "You may pay any amount of life. If you do, draw that many cards. You may pay any amount of life. If you do, draw that many cards."
)]
fn l31b_near_misses_are_not_worded_as_any_amount(#[case] json: &str, #[case] text: &str) {
    assert!(!says(json, text), "{text}");
}

#[rstest]
#[case::total_not_each("", "")]
#[case::paid_kept_per_player_counted_in_all(PER, "")]
#[case::counted_for_the_payer(PER, PER)]
fn l31b_a_total_is_not_what_each_player_paid(#[case] per_pay: &str, #[case] per_count: &str) {
    assert!(!says(&liege(r#""{X}""#, per_pay, per_count), LIEGE_TEXT));
}

#[test]
fn l31b_a_fixed_payment_is_not_any_amount() {
    assert!(!says(&liege(r#""{2}""#, PER, PER_CREATOR), LIEGE_TEXT));
}
