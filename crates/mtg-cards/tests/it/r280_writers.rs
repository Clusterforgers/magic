//! Goal: the writers of the r28-0 pack say each shape they read as the printed wording, and a near miss of each shape
//! (another zone, player, duration, kind or count) is never said that way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::written_variants;
use rstest::rstest;

/// The wordings the writers give an ability written as JSON.
fn written(json: &str) -> Vec<String> {
    let ability: Ability = serde_json::from_str(json).unwrap();
    written_variants(&ability)
}

/// A card revealed at random from the hand of the first target, then `rest` as more steps.
fn revealed(zone: &str, rest: &str, targets: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":"{{3}}{{W}}","targets":[{targets}],"effects":[{{"ChooseAtRandom":{{"from":{{"Each":[{{"OwnedBy":{{"Target":0}}}},{{"In":"{zone}"}}]}},"as":"revealed"}}}},{{"Reveal":{{"what":{{"Bound":"revealed"}}}}}},{rest}]}}}}"#
    )
}

const VALUE: &str = r#"{"Stat":{"of":{"Bound":"revealed"},"stat":"ManaValue"}}"#;
const OPPONENT: &str = r#"{"what":"Opponent"}"#;

#[rstest]
#[case::life(
    "Hand",
    format!(r#"{{"GainLife":{{"amount":{VALUE}}}}}"#),
    OPPONENT.to_owned(),
    "{3}{W}: Target opponent reveals a card at random from their hand. You gain life equal to that card's mana value."
)]
#[case::damage(
    "Hand",
    format!(r#"{{"DealDamage":{{"to":{{"Target":0}},"amount":{VALUE}}}}}"#),
    OPPONENT.to_owned(),
    "{3}{W}: Target opponent reveals a card at random from their hand. ~ deals damage equal to that card's mana value to that player."
)]
#[case::bonus(
    "Hand",
    format!(r#"{{"Continuous":{{"affected":{{"Target":1}},"mods":[{{"ModifyPT":{{"power":{VALUE},"toughness":{VALUE}}}}}],"duration":"UntilEndOfTurn"}}}}"#),
    format!(r#"{OPPONENT},{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}"#),
    "{3}{W}: Target opponent reveals a card at random from their hand. Target creature gets +X/+X until end of turn, where X is the revealed card's mana value."
)]
fn a_card_revealed_at_random_is_read_by_its_mana_value(
    #[case] zone: &str,
    #[case] rest: String,
    #[case] targets: String,
    #[case] wanted: &str,
) {
    let texts = written(&revealed(zone, &rest, &targets));
    assert!(texts.iter().any(|text| text == wanted), "{texts:?}");
}

#[rstest]
#[case::from_the_library("Library")]
#[case::from_the_graveyard("Graveyard")]
fn a_card_picked_from_another_zone_is_not_revealed_from_the_hand(#[case] zone: &str) {
    let rest = format!(r#"{{"GainLife":{{"amount":{VALUE}}}}}"#);
    let texts = written(&revealed(zone, &rest, OPPONENT));
    assert!(
        !texts.iter().any(
            |text| text.contains("at random from their hand") || text.contains("revealed card")
        ),
        "{texts:?}"
    );
}

#[rstest]
#[case::mine(r#""You""#, r#""UntilTurnSkipped""#, true)]
#[case::the_opponents(r#""Opponents""#, r#""UntilTurnSkipped""#, false)]
#[case::for_good(r#""You""#, r#""Forever""#, false)]
fn the_next_turn_skipped_is_said_only_of_your_next_turn(
    #[case] who: &str,
    #[case] duration: &str,
    #[case] said: bool,
) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{0}}","effects":[{{"Continuous":{{"affected":"This","mods":[{{"ModifyPT":{{"power":3,"toughness":3}}}}],"duration":"UntilEndOfTurn"}}}},{{"Rule":{{"rule":{{"Cant":{{"who":{who},"action":"SkipTurn"}}}},"duration":{duration}}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "{0}: ~ gets +3/+3 until end of turn. You skip your next turn.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::spell_or_permanent(r#"[{"AnyOf":["Spell",{"In":"Battlefield"}]}]"#, true)]
#[case::a_spell(r#"["Spell"]"#, false)]
#[case::a_creature(r#"[{"Type":"Creature"}]"#, false)]
fn a_target_spell_or_permanent_is_said_as_such(#[case] what: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{1}}{{U}}","targets":[{{"what":{{"Object":{what}}}}}],"effects":[{{"ChooseValue":{{"kind":"Color","as":"chosen"}}}},{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"SetColorsChosen":"chosen"}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted =
        "{1}{U}: Target spell or permanent becomes the color of your choice until end of turn.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::any_permanent(
    r#"[{"Named":"Wirefly"}]"#,
    Some("Destroy all permanents named Wirefly.")
)]
#[case::creatures(
    r#"[{"Type":"Creature"},{"Named":"Wirefly"}]"#,
    Some("Destroy all creatures named Wirefly.")
)]
fn permanents_with_a_name_are_said_with_their_noun(
    #[case] filter: &str,
    #[case] wanted: Option<&str>,
) {
    let json =
        format!(r#"{{"Spell":{{"effects":[{{"Destroy":{{"what":{{"Each":{filter}}}}}}}]}}}}"#);
    let texts = written(&json);
    assert!(
        wanted.is_none_or(|wanted| texts.iter().any(|text| text == wanted)),
        "{texts:?}"
    );
    if filter.contains("Creature") {
        assert!(
            !texts.iter().any(|text| text.contains("permanents named")),
            "{texts:?}"
        );
    }
}

#[rstest]
#[case::restricted(
    r#"["CantAttack","CantBlock"]"#,
    "Each creature with mana value X can't attack or block this turn."
)]
#[case::pumped(
    r#"[{"ModifyPT":{"power":1,"toughness":1}}]"#,
    "Each creature with mana value X gets +1/+1 until end of turn."
)]
fn a_group_restriction_is_also_said_of_each_creature(#[case] mods: &str, #[case] wanted: &str) {
    let json = format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{X}}"}},"Tap"],"effects":[{{"Continuous":{{"affected":{{"Each":[{{"Type":"Creature"}},{{"Compare":{{"stat":"ManaValue","op":"Eq","to":"X"}}}}]}},"mods":{mods},"duration":"UntilEndOfTurn"}}}}]}}}}"#
    );
    let texts = written(&json);
    assert!(
        texts
            .iter()
            .any(|text| text == &format!("{{X}}, {{T}}: {wanted}")),
        "{texts:?}"
    );
    // The plural verb is never kept after "each creature".
    assert!(
        !texts
            .iter()
            .any(|text| text.contains("Each creature with mana value X get ")),
        "{texts:?}"
    );
}

#[rstest]
#[case::x_lands(r#","bounds":{"min":"X","max":"X"}"#, true)]
#[case::one_land("", false)]
fn x_target_lands_become_a_land_type(#[case] bounds: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{X}}{{G}}{{G}}","targets":[{{"what":{{"Object":[{{"Type":"Land"}}]}}{bounds}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":[{{"SetLandTypes":["Forest"]}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "{X}{G}{G}: X target lands become Forests until end of turn.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::every_type(r#","AddAllCreatureTypes""#, true)]
#[case::no_type("", false)]
fn a_land_becomes_a_creature_with_all_creature_types(#[case] all: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{4}}","effects":[{{"Continuous":{{"affected":"This","mods":[{{"SetPT":{{"power":3,"toughness":3}}}},{{"AddType":"Creature"}}{all},{{"AddKeyword":"Vigilance"}}],"duration":"Forever"}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted =
        "{4}: ~ becomes a 3/3 creature with vigilance and all creature types. It's still a land.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::any_graveyard(r#","any_player":true"#, true)]
#[case::yours("", false)]
fn a_card_exiled_from_a_graveyard_and_counters_on_each_of_a_kind(
    #[case] any: &str,
    #[case] said: bool,
) {
    let json = format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{B}}{{G}}"}},{{"ExileFromGraveyard":{{"filter":[{{"Subtype":"Fungus"}}]{any}}}}}],"effects":[{{"PutCounters":{{"on":{{"Each":[{{"Subtype":"Fungus"}}]}},"counter":{{"Other":"spore"}}}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "{B}{G}, Exile a Fungus card from a graveyard: Put a spore counter on each Fungus on the battlefield.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::this_turn("true", true)]
#[case::any_time("false", false)]
fn a_creature_that_dies_this_turn_returns_to_its_owners_hand(
    #[case] this_turn: &str,
    #[case] said: bool,
) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{1}}","targets":[{{"what":{{"Object":[{{"Type":"Creature"}},"HasAnyCounter"]}}}}],"effects":[{{"DelayedTrigger":{{"when":{{"ZoneChange":{{"what":[{{"Is":{{"Target":0}}}}],"from":"Battlefield","to":"Graveyard"}}}},"this_turn":{this_turn},"effects":[{{"Move":{{"what":"Triggering","to":"Hand"}}}}]}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "{1}: Choose target creature with a counter on it. When that creature dies this turn, return that card to its owner's hand.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::only_this(r#"["Spell","SingleTarget",{"Targeting":["This"]}]"#, true)]
#[case::any_single(r#"["Spell","SingleTarget"]"#, false)]
fn a_spell_that_targets_only_the_source(#[case] what: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{R}}","targets":[{{"what":{{"Object":{what}}}}}],"effects":[{{"ChangeTargets":{{"what":{{"Target":0}},"to":"Another"}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "{R}: Change the target of target spell that targets only ~.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::an_enchantment(
    r#"[{"Type":"Enchantment"}]"#,
    Some("if ~ is an enchantment, it becomes")
)]
#[case::a_tapped_enchantment(r#"[{"Type":"Enchantment"},"Tapped"]"#, None)]
fn an_intervening_if_on_the_sources_card_type(#[case] selector: &str, #[case] said: Option<&str>) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":[{{"Type":"Creature"}}],"by":"Opponents"}}}},"if":{{"Matches":{{"subject":"This","selector":{selector}}}}},"effects":[{{"Continuous":{{"affected":"This","mods":[{{"SetPT":{{"power":2,"toughness":4}}}},{{"AddType":"Creature"}},{{"SetCreatureTypes":["Soldier"]}}],"duration":"Forever"}}}}]}}}}"#
    );
    let texts = written(&json);
    match said {
        Some(words) => assert!(texts.iter().any(|text| text.contains(words)), "{texts:?}"),
        None => assert!(
            !texts.iter().any(|text| text.contains(" is an enchantment")),
            "{texts:?}"
        ),
    }
}

#[rstest]
#[case::for_good(r#""Forever""#, true)]
#[case::this_turn(r#""UntilEndOfTurn""#, false)]
fn a_permanent_becomes_one_card_type(#[case] duration: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{0}}","effects":[{{"Continuous":{{"affected":"This","mods":[{{"SetTypes":["Enchantment"]}}],"duration":{duration}}}}}]}}}}"#
    );
    let texts = written(&json);
    assert_eq!(
        texts
            .iter()
            .any(|text| text == "{0}: ~ becomes an enchantment."),
        said,
        "{texts:?}"
    );
}

#[rstest]
#[case::taken(
    r#"{"Move":{"what":"Triggering","to":{"Battlefield":{"under":"You"}}}}"#,
    true
)]
#[case::returned_to_hand(r#"{"Move":{"what":"Triggering","to":"Hand"}}"#, false)]
fn a_permanent_put_into_a_graveyard_is_taken_unless_paid(#[case] unpaid: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"ControlledBy":"Opponents"}}],"from":"Battlefield","to":"Graveyard"}}}},"effects":[{{"Pay":{{"who":{{"ControllerOf":"Triggering"}},"cost":[{{"PayLife":3}}],"unpaid":[{unpaid}]}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "Whenever a permanent an opponent controls is put into a graveyard, put that card onto the battlefield under your control unless that opponent pays 3 life.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::the_sources_controller(r#"{"ControllerOf":"This"}"#, Some("~'s controller loses 2 life"))]
#[case::the_targets_controller(r#"{"ControllerOf":{"Target":0}}"#, None)]
fn the_controller_of_the_source_loses_life(#[case] who: &str, #[case] said: Option<&str>) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{3}}","by":"Anyone","effects":[{{"LoseLife":{{"who":{who},"amount":2}}}},{{"Draw":{{}}}}]}}}}"#
    );
    let texts = written(&json);
    match said {
        Some(words) => assert!(texts.iter().any(|text| text.contains(words)), "{texts:?}"),
        None => assert!(
            !texts.iter().any(|text| text.contains("~'s controller")),
            "{texts:?}"
        ),
    }
}

#[rstest]
#[case::at_least_one(r#",{"XAtLeast":1}"#, true)]
#[case::any_x("", false)]
fn an_x_that_cannot_be_zero(#[case] least: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{X}}{{U}}{{U}}{{B}}{{B}}"}},{{"ExileFromGraveyard":{{"filter":[{{"Type":"Creature"}}],"count":"X"}}}}{least}],"timing":"Sorcery","effects":[{{"Transform":{{"what":"This"}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "{X}{U}{U}{B}{B}, Exile X creature cards from your graveyard: Transform ~. X can't be 0. Activate only as a sorcery.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::and_vehicles(
    r#"[{"AnyOf":[{"Type":"Creature"},{"Subtype":"Vehicle"}]},{"ControlledBy":"You"}]"#,
    true
)]
#[case::creatures_alone(r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#, false)]
fn creatures_and_vehicles_gain_a_keyword(#[case] each: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{4}}{{R}}{{G}}","effects":[{{"Continuous":{{"affected":{{"Each":{each}}},"mods":[{{"AddKeyword":"Trample"}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "{4}{R}{G}: Creatures and Vehicles you control gain trample until end of turn.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::while_tapped(
    r#"["LoseAbilities","CantAttack","CantBlock"],"duration":"WhileSourceTapped""#,
    true
)]
#[case::this_turn(
    r#"["LoseAbilities","CantAttack","CantBlock"],"duration":"UntilEndOfTurn""#,
    false
)]
fn a_target_loses_its_abilities_while_the_source_stays_tapped(
    #[case] mods: &str,
    #[case] said: bool,
) {
    let json = format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{3}}{{W}}"}},"Tap"],"targets":[{{"what":{{"Object":["Other"]}}}}],"effects":[{{"Continuous":{{"affected":{{"Target":0}},"mods":{mods}}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "{3}{W}, {T}: For as long as ~ remains tapped, another target permanent loses all abilities and can't attack or block.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::face_down(
    r#"{"BattlefieldFaceDown":{"under":{"OwnerOf":{"Bound":"exiled"}}}}"#,
    true
)]
#[case::face_up(r#"{"Battlefield":{}}"#, false)]
fn the_source_returns_face_down(#[case] to: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{U}}","effects":[{{"Exile":{{"what":"This","as":"exiled"}}}},{{"Move":{{"what":{{"Bound":"exiled"}},"to":{to}}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted =
        "{U}: Exile ~, then return it to the battlefield face down under its owner's control.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::yours(r#"{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#, true)]
#[case::the_source(r#""This""#, false)]
fn each_creature_you_control_becomes_prepared(#[case] what: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{1}}","effects":[{{"Prepare":{{"what":{what}}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "{1}: Each creature you control becomes prepared.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

/// An activation that names a card and reveals one at random from the hand, then `then` if it has the name.
fn named(hand: &str, selector: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{3}}"}},"Tap"],"targets":[{{"what":"AnyTarget"}}],"effects":[{{"ChooseValue":{{"kind":{{"CardName":[]}},"as":"name"}}}},{{"ChooseAtRandom":{{"from":{{"Each":{hand}}},"as":"revealed"}}}},{{"Reveal":{{"what":{{"Bound":"revealed"}}}}}},{{"If":{{"condition":{{"Matches":{{"subject":{{"Bound":"revealed"}},"selector":{selector}}}}},"then":[{{"DealDamage":{{"to":{{"Target":0}},"amount":2}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::your_hand(
    r#"[{"OwnedBy":"You"},{"In":"Hand"}]"#,
    r#"[{"ChosenAs":"name"}]"#,
    true
)]
#[case::an_opponents_hand(
    r#"[{"OwnedBy":"Opponents"},{"In":"Hand"}]"#,
    r#"[{"ChosenAs":"name"}]"#,
    false
)]
#[case::another_test(r#"[{"OwnedBy":"You"},{"In":"Hand"}]"#, r#"[{"Type":"Land"}]"#, false)]
fn a_card_named_then_revealed_at_random(
    #[case] hand: &str,
    #[case] selector: &str,
    #[case] said: bool,
) {
    let texts = written(&named(hand, selector));
    let wanted = "{3}, {T}: Choose a card name, then reveal a card at random from your hand. If that card has the chosen name, ~ deals 2 damage to any target.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::for_good(r#""Forever""#, true)]
#[case::this_turn(r#""UntilEndOfTurn""#, false)]
fn the_source_becomes_new_creature_types_for_good(#[case] duration: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{1}}{{W}}"}},{{"CollectEvidence":2}}],"effects":[{{"Continuous":{{"affected":"This","mods":[{{"SetCreatureTypes":["Human","Detective"]}},{{"SetPT":{{"power":4,"toughness":4}}}},{{"AddKeyword":"Vigilance"}}],"duration":{duration}}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "{1}{W}, Collect evidence 2: ~ becomes a Human Detective with base power and toughness 4/4 and gains vigilance.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}

#[rstest]
#[case::a_citizen(r#"[{"Subtype":"Citizen"}]"#, true)]
#[case::a_tapped_citizen(r#"[{"Subtype":"Citizen"},"Tapped"]"#, false)]
fn the_source_of_a_creature_type_becomes_another(#[case] selector: &str, #[case] said: bool) {
    let json = format!(
        r#"{{"Activated":{{"cost":"{{W/B}}{{W/B}}","effects":[{{"If":{{"condition":{{"Matches":{{"subject":"This","selector":{selector}}}}},"then":[{{"Continuous":{{"affected":"This","mods":[{{"SetCreatureTypes":["Halfling","Scout"]}},{{"SetPT":{{"power":2,"toughness":3}}}},{{"AddKeyword":"Lifelink"}}],"duration":"Forever"}}}}]}}}}]}}}}"#
    );
    let texts = written(&json);
    let wanted = "{W/B}{W/B}: If ~ is a Citizen, it becomes a Halfling Scout with base power and toughness 2/3 and lifelink.";
    assert_eq!(texts.iter().any(|text| text == wanted), said, "{texts:?}");
}
