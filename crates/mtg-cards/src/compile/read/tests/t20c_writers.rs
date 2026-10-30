//! Goal: a payment of `{1}` repeated by an amount reads back as "pays {X}, where X is" that amount, and a life total that cannot change or a player who gains the keyword the group gains is worded beside the grant of a spell; another cost, count, keyword, group, duration or condition never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// A trigger on an opponent casting their first noncreature spell whose one effect is `effect`.
fn first_noncreature(effect: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Nth":{{"event":{{"SpellCast":{{"by":"Opponents","what":[{{"Not":{{"Type":"Creature"}}}}]}}}},"nth":1}}}},"effects":[{effect}]}}}}"#
    )
}

/// The payment of the caster of the spell: `cost` made `times` times, the card drawn when it is not paid.
fn pays(cost: &str, times: &str, paid: &str) -> String {
    format!(
        r#"{{"Pay":{{"who":"Triggering","cost":"{cost}","times":{times},"paid":{paid},"unpaid":[{{"Draw":{{}}}}]}}}}"#
    )
}

const POWER: &str = r#"{"Stat":{"of":"This","stat":"Power"}}"#;
const TOUGHNESS: &str = r#"{"Stat":{"of":"This","stat":"Toughness"}}"#;
const CREATURES: &str = r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#;

#[rstest]
#[case::power(
    POWER,
    "Whenever an opponent casts their first noncreature spell each turn, draw a card unless that player pays {X}, where X is this creature's power."
)]
#[case::toughness(
    TOUGHNESS,
    "Whenever an opponent casts their first noncreature spell each turn, draw a card unless that player pays {X}, where X is this creature's toughness."
)]
#[case::a_count(
    CREATURES,
    "Whenever an opponent casts their first noncreature spell each turn, draw a card unless that player pays {X}, where X is the number of creatures you control."
)]
fn a_payment_repeated_by_an_amount_is_a_payment_of_x(#[case] times: &str, #[case] clause: &str) {
    let json = first_noncreature(&pays("{1}", times, "[]"));
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_stat(
    POWER,
    "{1}",
    "[]",
    "Whenever an opponent casts their first noncreature spell each turn, draw a card unless that player pays {X}, where X is this creature's toughness."
)]
#[case::the_amount_left_out(
    POWER,
    "{1}",
    "[]",
    "Whenever an opponent casts their first noncreature spell each turn, draw a card unless that player pays {X}."
)]
#[case::two_mana_each_time(
    POWER,
    "{2}",
    "[]",
    "Whenever an opponent casts their first noncreature spell each turn, draw a card unless that player pays {X}, where X is this creature's power."
)]
#[case::colored_mana_each_time(
    POWER,
    "{G}",
    "[]",
    "Whenever an opponent casts their first noncreature spell each turn, draw a card unless that player pays {X}, where X is this creature's power."
)]
#[case::something_follows_the_payment(
    POWER,
    "{1}",
    r#"[{"GainLife":{"amount":1}}]"#,
    "Whenever an opponent casts their first noncreature spell each turn, draw a card unless that player pays {X}, where X is this creature's power."
)]
fn another_payment_never_reads_as_a_payment_of_x(
    #[case] times: &str,
    #[case] cost: &str,
    #[case] paid: &str,
    #[case] clause: &str,
) {
    let json = first_noncreature(&pays(cost, times, paid));
    assert!(!written(&json, clause), "{clause}");
}

const FROZEN: &str =
    r#"{"Rule":{"duration":"UntilEndOfTurn","rule":{"LifeTotalCantChange":"You"}}}"#;

/// The grant of `keywords` to `group` until end of turn.
fn grant(group: &str, keywords: &[&str]) -> String {
    let mods: Vec<String> = keywords
        .iter()
        .map(|keyword| format!(r#"{{"AddKeyword":"{keyword}"}}"#))
        .collect();
    format!(
        r#"{{"Continuous":{{"affected":{{"Each":[{group}]}},"duration":"UntilEndOfTurn","mods":[{}]}}}}"#,
        mods.join(",")
    )
}

const PERMANENTS: &str = r#"{"ControlledBy":"You"}"#;
const CREATURE_GROUP: &str = r#"{"Type":"Creature"},{"ControlledBy":"You"}"#;

#[rstest]
#[case::two_keywords(
    &["Hexproof", "Indestructible"],
    PERMANENTS,
    "Until end of turn, your life total can't change, and permanents you control gain hexproof and indestructible."
)]
#[case::one_keyword(
    &["Hexproof"],
    PERMANENTS,
    "Until end of turn, your life total can't change, and permanents you control gain hexproof."
)]
#[case::creatures(
    &["Indestructible"],
    CREATURE_GROUP,
    "Until end of turn, your life total can't change, and creatures you control gain indestructible."
)]
fn a_life_total_that_cannot_change_is_worded_before_the_grant(
    #[case] keywords: &[&str],
    #[case] group: &str,
    #[case] clause: &str,
) {
    let json = format!(
        r#"{{"Spell":{{"effects":[{FROZEN},{}]}}}}"#,
        grant(group, keywords)
    );
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::another_player(
    r#"{"Rule":{"duration":"UntilEndOfTurn","rule":{"LifeTotalCantChange":"Opponents"}}}"#,
    "Until end of turn, your life total can't change, and permanents you control gain hexproof."
)]
#[case::until_your_next_turn(
    r#"{"Rule":{"duration":"UntilYourNextTurn","rule":{"LifeTotalCantChange":"You"}}}"#,
    "Until end of turn, your life total can't change, and permanents you control gain hexproof."
)]
#[case::another_rule(
    r#"{"Rule":{"duration":"UntilEndOfTurn","rule":{"CantGainLife":"You"}}}"#,
    "Until end of turn, your life total can't change, and permanents you control gain hexproof."
)]
fn another_rule_never_reads_as_a_life_total_that_cannot_change(
    #[case] rule: &str,
    #[case] clause: &str,
) {
    let json = format!(
        r#"{{"Spell":{{"effects":[{rule},{}]}}}}"#,
        grant(PERMANENTS, &["Hexproof"])
    );
    assert!(!written(&json, clause), "{clause}");
}

/// You gain `keyword` until end of turn.
fn you_have(keyword: &str) -> String {
    format!(
        r#"{{"Rule":{{"duration":"UntilEndOfTurn","rule":{{"PlayersHave":{{"who":"You","keyword":"{keyword}"}}}}}}}}"#
    )
}

/// "If the gift was promised", the group gains `keyword` as well.
fn promised(group: &str, keyword: &str) -> String {
    format!(
        r#"{{"If":{{"condition":"AdditionalCostPaid","then":[{}]}}}}"#,
        grant(group, &[keyword])
    )
}

#[rstest]
#[case::hexproof_and_the_gift(
    "Hexproof",
    Some("Indestructible"),
    "You and permanents you control gain hexproof until end of turn. If the gift was promised, permanents you control also gain indestructible until end of turn."
)]
#[case::the_gift_gives_another_keyword(
    "Hexproof",
    Some("Lifelink"),
    "You and permanents you control gain hexproof until end of turn. If the gift was promised, permanents you control also gain lifelink until end of turn."
)]
#[case::no_gift(
    "Hexproof",
    None,
    "You and permanents you control gain hexproof until end of turn."
)]
fn a_player_and_a_group_gain_a_keyword(
    #[case] keyword: &str,
    #[case] gift: Option<&str>,
    #[case] clause: &str,
) {
    let mut steps = vec![you_have(keyword), grant(PERMANENTS, &[keyword])];
    steps.extend(gift.map(|extra| promised(PERMANENTS, extra)));
    let json = format!(r#"{{"Spell":{{"effects":[{}]}}}}"#, steps.join(","));
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::the_player_has_another_keyword(
    "Shroud",
    "Hexproof",
    None,
    "You and permanents you control gain hexproof until end of turn."
)]
#[case::the_gift_names_another_group(
    "Hexproof",
    "Hexproof",
    Some(CREATURE_GROUP),
    "You and permanents you control gain hexproof until end of turn. If the gift was promised, permanents you control also gain indestructible until end of turn."
)]
#[case::the_gift_text_is_not_the_data(
    "Hexproof",
    "Hexproof",
    None,
    "You and permanents you control gain hexproof until end of turn. If the gift was promised, permanents you control also gain indestructible until end of turn."
)]
fn another_keyword_or_group_never_reads_as_the_pair(
    #[case] player: &str,
    #[case] group: &str,
    #[case] gift_group: Option<&str>,
    #[case] clause: &str,
) {
    let mut steps = vec![you_have(player), grant(PERMANENTS, &[group])];
    steps.extend(gift_group.map(|grouped| promised(grouped, "Indestructible")));
    let json = format!(r#"{{"Spell":{{"effects":[{}]}}}}"#, steps.join(","));
    assert!(!written(&json, clause), "{clause}");
}
