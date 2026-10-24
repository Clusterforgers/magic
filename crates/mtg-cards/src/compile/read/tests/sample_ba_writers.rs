//! Goal: the writers of the third sample of the names A to F give the oracle wording of the controller of a dead creature as "that player", of damage dealt to you, of a creature that can't be regenerated, of a kicked card as "it", of a doubled power, of energy for each thing, of a counter of an ability, of several destroyed targets, of counters you put on the card, of a token its controller may create, of a creature taken for the turn and of another card discarded; a wrong count, kind, player, event, duration or step never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// "Whenever a creature an opponent controls dies, that player loses 2 life."
fn dies(zone: (&str, &str), who: &str, amount: u32) -> String {
    let (from, to) = zone;
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Creature"}},{{"ControlledBy":"Opponents"}}],"from":"{from}","to":"{to}"}}}},"effects":[{{"LoseLife":{{"who":{who},"amount":{amount}}}}}]}}}}"#
    )
}

const CONTROLLER: &str = r#"{"ControllerOf":"Triggering"}"#;

#[rstest]
#[case::dies(
    ("Battlefield", "Graveyard"),
    CONTROLLER,
    2,
    "Whenever a creature an opponent controls dies, that player loses 2 life."
)]
#[case::the_other_wording(
    ("Battlefield", "Graveyard"),
    CONTROLLER,
    2,
    "Whenever a creature an opponent controls dies, that permanent's controller loses 2 life."
)]
fn the_controller_of_a_dead_creature_is_that_player(
    #[case] zone: (&str, &str),
    #[case] who: &str,
    #[case] amount: u32,
    #[case] clause: &str,
) {
    assert!(written(&dies(zone, who, amount), clause), "{clause}");
}

#[rstest]
#[case::another_amount(
    ("Battlefield", "Graveyard"),
    CONTROLLER,
    2,
    "Whenever a creature an opponent controls dies, that player loses 3 life."
)]
#[case::you_in_the_data(
    ("Battlefield", "Graveyard"),
    r#""You""#,
    2,
    "Whenever a creature an opponent controls dies, that player loses 2 life."
)]
#[case::an_entry_not_a_death(
    ("Library", "Battlefield"),
    CONTROLLER,
    2,
    "Whenever a creature an opponent controls dies, that player loses 2 life."
)]
#[case::an_entry_said_as_one(
    ("Library", "Battlefield"),
    CONTROLLER,
    2,
    "Whenever a creature an opponent controls enters, that player loses 2 life."
)]
fn the_controller_of_a_dead_creature_is_not_that_player_for_another_player_event_or_amount(
    #[case] zone: (&str, &str),
    #[case] who: &str,
    #[case] amount: u32,
    #[case] clause: &str,
) {
    assert!(!written(&dies(zone, who, amount), clause), "{clause}");
}

/// "Whenever you're dealt damage, you may create that many 1/1 white Soldier creature tokens."
const ANY_DAMAGE: &str = "";
const COMBAT_DAMAGE: &str = r#","combat":true"#;
const NONCOMBAT_DAMAGE: &str = r#","combat":false"#;

fn dealt(source: &str, to: &str, combat: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":[{source}],"to":{to}{combat}}}}},"effects":[{{"May":{{"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}}}},"count":"Triggering"}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::damage(
    "",
    r#""You""#,
    ANY_DAMAGE,
    "Whenever you're dealt damage, you may create that many 1/1 white Soldier creature tokens."
)]
#[case::combat_damage(
    "",
    r#""You""#,
    COMBAT_DAMAGE,
    "Whenever you're dealt combat damage, you may create that many 1/1 white Soldier creature tokens."
)]
fn damage_dealt_to_you_reads_as_you_being_dealt_damage(
    #[case] source: &str,
    #[case] to: &str,
    #[case] combat: &str,
    #[case] clause: &str,
) {
    assert!(written(&dealt(source, to, combat), clause), "{clause}");
}

#[rstest]
#[case::combat_in_the_data(
    "",
    r#""You""#,
    COMBAT_DAMAGE,
    "Whenever you're dealt damage, you may create that many 1/1 white Soldier creature tokens."
)]
#[case::any_damage_in_the_data(
    "",
    r#""You""#,
    ANY_DAMAGE,
    "Whenever you're dealt combat damage, you may create that many 1/1 white Soldier creature tokens."
)]
#[case::noncombat_in_the_data(
    "",
    r#""You""#,
    NONCOMBAT_DAMAGE,
    "Whenever you're dealt damage, you may create that many 1/1 white Soldier creature tokens."
)]
#[case::an_opponent(
    "",
    r#""Opponents""#,
    ANY_DAMAGE,
    "Whenever you're dealt damage, you may create that many 1/1 white Soldier creature tokens."
)]
#[case::a_source_of_a_kind(
    r#"{"Type":"Creature"}"#,
    r#""You""#,
    ANY_DAMAGE,
    "Whenever you're dealt damage, you may create that many 1/1 white Soldier creature tokens."
)]
fn damage_dealt_to_you_does_not_read_for_another_kind_player_or_source(
    #[case] source: &str,
    #[case] to: &str,
    #[case] combat: &str,
    #[case] clause: &str,
) {
    assert!(!written(&dealt(source, to, combat), clause), "{clause}");
}

/// "Destroy target creature. A creature destroyed this way can't be regenerated."
fn destroyed(regenerate: bool, kind: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"{kind}"}}]}}}}],"effects":[{{"Destroy":{{"what":{{"Target":0}},"regenerate":{regenerate}}}}}]}}}}"#
    )
}

#[test]
fn a_creature_destroyed_this_way_can_t_be_regenerated() {
    let clause = "Destroy target creature. A creature destroyed this way can't be regenerated.";
    assert!(written(&destroyed(false, "Creature"), clause));
}

#[rstest]
#[case::regeneration_allowed(
    true,
    "Creature",
    "Destroy target creature. A creature destroyed this way can't be regenerated."
)]
#[case::another_kind(
    false,
    "Artifact",
    "Destroy target artifact. A creature destroyed this way can't be regenerated."
)]
#[case::another_object(
    false,
    "Creature",
    "Destroy target creature. A land destroyed this way can't be regenerated."
)]
fn a_creature_destroyed_this_way_reads_only_when_regeneration_is_off_for_a_creature(
    #[case] regenerate: bool,
    #[case] kind: &str,
    #[case] clause: &str,
) {
    assert!(!written(&destroyed(regenerate, kind), clause), "{clause}");
}

/// "When this creature enters, destroy all other creatures you control. If it was kicked, destroy all other creatures instead."
fn kicked(trigger: bool, mine: &str) -> String {
    let body = format!(
        r#""effects":[{{"If":{{"condition":"Kicked","then":[{{"Destroy":{{"what":{{"Each":[{{"Type":"Creature"}},"Other"]}}}}}}],"otherwise":[{{"Destroy":{{"what":{{"Each":[{{"Type":"Creature"}},"Other",{mine}]}}}}}}]}}}}]"#
    );
    if trigger {
        format!(
            r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},{body}}}}}"#
        )
    } else {
        format!(r#"{{"Spell":{{{body}}}}}"#)
    }
}

const MINE: &str = r#"{"ControlledBy":"You"}"#;

#[test]
fn a_kicked_card_is_it_in_its_own_trigger() {
    let clause = "When this creature enters, destroy all other creatures you control. If it was kicked, destroy all other creatures instead.";
    assert!(written(&kicked(true, MINE), clause));
}

#[rstest]
#[case::a_spell(
    false,
    MINE,
    "Destroy all other creatures you control. If it was kicked, destroy all other creatures instead."
)]
#[case::another_group(
    true,
    r#"{"ControlledBy":"Opponents"}"#,
    "When this creature enters, destroy all other creatures you control. If it was kicked, destroy all other creatures instead."
)]
fn a_kicked_card_is_not_it_in_a_spell_and_keeps_its_group(
    #[case] trigger: bool,
    #[case] mine: &str,
    #[case] clause: &str,
) {
    assert!(!written(&kicked(trigger, mine), clause), "{clause}");
}

/// "Whenever another creature you control enters, double this creature's power until end of turn."
fn doubling(of: &str, toughness: u32, duration: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Creature"}},"Other",{{"ControlledBy":"You"}}],"to":"Battlefield"}}}},"effects":[{{"Continuous":{{"affected":"This","mods":[{{"ModifyPT":{{"power":{{"Stat":{{"of":{of},"stat":"Power"}}}},"toughness":{toughness}}}}}],"duration":"{duration}"}}}}]}}}}"#
    )
}

#[test]
fn a_creature_doubles_its_own_power() {
    let clause = "Whenever another creature you control enters, double this creature's power until end of turn.";
    assert!(written(&doubling(r#""This""#, 0, "UntilEndOfTurn"), clause));
}

#[rstest]
#[case::the_power_of_another(
    "\"Triggering\"",
    0,
    "UntilEndOfTurn",
    "Whenever another creature you control enters, double this creature's power until end of turn."
)]
#[case::a_toughness_bonus_as_well(
    "\"This\"",
    1,
    "UntilEndOfTurn",
    "Whenever another creature you control enters, double this creature's power until end of turn."
)]
#[case::another_duration(
    "\"This\"",
    0,
    "UntilYourNextTurn",
    "Whenever another creature you control enters, double this creature's power until end of turn."
)]
fn a_doubled_power_does_not_read_for_another_creature_stat_or_duration(
    #[case] of: &str,
    #[case] toughness: u32,
    #[case] duration: &str,
    #[case] clause: &str,
) {
    assert!(
        !written(&doubling(of, toughness, duration), clause),
        "{clause}"
    );
}

/// "Whenever this creature enters or attacks, you get {E} for each creature you control."
fn energy(selector: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"AnyOf":[{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},{{"Attacks":{{"what":["This"]}}}}]}},"effects":[{{"PutCounters":{{"on":"You","counter":{{"Other":"energy"}},"count":{{"Count":[{selector}]}}}}}}]}}}}"#
    )
}

const CREATURES: &str = r#"{"Type":"Creature"},{"ControlledBy":"You"}"#;

#[test]
fn energy_for_each_thing_reads() {
    let clause =
        "Whenever this creature enters or attacks, you get {E} for each creature you control.";
    assert!(written(&energy(CREATURES), clause));
}

#[rstest]
#[case::more_energy(
    CREATURES,
    "Whenever this creature enters or attacks, you get {E}{E} for each creature you control."
)]
#[case::another_group(
    r#"{"Type":"Land"},{"ControlledBy":"You"}"#,
    "Whenever this creature enters or attacks, you get {E} for each creature you control."
)]
#[case::another_player(
    r#"{"Type":"Creature"},{"ControlledBy":"Opponents"}"#,
    "Whenever this creature enters or attacks, you get {E} for each creature you control."
)]
fn energy_for_each_thing_does_not_read_for_another_amount_or_group(
    #[case] selector: &str,
    #[case] clause: &str,
) {
    assert!(!written(&energy(selector), clause), "{clause}");
}

/// "Counter target activated ability." as a spell and as an ability with a cost.
fn countering(kinds: &str, activated: bool) -> String {
    let target = format!(
        r#""targets":[{{"what":{{"Object":[{kinds}]}}}}],"effects":[{{"Counter":{{"what":{{"Target":0}}}}}}]"#
    );
    if activated {
        format!(r#"{{"Activated":{{"cost":"{{2}}{{U}}",{target}}}}}"#)
    } else {
        format!(r#"{{"Spell":{{{target}}}}}"#)
    }
}

const ACTIVATED: &str = r#"{"Ability":"Activated"}"#;
const TRIGGERED: &str = r#"{"Ability":"Triggered"}"#;

#[rstest]
#[case::an_activated_ability(ACTIVATED, true, "{2}{U}: Counter target activated ability.")]
#[case::a_triggered_ability(TRIGGERED, true, "{2}{U}: Counter target triggered ability.")]
#[case::either(
    r#"{"AnyOf":[{"Ability":"Activated"},{"Ability":"Triggered"}]}"#,
    true,
    "{2}{U}: Counter target activated or triggered ability."
)]
#[case::as_a_spell(ACTIVATED, false, "Counter target activated ability.")]
fn the_counter_of_an_ability_reads(
    #[case] kinds: &str,
    #[case] activated: bool,
    #[case] clause: &str,
) {
    assert!(written(&countering(kinds, activated), clause), "{clause}");
}

#[rstest]
#[case::the_other_kind(ACTIVATED, true, "{2}{U}: Counter target triggered ability.")]
#[case::a_spell(ACTIVATED, true, "{2}{U}: Counter target spell.")]
#[case::another_cost(ACTIVATED, true, "{1}{U}: Counter target activated ability.")]
#[case::both_in_the_data(
    r#"{"AnyOf":[{"Ability":"Activated"},{"Ability":"Triggered"}]}"#,
    true,
    "{2}{U}: Counter target activated ability."
)]
fn the_counter_of_an_ability_does_not_read_for_another_kind_cost_or_target(
    #[case] kinds: &str,
    #[case] activated: bool,
    #[case] clause: &str,
) {
    assert!(!written(&countering(kinds, activated), clause), "{clause}");
}

/// "Destroy target artifact, target creature, target enchantment, and target land."
fn destroys(kinds: &[&str], order: &[usize]) -> String {
    let targets: Vec<String> = kinds
        .iter()
        .map(|kind| format!(r#"{{"what":{{"Object":[{{"Type":"{kind}"}}]}}}}"#))
        .collect();
    let effects: Vec<String> = order
        .iter()
        .map(|at| format!(r#"{{"Destroy":{{"what":{{"Target":{at}}}}}}}"#))
        .collect();
    format!(
        r#"{{"Spell":{{"targets":[{}],"effects":[{}]}}}}"#,
        targets.join(","),
        effects.join(",")
    )
}

#[rstest]
#[case::four(&["Artifact", "Creature", "Enchantment", "Land"], &[0, 1, 2, 3], "Destroy target artifact, target creature, target enchantment, and target land.")]
#[case::three(&["Artifact", "Creature", "Land"], &[0, 1, 2], "Destroy target artifact, target creature, and target land.")]
#[case::two(&["Artifact", "Land"], &[0, 1], "Destroy target artifact and target land.")]
fn several_destroyed_targets_are_one_sentence(
    #[case] kinds: &[&str],
    #[case] order: &[usize],
    #[case] clause: &str,
) {
    assert!(written(&destroys(kinds, order), clause), "{clause}");
}

#[rstest]
#[case::another_order(&["Artifact", "Creature", "Land"], &[0, 1, 2], "Destroy target creature, target artifact, and target land.")]
#[case::a_target_destroyed_twice(&["Artifact", "Creature"], &[0, 0], "Destroy target artifact and target creature.")]
#[case::a_target_left(&["Artifact", "Creature", "Land"], &[0, 1], "Destroy target artifact, target creature, and target land.")]
#[case::the_effects_swapped(&["Artifact", "Creature"], &[1, 0], "Destroy target artifact and target creature.")]
fn several_destroyed_targets_do_not_read_in_another_order_or_count(
    #[case] kinds: &[&str],
    #[case] order: &[usize],
    #[case] clause: &str,
) {
    assert!(!written(&destroys(kinds, order), clause), "{clause}");
}

/// "Whenever you put one or more +1/+1 counters on this creature, draw a card."
fn counters_on_itself(what: &str, counter: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"CountersPut":{{"what":[{what}],"counter":"{counter}"}}}},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

#[rstest]
#[case::plus(
    "\"This\"",
    "+1/+1",
    "Whenever you put one or more +1/+1 counters on this creature, draw a card."
)]
#[case::minus(
    "\"This\"",
    "-1/-1",
    "Whenever you put one or more -1/-1 counters on this creature, draw a card."
)]
fn counters_you_put_on_the_card_read(
    #[case] what: &str,
    #[case] counter: &str,
    #[case] clause: &str,
) {
    assert!(
        written(&counters_on_itself(what, counter), clause),
        "{clause}"
    );
}

#[rstest]
#[case::another_kind(
    "\"This\"",
    "+1/+1",
    "Whenever you put one or more -1/-1 counters on this creature, draw a card."
)]
#[case::another_creature(
    "\"Other\"",
    "+1/+1",
    "Whenever you put one or more +1/+1 counters on this creature, draw a card."
)]
fn counters_you_put_on_the_card_do_not_read_for_another_kind_or_creature(
    #[case] what: &str,
    #[case] counter: &str,
    #[case] clause: &str,
) {
    assert!(
        !written(&counters_on_itself(what, counter), clause),
        "{clause}"
    );
}

/// "Whenever a Sliver deals combat damage to a player, its controller may create a 1/1 colorless Sliver creature token."
fn sliver(chooser: &str, maker: &str, subtype: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":[{{"Subtype":"{subtype}"}}],"to":"EachPlayer","combat":true}}}},"effects":[{{"May":{{"who":{chooser},"effects":[{{"CreateToken":{{"who":{maker},"token":{{"Custom":{{"name":"Sliver","types":["Creature"],"subtypes":["Sliver"],"power":1,"toughness":1}}}}}}}}]}}}}]}}}}"#
    )
}

#[test]
fn the_controller_of_the_creature_of_an_event_may_create_a_token() {
    let clause = "Whenever a Sliver deals combat damage to a player, its controller may create a 1/1 colorless Sliver creature token.";
    assert!(written(&sliver(CONTROLLER, CONTROLLER, "Sliver"), clause));
}

#[rstest]
#[case::you_choose(r#""You""#, CONTROLLER, "Sliver")]
#[case::you_create(CONTROLLER, r#""You""#, "Sliver")]
#[case::another_subtype(CONTROLLER, CONTROLLER, "Elf")]
fn the_controller_may_create_a_token_only_when_the_controller_chooses_and_creates(
    #[case] chooser: &str,
    #[case] maker: &str,
    #[case] subtype: &str,
) {
    let clause = "Whenever a Sliver deals combat damage to a player, its controller may create a 1/1 colorless Sliver creature token.";
    assert!(!written(&sliver(chooser, maker, subtype), clause));
}

/// "Gain control of target creature an opponent controls until end of turn. Untap that creature. It gains haste until end of turn."
fn stolen(controlled: &str, untap: &str, keyword: &str, duration: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}{controlled}]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"SetController":"You"}}],"duration":"UntilEndOfTurn"}}}}{untap},{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"AddKeyword":"{keyword}"}}],"duration":"{duration}"}}}}]}}}}"#
    )
}

const UNTAP: &str = r#",{"Untap":{"what":{"Target":0}}}"#;
const OPPONENT: &str = r#",{"ControlledBy":"Opponents"}"#;

#[rstest]
#[case::any_creature(
    "",
    "It gains haste",
    "Gain control of target creature until end of turn. Untap that creature. It gains haste until end of turn."
)]
#[case::an_opponent_s_creature(
    OPPONENT,
    "It gains haste",
    "Gain control of target creature an opponent controls until end of turn. Untap that creature. It gains haste until end of turn."
)]
fn a_creature_taken_for_the_turn_reads(
    #[case] controlled: &str,
    #[case] _gain: &str,
    #[case] clause: &str,
) {
    assert!(
        written(
            &stolen(controlled, UNTAP, "Haste", "UntilEndOfTurn"),
            clause
        ),
        "{clause}"
    );
}

#[rstest]
#[case::no_untap(
    OPPONENT,
    "",
    "Haste",
    "UntilEndOfTurn",
    "Gain control of target creature an opponent controls until end of turn. Untap that creature. It gains haste until end of turn."
)]
#[case::another_keyword(
    OPPONENT,
    UNTAP,
    "Trample",
    "UntilEndOfTurn",
    "Gain control of target creature an opponent controls until end of turn. Untap that creature. It gains haste until end of turn."
)]
#[case::another_duration(
    OPPONENT,
    UNTAP,
    "Haste",
    "Forever",
    "Gain control of target creature an opponent controls until end of turn. Untap that creature. It gains haste until end of turn."
)]
#[case::another_group(
    OPPONENT,
    UNTAP,
    "Haste",
    "UntilEndOfTurn",
    "Gain control of target creature until end of turn. Untap that creature. It gains haste until end of turn."
)]
fn a_creature_taken_for_the_turn_does_not_read_without_its_untap_or_with_another_keyword_duration_or_group(
    #[case] controlled: &str,
    #[case] untap: &str,
    #[case] keyword: &str,
    #[case] duration: &str,
    #[case] clause: &str,
) {
    assert!(
        !written(&stolen(controlled, untap, keyword, duration), clause),
        "{clause}"
    );
}

/// "Whenever you discard another card, scry 1."
fn discarding(what: &str, by: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Discarded":{{"what":[{what}],"by":{by}}}}},"effects":[{{"Scry":{{}}}}]}}}}"#
    )
}

#[rstest]
#[case::another_card(r#""Other""#, r#""You""#, "Whenever you discard another card, scry 1.")]
#[case::a_card("", r#""You""#, "Whenever you discard a card, scry 1.")]
fn a_discard_of_another_card_reads(#[case] what: &str, #[case] by: &str, #[case] clause: &str) {
    assert!(written(&discarding(what, by), clause), "{clause}");
}

#[rstest]
#[case::another_card_for_a_card(r#""Other""#, r#""You""#, "Whenever you discard a card, scry 1.")]
#[case::a_card_for_another_card("", r#""You""#, "Whenever you discard another card, scry 1.")]
#[case::an_opponent(
    r#""Other""#,
    r#""Opponents""#,
    "Whenever you discard another card, scry 1."
)]
fn a_discard_of_another_card_does_not_read_for_a_card_or_another_player(
    #[case] what: &str,
    #[case] by: &str,
    #[case] clause: &str,
) {
    assert!(!written(&discarding(what, by), clause), "{clause}");
}
