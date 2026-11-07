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

const ACCURSED: &str = r#"{"Spell":{"effects":[{"Together":{"effects":[{"DealDamage":{"amount":5,"to":{"Target":0}}},{"Shield":{"by":{"MoveTo":"Exile"},"duration":"UntilEndOfTurn","when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Is":{"Target":0}},{"Type":"Creature"}]}}}},{"DealDamage":{"amount":2,"to":{"ControllerOf":{"Target":0}}}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const ACCURSED_TEXT: &str = "~ deals 5 damage to target creature and 2 damage to that creature's controller. If that creature would die this turn, exile it instead.";

#[rstest]
#[case::accursed(ACCURSED.to_owned(), ACCURSED_TEXT, true)]
#[case::accursed_for_good(ACCURSED.replace(r#""UntilEndOfTurn""#, r#""Forever""#), ACCURSED_TEXT, false)]
#[case::accursed_to_hand(ACCURSED.replace(r#""MoveTo":"Exile""#, r#""MoveTo":"Hand""#), ACCURSED_TEXT, false)]
#[case::accursed_other_amount(ACCURSED.replace(r#""amount":2"#, r#""amount":3"#), ACCURSED_TEXT, false)]
#[case::wing_storm(
    each_player("EachPlayer", r#"{"Times":{"by":2,"of":{"Count":[{"Type":"Creature"},{"HasKeyword":"Flying"},{"ControlledBy":{"Bound":"each"}}]}}}"#),
    "~ deals damage to each player equal to twice the number of creatures that player controls with flying.",
    true
)]
#[case::wing_storm_reach(
    each_player("EachPlayer", r#"{"Times":{"by":2,"of":{"Count":[{"Type":"Creature"},{"HasKeyword":"Reach"},{"ControlledBy":{"Bound":"each"}}]}}}"#),
    "~ deals damage to each player equal to twice the number of creatures that player controls with flying.",
    false
)]
fn a_death_shield_in_a_batch_reads_after_the_damage(
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

const WHIPKEEPER: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"DealDamage":{"amount":{"Tallied":{"what":"DamageDealtTo","who":WHO}},"to":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const WHIPKEEPER_TEXT: &str = "{T}: This creature deals damage to target creature equal to the damage already dealt to it this turn.";
const HORNBEETLE: &str = r#"{"Triggered":{"effects":[{"CreateToken":{"count":{"Tally":{"event":"PlusOneCountersPut","filter":FILTER,"who":"You"}},"token":{"Custom":{"colors":["Green"],"name":"Insect","power":1,"subtypes":["Insect"],"toughness":1,"types":["Creature"]}}}}],"when":{"StepBegins":{"step":"End","whose":"You"}}}}"#;
const HORNBEETLE_TEXT: &str = "At the beginning of your end step, create a 1/1 green Insect creature token for each +1/+1 counter you've put on creatures under your control this turn.";
const COLONY: &str = r#"{"Triggered":{"effects":[{"CreateToken":{"count":{"Tallied":{"what":"WHAT","who":"This"}},"token":{"Custom":{"colors":["Black"],"name":"Rat","power":1,"subtypes":["Rat"],"toughness":1,"types":["Creature"]}}}}],"when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":["This"]}}}}"#;
const COLONY_TEXT: &str = "When this creature dies, create X 1/1 black Rat creature tokens, where X is the amount of damage dealt to it this turn.";

#[rstest]
#[case::whipkeeper(WHIPKEEPER.replace("WHO", r#"{"Target":0}"#), WHIPKEEPER_TEXT, true)]
#[case::whipkeeper_own_damage(WHIPKEEPER.replace("WHO", r#""This""#), WHIPKEEPER_TEXT, false)]
#[case::whipkeeper_damage_taken(
    WHIPKEEPER.replace("WHO", r#"{"Target":0}"#).replace("DamageDealtTo", "DamageTaken"),
    WHIPKEEPER_TEXT,
    false
)]
#[case::hornbeetle(
    HORNBEETLE.replace("FILTER", r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#),
    HORNBEETLE_TEXT,
    true
)]
#[case::hornbeetle_any_creature(
    HORNBEETLE.replace("FILTER", r#"[{"Type":"Creature"}]"#),
    HORNBEETLE_TEXT,
    false
)]
#[case::hornbeetle_anyone(
    HORNBEETLE.replace("FILTER", r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#).replace(r#""who":"You"}}"#, r#""who":"EachPlayer"}}"#),
    HORNBEETLE_TEXT,
    false
)]
#[case::colony(COLONY.replace("WHAT", "DamageDealtTo"), COLONY_TEXT, true)]
#[case::colony_cards_graved(COLONY.replace("WHAT", "CardsGraved"), COLONY_TEXT, false)]
fn facts_of_the_turn_as_amounts_read_back(
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

const RISE: &str = r#"{"Spell":{"effects":[{"Move":{"as":"moved","to":{"Battlefield":{"under":"You"}},"what":{"Target":0}}},{"Continuous":{"affected":{"Bound":"moved"},"duration":"Forever","mods":[{"AddSubtype":"Zombie"},{"AddColors":["Black"]}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}]}}"#;
const RISE_TEXT: &str = "Put target creature card from a graveyard onto the battlefield under your control. That creature is a black Zombie in addition to its other colors and types.";

#[rstest]
#[case::rise(RISE.to_owned(), true)]
#[case::until_end_of_turn(RISE.replace(r#""Forever""#, r#""UntilEndOfTurn""#), false)]
#[case::other_color(RISE.replace(r#"["Black"]"#, r#"["Blue"]"#), false)]
#[case::colors_set(RISE.replace("AddColors", "SetColors"), false)]
#[case::the_target_instead(RISE.replace(r#"{"Bound":"moved"},"duration""#, r#""This","duration""#), false)]
#[case::to_the_hand(RISE.replace(r#"{"Battlefield":{"under":"You"}}"#, r#""Hand""#), false)]
fn returned_as_a_zombie_reads_back(#[case] json: String, #[case] same: bool) {
    assert_eq!(
        written(&json, RISE_TEXT),
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
