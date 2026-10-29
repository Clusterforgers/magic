//! Goal: a change of control reads and writes back as the card words it (a target for the turn, for good or while the source stays, the enchanted permanent, all of a group, a target of each opponent, the untap and the keywords after it, and an exchange of two targets with its combat restriction), and a near-miss stays with the table.
use super::*;

#[rstest]
#[case::for_the_turn(
    "Gain control of target creature until end of turn.",
    ClauseKind::Spell
)]
#[case::for_good(
    "Gain control of target creature with power 2 or less.",
    ClauseKind::Spell
)]
#[case::up_to_one(
    "Gain control of up to one target creature an opponent controls until end of turn. Untap that creature. It gains haste until end of turn.",
    ClauseKind::Spell
)]
#[case::untapped_with_keywords(
    "When you cast this spell, gain control of target creature until end of turn. Untap that creature. It gains trample and haste until end of turn.",
    ClauseKind::Triggered
)]
#[case::while_you_control(
    "When this creature enters, gain control of target artifact for as long as you control this creature.",
    ClauseKind::Triggered
)]
#[case::enchanted_permanent(
    "When this Aura enters and at the beginning of your upkeep, gain control of enchanted permanent until end of turn. Untap that permanent. It gains haste until end of turn.",
    ClauseKind::Triggered
)]
#[case::a_group("Gain control of all artifacts.", ClauseKind::Spell)]
#[case::that_players_group(
    "Whenever this creature deals combat damage to a player, gain control of all artifacts that player controls.",
    ClauseKind::Triggered
)]
#[case::each_opponent(
    "For each opponent, gain control of target permanent that player controls.",
    ClauseKind::Spell
)]
#[case::each_opponent_untapped(
    "For each opponent, gain control of up to one target creature that player controls until end of turn. Untap those creatures. They gain haste until end of turn.",
    ClauseKind::Spell
)]
#[case::each_opponent_keywords(
    "When you cast this spell, for each opponent, gain control of up to one target creature that player controls until end of turn. Untap those creatures. They gain trample, haste, and annihilator 1 until end of turn.",
    ClauseKind::Triggered
)]
#[case::exchange_two(
    "{T}: Exchange control of two target nonlegendary creatures. You can't activate this ability during combat.",
    ClauseKind::Activated
)]
#[case::exchange_pair(
    "When this creature enters, exchange control of target land you control and target land an opponent controls.",
    ClauseKind::Triggered
)]
fn w6e_a_change_of_control_reads_and_writes_back(#[case] text: &str, #[case] kind: ClauseKind) {
    golden(text, kind, false);
}

#[test]
fn w6e_the_steps_read_into_the_dsl() {
    reads_as(
        "For each opponent, gain control of up to one target creature that player controls until end of turn. Untap those creatures. They gain haste until end of turn.",
        ClauseKind::Spell,
        false,
        r#"{"Spell":{"targets":[{"what":{"PerPlayer":{"players":"Opponents","what":[{"Type":"Creature"}]}},"bounds":{"min":0,"max":1}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}},{"Untap":{"what":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}"#,
    );
    reads_as(
        "{T}: Exchange control of two target creatures. You can't activate this ability during combat.",
        ClauseKind::Activated,
        false,
        r#"{"Activated":{"cost":["Tap"],"condition":{"Not":"InCombat"},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":2,"max":2}}],"effects":[{"ExchangeControl":{"what":[{"Target":0}],"duration":"Forever"}}]}}"#,
    );
}

#[rstest]
#[case::plural_of_one_target(
    "Gain control of target creature until end of turn. Untap those creatures. They gain haste until end of turn.",
    ClauseKind::Spell
)]
#[case::another_noun(
    "Gain control of target creature until end of turn. Untap that permanent. It gains haste until end of turn.",
    ClauseKind::Spell
)]
#[case::untap_for_good(
    "Gain control of target creature. Untap that creature. It gains haste until end of turn.",
    ClauseKind::Spell
)]
#[case::three_to_exchange("Exchange control of three target creatures.", ClauseKind::Spell)]
#[case::exchange_one("Exchange control of target creature.", ClauseKind::Spell)]
#[case::during_your_turn(
    "{T}: Draw a card. You can't activate this ability during your turn.",
    ClauseKind::Activated
)]
fn w6e_a_near_miss_stays_unread(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
