//! Goal: the writers of the shapes the stored local-model proposals hit (r4d) give the oracle wording of the ability, and a changed clause does not read back.
use super::*;

/// The wordings the checker accepts for the rules, for the message of a failed golden.
fn wordings(json: &str) -> Vec<String> {
    written_variants(&ability(json))
}

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    wordings(json)
        .iter()
        .any(|wording| same_text(wording, clause))
}

const HUNTER: &str = r#"{"Activated":{"cost":[{"Mana":"{B}"},"Tap"],"effects":[{"Tap":{"what":{"Target":0}}},{"DealDamage":{"amount":{"Stat":{"of":"This","stat":"Power"}},"to":{"Target":0},"source":"This"}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const BOON: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":2,"toughness":0}}]}},{"Regenerate":{"what":{"Target":0}}},{"LoseLife":{"amount":2}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const FLESH: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":2,"toughness":2}}]}},{"PutCounters":{"counter":{"Other":"oil"},"on":{"Target":0},"count":2}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const CRUELTY: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":2,"to":{"Target":0}}},{"Discard":{"count":2,"who":{"Target":0}}}],"targets":[{"what":"Player"}]}}"#;
const SNIPER: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"DealDamage":{"amount":1,"to":{"Target":0},"source":"This"}},{"Tap":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const SNIPER_TWO: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"DealDamage":{"amount":1,"to":{"Target":0},"source":"This"}},{"Tap":{"what":{"Target":1}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const TWICE: &str = r#"{"Spell":{"effects":[{"Draw":{"count":3,"who":{"Target":0}}},{"LoseLife":{"amount":3,"who":{"Target":0}}},{"Discard":{"who":{"Target":0}}}],"targets":[{"what":"Player"}]}}"#;
const SILKENFIST: &str = r#"{"Triggered":{"effects":[{"Untap":{"what":"This"}}],"when":{"BecomesBlocked":{"what":["This"]}}}}"#;
const THRASHER: &str = r#"{"Triggered":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":["CantBlock"]}}],"when":{"Attacks":{"what":[{"Is":"This"}]}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const TRAINEE: &str = r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Is":"This"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":-2,"toughness":0}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const TRAINEE_ANY: &str = r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":-2,"toughness":0}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const MUDHOLE: &str = r#"{"Spell":{"effects":[{"Exile":{"what":{"Each":[{"Type":"Land"},{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}}}],"targets":[{"what":"Player"}]}}"#;
const LIBRARY: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}],"condition":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Eq","right":7}}}}"#;
const LIBRARY_AT_LEAST: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}],"condition":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Ge","right":7}}}}"#;
const CLAIRVOYANCE: &str = r#"{"Spell":{"effects":[{"DelayedTrigger":{"effects":[{"Draw":{}}],"when":{"StepBegins":{"step":"Upkeep"}}}}]}}"#;
const PIGEONS: &str = r#"{"Triggered":{"effects":[{"DelayedTrigger":{"effects":[{"Draw":{}}],"when":{"StepBegins":{"step":"Upkeep"}}}}],"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}}}}"#;
const YOUR_UPKEEP: &str = r#"{"Spell":{"effects":[{"DelayedTrigger":{"effects":[{"Draw":{}}],"when":{"StepBegins":{"step":"Upkeep","whose":"You"}}}}]}}"#;

#[rstest]
#[case::hunter(
    HUNTER,
    "{B}, {T}: Tap target creature. This creature deals damage equal to its power to that creature."
)]
#[case::boon(
    BOON,
    "Target creature gets +2/+0 until end of turn. Regenerate it. You lose 2 life."
)]
#[case::flesh(
    FLESH,
    "Target creature gets +2/+2 until end of turn. Put two oil counters on it."
)]
#[case::cruelty(
    CRUELTY,
    "~ deals 2 damage to target player. That player discards two cards."
)]
#[case::sniper(
    SNIPER,
    "{1}, {T}: This creature deals 1 damage to target creature. Tap that creature."
)]
#[case::silkenfist(SILKENFIST, "Whenever this creature becomes blocked, untap it.")]
#[case::thrasher(
    THRASHER,
    "Whenever this creature attacks, target creature can't block this turn."
)]
#[case::trainee(
    TRAINEE,
    "Sacrifice this creature: Target creature gets -2/+0 until end of turn."
)]
#[case::trainee_signed(
    TRAINEE,
    "Sacrifice this creature: Target creature gets -2/-0 until end of turn."
)]
#[case::mudhole(MUDHOLE, "Target player exiles all land cards from their graveyard.")]
#[case::library(
    LIBRARY,
    "{T}: Draw a card. Activate only if you have exactly seven cards in hand."
)]
#[case::clairvoyance(
    CLAIRVOYANCE,
    "Draw a card at the beginning of the next turn's upkeep."
)]
#[case::pigeons(
    PIGEONS,
    "When this creature enters, draw a card at the beginning of the next turn's upkeep."
)]
fn writes_the_oracle_wording(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}: {:?}", wordings(json));
}

#[rstest]
#[case::hunter_other(
    HUNTER,
    "{B}, {T}: Tap target creature. This creature deals damage equal to its power to that player."
)]
#[case::boon_life(
    BOON,
    "Target creature gets +2/+0 until end of turn. Regenerate it. You lose 3 life."
)]
#[case::flesh_count(
    FLESH,
    "Target creature gets +2/+2 until end of turn. Put an oil counter on it."
)]
#[case::cruelty_player(
    CRUELTY,
    "~ deals 2 damage to target player. That player discards a card."
)]
#[case::cruelty_it(CRUELTY, "~ deals 2 damage to target player. It discards two cards.")]
#[case::sniper_two(
    SNIPER_TWO,
    "{1}, {T}: This creature deals 1 damage to target creature. Tap that creature."
)]
#[case::twice(
    TWICE,
    "Target player draws three cards. That player loses 3 life. That player discards a card."
)]
#[case::silkenfist_tap(SILKENFIST, "Whenever this creature becomes blocked, tap it.")]
#[case::thrasher_attack(
    THRASHER,
    "Whenever this creature attacks, target creature can't attack this turn."
)]
#[case::trainee_any(
    TRAINEE_ANY,
    "Sacrifice this creature: Target creature gets -2/+0 until end of turn."
)]
#[case::mudhole_you(MUDHOLE, "Target player exiles all land cards from your graveyard.")]
#[case::mudhole_kind(
    MUDHOLE,
    "Target player exiles all creature cards from their graveyard."
)]
#[case::library_count(
    LIBRARY,
    "{T}: Draw a card. Activate only if you have exactly six cards in hand."
)]
#[case::library_at_least(
    LIBRARY_AT_LEAST,
    "{T}: Draw a card. Activate only if you have exactly seven cards in hand."
)]
#[case::trainee_toughness(
    TRAINEE,
    "Sacrifice this creature: Target creature gets -2/-1 until end of turn."
)]
#[case::boon_signed(
    BOON,
    "Target creature gets +2/-0 until end of turn. Regenerate it. You lose 2 life."
)]
#[case::your_upkeep(YOUR_UPKEEP, "Draw a card at the beginning of the next turn's upkeep.")]
fn a_changed_clause_does_not_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}
