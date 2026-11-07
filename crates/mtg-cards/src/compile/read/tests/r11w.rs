//! Goal: the round 11 writers read back steps done together as one batch (as their steps), damage to a target and to
//! its controller, the source or the other creatures it names, damage to each player equal to their own count, and a
//! token for the controller of each object removed this way; a rule of another meaning (other amount, other player,
//! other kind, a missing "other", another zone or another removal) never reads back as the clause.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const CREATURE: &str = r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#;
const ANY: &str = r#"[{"what":"AnyTarget"}]"#;

/// A spell with the targets that deals both damages in one batch.
fn pair(targets: &str, first: &str, second: &str, to: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":{targets},"effects":[{{"Together":{{"effects":[{{"DealDamage":{{"amount":{first},"to":{{"Target":0}}}}}},{{"DealDamage":{{"amount":{second},"to":{to}}}}}]}}}}]}}}}"#
    )
}

const CONTROLLER: &str = r#"{"ControllerOf":{"Target":0}}"#;
const SAME_CONTROLLER: &str = r#"{"Each":[{"Type":"Creature"},{"Not":{"Is":{"Target":0}}},{"ControlledBy":{"ControllerOf":{"Target":0}}}]}"#;
const SHARES_TYPE: &str =
    r#"{"Each":[{"Type":"Creature"},{"Not":{"Is":{"Target":0}}},{"SharesSubtype":{"Target":0}}]}"#;
const OUTRAGE: &str =
    "~ deals 4 damage to target creature and 2 damage to that creature's controller.";
const EMBERMAGE: &str = "~ deals 1 damage to any target and 1 damage to itself.";
const ARC_TRAIL: &str = "~ deals 2 damage to any target and 1 damage to another target.";
const TRAIL: &str = r#"{"Spell":{"targets":[{"what":"AnyTarget"},{"what":{"Another":{"earlier":[0],"what":"AnyTarget"}}}],"effects":[{"Together":{"effects":[{"DealDamage":{"amount":2,"to":{"Target":0}}},{"DealDamage":{"amount":AMOUNT,"to":{"Target":1}}}]}}]}}"#;

#[rstest]
#[case::arc_trail(TRAIL.replace("AMOUNT", "1"), ARC_TRAIL, true)]
#[case::arc_trail_other_amount(TRAIL.replace("AMOUNT", "2"), ARC_TRAIL, false)]
#[case::any_other_target(
    TRAIL.replace("AMOUNT", "1"),
    "~ deals 2 damage to any target and 1 damage to any other target.",
    true
)]
#[case::any_other_target_may_be_the_same(
    TRAIL.replace("AMOUNT", "1").replace(r#"{"Another":{"earlier":[0],"what":"AnyTarget"}}"#, r#""AnyTarget""#),
    "~ deals 2 damage to any target and 1 damage to any other target.",
    false
)]
#[case::outrage(pair(CREATURE, "4", "2", CONTROLLER), OUTRAGE, true)]
#[case::its_controller(
    pair(CREATURE, "1", "1", CONTROLLER),
    "~ deals 1 damage to target creature and 1 damage to its controller.",
    true
)]
#[case::outrage_other_amount(pair(CREATURE, "4", "3", CONTROLLER), OUTRAGE, false)]
#[case::outrage_to_you(pair(CREATURE, "4", "2", r#""You""#), OUTRAGE, false)]
#[case::outrage_any_target(
    pair(ANY, "4", "2", CONTROLLER),
    "~ deals 4 damage to any target and 2 damage to that creature's controller.",
    false
)]
#[case::embermage(pair(ANY, "1", "1", r#""This""#), EMBERMAGE, true)]
#[case::embermage_to_controller(pair(ANY, "1", "1", r#""You""#), EMBERMAGE, false)]
#[case::same_controller(
    pair(CREATURE, r#""X""#, "1", SAME_CONTROLLER),
    "~ deals X damage to target creature and 1 damage to each other creature with the same controller.",
    true
)]
#[case::same_controller_not_other(
    pair(CREATURE, r#""X""#, "1", &SAME_CONTROLLER.replace(r#"{"Not":{"Is":{"Target":0}}},"#, r#""Tapped","#)),
    "~ deals X damage to target creature and 1 damage to each other creature with the same controller.",
    false
)]
#[case::shares_a_type(
    pair(CREATURE, "3", "3", SHARES_TYPE),
    "~ deals 3 damage to target creature and each other creature that shares a creature type with it.",
    true
)]
#[case::shares_a_type_other_amount(
    pair(CREATURE, "3", "2", SHARES_TYPE),
    "~ deals 3 damage to target creature and each other creature that shares a creature type with it.",
    false
)]
#[case::same_name_without_the_creature_type(
    pair(
        CREATURE,
        "4",
        "4",
        r#"{"Each":[{"SharesName":{"Target":0}},{"Not":{"Is":{"Target":0}}}]}"#
    ),
    "~ deals 4 damage to target creature and each other creature with the same name as that creature.",
    false
)]
fn damage_in_one_batch_reads_back(#[case] json: String, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(
        written(&json, clause),
        same,
        "{json}: {:?}",
        written_variants(&ability(&json))
    );
}

/// A spell that deals each player of `over` the amount.
fn each_player(over: &str, amount: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Together":{{"effects":[{{"RepeatEach":{{"as":"each","over":"{over}","effects":[{{"DealDamage":{{"amount":{amount},"to":{{"Bound":"each"}}}}}}]}}}}]}}}}]}}}}"#
    )
}

const ISLANDS: &str = r#"{"Count":[{"Subtype":"Island"},{"ControlledBy":{"Bound":"each"}}]}"#;
const TYPHOON: &str =
    "~ deals damage to each opponent equal to the number of Islands that player controls.";
const NONBASIC: &str = r#"{"Times":{"by":2,"of":{"Count":[{"Type":"Land"},{"Not":{"Supertype":"Basic"}},{"ControlledBy":{"Bound":"each"}}]}}}"#;
const PRICE: &str = "~ deals damage to each player equal to twice the number of nonbasic lands that player controls.";
const HAND: &str = r#"{"Count":[{"OwnedBy":{"Bound":"each"}},{"In":"Hand"}]}"#;
const STORMBREATH: &str =
    "~ deals damage to each opponent equal to the number of cards in that player's hand.";
const HALF: &str = r#"{"Half":{"of":{"Life":{"Bound":"each"}},"up":false}}"#;
const HIDETSUGU: &str =
    "~ deals damage to each player equal to half that player's life total, rounded down.";

#[rstest]
#[case::typhoon(each_player("Opponents", ISLANDS), TYPHOON, true)]
#[case::typhoon_opponent(
    each_player("Opponents", ISLANDS),
    "~ deals damage to each opponent equal to the number of Islands that opponent controls.",
    true
)]
#[case::typhoon_each_player(each_player("EachPlayer", ISLANDS), TYPHOON, false)]
#[case::typhoon_your_islands(
    each_player(
        "Opponents",
        r#"{"Count":[{"Subtype":"Island"},{"ControlledBy":"You"}]}"#
    ),
    TYPHOON,
    false
)]
#[case::typhoon_forests(each_player("Opponents", &ISLANDS.replace("Island", "Forest")), TYPHOON, false)]
#[case::acidic_soil(
    each_player(
        "EachPlayer",
        r#"{"Count":[{"Type":"Land"},{"ControlledBy":{"Bound":"each"}}]}"#
    ),
    "~ deals damage to each player equal to the number of lands they control.",
    true
)]
#[case::price(each_player("EachPlayer", NONBASIC), PRICE, true)]
#[case::price_once(each_player("EachPlayer", &NONBASIC.replace(r#""by":2"#, r#""by":3"#)), PRICE, false)]
#[case::stormbreath(each_player("Opponents", HAND), STORMBREATH, true)]
#[case::stormbreath_graveyard(each_player("Opponents", &HAND.replace("Hand", "Graveyard")), STORMBREATH, false)]
#[case::hidetsugu(each_player("EachPlayer", HALF), HIDETSUGU, true)]
#[case::hidetsugu_rounded_up(each_player("EachPlayer", &HALF.replace("false", "true")), HIDETSUGU, false)]
#[case::hidetsugu_your_life(
    each_player("EachPlayer", r#"{"Half":{"of":{"Life":"You"},"up":false}}"#),
    HIDETSUGU,
    false
)]
fn damage_to_each_player_reads_back(
    #[case] json: String,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(
        written(&json, clause),
        same,
        "{json}: {:?}",
        written_variants(&ability(&json))
    );
}

/// A spell that removes its targets and gives the controller of each removed object a token.
fn removed(removal: &str, who: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"bounds":{{"max":"X","min":"X"}},"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{{{removal}:{{"as":"gone","what":{{"Target":0}}}}}},{{"Together":{{"effects":[{{"RepeatEach":{{"as":"each","over":{{"Bound":"gone"}},"effects":[{{"CreateToken":{{"token":{{"Custom":{{"colors":["Green"],"name":"Boar","power":2,"subtypes":["Boar"],"toughness":2,"types":["Creature"]}}}},"who":{who}}}}}]}}}}]}}}}]}}}}"#
    )
}

const LAST: &str = r#"{"ControllerOf":{"Departed":{"Bound":"each"}}}"#;
const SWINE: &str = "Exile X target creatures. For each creature exiled this way, its controller creates a 2/2 green Boar creature token.";

#[rstest]
#[case::swine(removed(r#""Exile""#, LAST), SWINE, true)]
#[case::destroyed(
    removed(r#""Destroy""#, LAST),
    "Destroy X target creatures. For each creature destroyed this way, its controller creates a 2/2 green Boar creature token.",
    true
)]
#[case::destroyed_for_exiled(removed(r#""Destroy""#, LAST), SWINE, false)]
#[case::you_create(removed(r#""Exile""#, r#""You""#), SWINE, false)]
#[case::its_owner(
    removed(r#""Exile""#, r#"{"OwnerOf":{"Departed":{"Bound":"each"}}}"#),
    SWINE,
    false
)]
fn tokens_for_what_was_removed_read_back(
    #[case] json: String,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(
        written(&json, clause),
        same,
        "{json}: {:?}",
        written_variants(&ability(&json))
    );
}

const GUARD: &str = r#"{"Activated":{"condition":"InCombat","cost":["Tap"],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":2,"toughness":2}}]}},{"DelayedTrigger":{"effects":[{"Sacrifice":{"filter":["This"]}}],"once":false,"this_turn":true,"when":{"ZoneChange":{"from":"Battlefield","what":[{"Is":{"Target":0}}]}}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const GUARD_TEXT: &str = "{T}: Target creature gets +2/+2 until end of turn. When that creature leaves the battlefield this turn, sacrifice this creature. Activate only during combat.";
const SACRIFICE: &str = r#"{"Sacrifice":{"filter":["This"]}}"#;

#[rstest]
#[case::guard(GUARD.to_owned(), true)]
#[case::for_good(GUARD.replace(r#""this_turn":true"#, r#""this_turn":false"#), false)]
#[case::from_the_graveyard(
    GUARD.replace(r#""from":"Battlefield""#, r#""from":"Graveyard""#),
    false
)]
#[case::the_target_sacrificed(
    GUARD.replace(SACRIFICE, r#"{"Sacrifice":{"filter":[{"Is":{"Target":0}}]}}"#),
    false
)]
#[case::an_opponent_sacrifices(
    GUARD.replace(SACRIFICE, r#"{"Sacrifice":{"filter":["This"],"who":"Opponents"}}"#),
    false
)]
#[case::this_leaving(GUARD.replace(r#""what":[{"Is":{"Target":0}}]"#, r#""what":["This"]"#), false)]
#[case::an_artifact_target(
    GUARD.replace(r#"{"Type":"Creature"}]}}]"#, r#"{"Type":"Artifact"}]}}]"#),
    false
)]
fn the_target_leaving_this_turn_reads_back(#[case] json: String, #[case] same: bool) {
    assert_eq!(
        written(&json, GUARD_TEXT),
        same,
        "{json}: {:?}",
        written_variants(&ability(&json))
    );
}
