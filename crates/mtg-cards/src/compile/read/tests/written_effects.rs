//! Goal: the writers of the proposal checker give the oracle wording of the effects a model's rules use that no template reads (divided damage, an exile of a graveyard, tokens for a chosen player or with protection, a pump the player may choose, a card put onto the battlefield from the hand, damage by the creature of an event, life paid as X, a subtype or "nonland permanent" card), and a wrong player, count or target never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const ENTERS: &str = r#"{"ZoneChange":{"what":["This"],"to":"Battlefield"}}"#;

fn on_entering(targets: &str, effects: &str) -> String {
    format!(r#"{{"Triggered":{{"when":{ENTERS},"targets":{targets},"effects":{effects}}}}}"#)
}

#[rstest]
#[case::inferno_titan(
    r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]},"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":3},"divide":3}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
    "Whenever this creature enters or attacks, it deals 3 damage divided as you choose among one, two, or three targets."
)]
#[case::bogardan_hellkite(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":null},"divide":5}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
    "When this creature enters, it deals 5 damage divided as you choose among any number of targets."
)]
#[case::dragonlord_atarka(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},{"ControlledBy":"Opponents"}]},"bounds":{"min":1,"max":null},"divide":5}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
    "When ~ enters, it deals 5 damage divided as you choose among any number of target creatures and/or planeswalkers your opponents control."
)]
#[case::fury(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]},"bounds":{"min":1,"max":null},"divide":4}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
    "When this creature enters, it deals 4 damage divided as you choose among any number of target creatures and/or planeswalkers."
)]
#[case::violent_eruption(
    r#"{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":null},"divide":4}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
    "~ deals 4 damage divided as you choose among any number of targets."
)]
#[case::up_to_x_creatures(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":"X"},"divide":"X"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
    "When this creature enters, it deals X damage divided as you choose among up to X target creatures."
)]
fn divided_damage_is_written_as_the_oracle_words_it(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_total(
    "Whenever this creature enters or attacks, it deals 4 damage divided as you choose among one, two, or three targets."
)]
#[case::one_or_two_for_one_to_three(
    "Whenever this creature enters or attacks, it deals 3 damage divided as you choose among one or two targets."
)]
#[case::any_number_for_one_to_three(
    "Whenever this creature enters or attacks, it deals 3 damage divided as you choose among any number of targets."
)]
#[case::one_target_not_divided(
    "Whenever this creature enters or attacks, it deals 3 damage to any target."
)]
fn divided_damage_does_not_read_as_another_total_or_count(#[case] clause: &str) {
    let json = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]},"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":3},"divide":3}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#;
    assert!(!written(json, clause), "{clause}");
}

const PLAYER_GRAVEYARD: &str = r#"{"Each":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}"#;

#[rstest]
#[case::angel_of_finality(
    on_entering(r#"[{"what":"Player"}]"#, &format!(r#"[{{"Exile":{{"what":{PLAYER_GRAVEYARD}}}}}]"#)),
    "When this creature enters, exile target player's graveyard."
)]
#[case::an_opponents_graveyard(
    on_entering(r#"[{"what":"Opponent"}]"#, &format!(r#"[{{"Exile":{{"what":{PLAYER_GRAVEYARD}}}}}]"#)),
    "When this creature enters, exile target opponent's graveyard."
)]
#[case::remorseful_cleric(
    format!(r#"{{"Activated":{{"cost":["SacrificeSelf"],"targets":[{{"what":"Player"}}],"effects":[{{"Exile":{{"what":{PLAYER_GRAVEYARD}}}}}]}}}}"#),
    "Sacrifice this creature: Exile target player's graveyard."
)]
#[case::nihil_spellbomb(
    format!(r#"{{"Activated":{{"cost":["Tap","SacrificeSelf"],"targets":[{{"what":"Player"}}],"effects":[{{"Exile":{{"what":{PLAYER_GRAVEYARD}}}}}]}}}}"#),
    "{T}, Sacrifice this artifact: Exile target player's graveyard."
)]
#[case::each_opponents_graveyard(
    r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"Exile":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}}}]}}"#.to_owned(),
    "{T}, Sacrifice this artifact: Exile each opponent's graveyard."
)]
fn the_exile_of_a_graveyard_is_written_as_the_oracle_words_it(
    #[case] json: String,
    #[case] clause: &str,
) {
    assert!(written(&json, clause), "{clause}");
}

#[rstest]
#[case::an_opponent_for_a_player("When this creature enters, exile target opponent's graveyard.")]
#[case::the_cards_of_a_creature("When this creature enters, exile target creature.")]
#[case::your_own("When this creature enters, exile your graveyard.")]
#[case::a_hand("When this creature enters, exile target player's hand.")]
fn an_exile_of_a_players_graveyard_does_not_read_as_another_zone_or_player(#[case] clause: &str) {
    let json = on_entering(
        r#"[{"what":"Player"}]"#,
        &format!(r#"[{{"Exile":{{"what":{PLAYER_GRAVEYARD}}}}}]"#),
    );
    assert!(!written(&json, clause), "{clause}");
}

const CENTAURS: &str = r#"{"CreateToken":{"who":{"Target":0},"count":2,"token":{"Custom":{"name":"Centaur","colors":["Green"],"types":["Creature"],"subtypes":["Centaur"],"power":3,"toughness":3,"abilities":[{"Keyword":{"Protection":[{"Color":"Black"}]}}]}}}}"#;

#[rstest]
#[case::hunted_horror(
    "When this creature enters, target opponent creates two 3/3 green Centaur creature tokens with protection from black.",
    true
)]
#[case::a_player_for_an_opponent(
    "When this creature enters, target player creates two 3/3 green Centaur creature tokens with protection from black.",
    false
)]
#[case::another_count(
    "When this creature enters, target opponent creates three 3/3 green Centaur creature tokens with protection from black.",
    false
)]
#[case::another_quality(
    "When this creature enters, target opponent creates two 3/3 green Centaur creature tokens with protection from white.",
    false
)]
#[case::no_quality(
    "When this creature enters, target opponent creates two 3/3 green Centaur creature tokens.",
    false
)]
#[case::the_controller_instead(
    "When this creature enters, you create two 3/3 green Centaur creature tokens with protection from black.",
    false
)]
fn tokens_for_a_chosen_player_with_protection(#[case] clause: &str, #[case] reads: bool) {
    let json = on_entering(r#"[{"what":"Opponent"}]"#, &format!("[{CENTAURS}]"));
    assert_eq!(written(&json, clause), reads, "{clause}");
}

const CULTIVATOR: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"May":{"effects":[{"Continuous":{"affected":{"Each":["Other","Attacking",{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":{"Stat":{"of":"This","stat":"Power"}},"toughness":{"Stat":{"of":"This","stat":"Power"}}}}],"duration":"UntilEndOfTurn"}}]}}]}}"#;

#[rstest]
#[case::cultivator_of_blades(
    "Whenever this creature attacks, you may have other attacking creatures get +X/+X until end of turn, where X is this creature's power.",
    true
)]
#[case::without_the_choice(
    "Whenever this creature attacks, other attacking creatures get +X/+X until end of turn, where X is this creature's power.",
    false
)]
#[case::another_characteristic(
    "Whenever this creature attacks, you may have other attacking creatures get +X/+X until end of turn, where X is this creature's toughness.",
    false
)]
#[case::all_attackers(
    "Whenever this creature attacks, you may have attacking creatures get +X/+X until end of turn, where X is this creature's power.",
    false
)]
fn a_pump_the_player_may_choose(#[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(CULTIVATOR, clause), reads, "{clause}");
}

const MASTERMIND: &str = r#"{"Triggered":{"when":{"Nth":{"event":{"Drew":{"who":"Opponents"}},"nth":2}},"effects":[{"Draw":{}}]}}"#;
const OWN_SECOND_DRAW: &str = r#"{"Triggered":{"when":{"Nth":{"event":{"Drew":{"who":"You"}},"nth":2}},"effects":[{"Draw":{}}]}}"#;

#[rstest]
#[case::faerie_mastermind(
    MASTERMIND,
    "Whenever an opponent draws their second card each turn, you draw a card.",
    true
)]
#[case::without_you(
    MASTERMIND,
    "Whenever an opponent draws their second card each turn, draw a card.",
    true
)]
#[case::another_verb(
    MASTERMIND,
    "Whenever an opponent draws their second card each turn, you discard a card.",
    false
)]
#[case::an_event_of_your_own(
    OWN_SECOND_DRAW,
    "Whenever you draw your second card each turn, you draw a card.",
    false
)]
fn you_after_the_event_of_another_player(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(json, clause), reads, "{clause}");
}

const PYREMAW: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}},"targets":[{"what":"Opponent"}],"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}},{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":"This","stat":"Power"}}}}]}}"#;
const BELAKOR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["Other",{"Subtype":"Demon"},{"ControlledBy":"You"}],"to":"Battlefield"}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"source":"Triggering","to":{"Target":0},"amount":{"Stat":{"of":"Triggering","stat":"Power"}}}}]}}"#;
const TYRANNOSAURUS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"DealDamage":{"to":"Opponents","amount":{"Stat":{"of":"This","stat":"Power"}}}}]}}"#;

#[rstest]
#[case::caldera_pyremaw(
    PYREMAW,
    "Whenever you cast an instant or sorcery spell, put a +1/+1 counter on this creature. Then this creature deals damage equal to its power to target opponent.",
    true
)]
#[case::caldera_pyremaw_as_one_sentence(
    PYREMAW,
    "Whenever you cast an instant or sorcery spell, put a +1/+1 counter on this creature. This creature deals damage equal to its power to target opponent.",
    true
)]
#[case::belakor(
    BELAKOR,
    "Whenever another Demon you control enters, it deals damage equal to its power to any target.",
    true
)]
#[case::flayer_of_the_hatebound(
    BELAKOR,
    "Whenever another Demon you control enters, that creature deals damage equal to its power to any target.",
    true
)]
#[case::flaming_tyrannosaurus(
    TYRANNOSAURUS,
    "When this creature dies, it deals damage equal to its power to each opponent.",
    true
)]
#[case::its_toughness(
    TYRANNOSAURUS,
    "When this creature dies, it deals damage equal to its toughness to each opponent.",
    false
)]
#[case::each_player(
    TYRANNOSAURUS,
    "When this creature dies, it deals damage equal to its power to each player.",
    false
)]
#[case::the_other_target(
    PYREMAW,
    "Whenever you cast an instant or sorcery spell, put a +1/+1 counter on this creature. Then this creature deals damage equal to its power to any target.",
    false
)]
fn damage_equal_to_the_power_of_its_source(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(json, clause), reads, "{clause}");
}

const GRAZER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Type":"Land"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}}]}}]}}"#;

#[rstest]
#[case::arboreal_grazer(
    GRAZER,
    "When this creature enters, you may put a land card from your hand onto the battlefield tapped.",
    true
)]
#[case::untapped(
    GRAZER,
    "When this creature enters, you may put a land card from your hand onto the battlefield.",
    false
)]
#[case::a_creature_card(
    GRAZER,
    "When this creature enters, you may put a creature card from your hand onto the battlefield tapped.",
    false
)]
#[case::from_the_graveyard(
    GRAZER,
    "When this creature enters, you may put a land card from your graveyard onto the battlefield tapped.",
    false
)]
#[case::without_the_choice(
    GRAZER,
    "When this creature enters, put a land card from your hand onto the battlefield tapped.",
    false
)]
fn a_card_put_onto_the_battlefield_from_the_hand(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(json, clause), reads, "{clause}");
}

fn returns(selector: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":{selector}}}}}],"effects":[{{"Move":{{"what":{{"Target":0}},"to":"Hand"}}}}]}}}}"#
    )
}

#[rstest]
#[case::a_spirit(
    r#"[{"In":"Graveyard"},{"OwnedBy":"You"},{"Subtype":"Spirit"}]"#,
    "Return target Spirit card from your graveyard to your hand."
)]
#[case::an_outlaw(
    r#"[{"In":"Graveyard"},{"OwnedBy":"You"},{"Subtype":"Outlaw"}]"#,
    "Return target Outlaw card from your graveyard to your hand."
)]
#[case::a_nonland_permanent(
    r#"[{"In":"Graveyard"},{"OwnedBy":"You"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Planeswalker"}]}]"#,
    "Return target nonland permanent card from your graveyard to your hand."
)]
#[case::a_creature(
    r#"[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"}]"#,
    "Return target creature card from your graveyard to your hand."
)]
fn a_card_of_a_subtype_or_a_nonland_permanent_card_in_a_graveyard(
    #[case] selector: &str,
    #[case] clause: &str,
) {
    assert!(written(&returns(selector), clause), "{clause}");
}

#[rstest]
#[case::another_subtype(
    r#"[{"In":"Graveyard"},{"OwnedBy":"You"},{"Subtype":"Spirit"}]"#,
    "Return target Dinosaur card from your graveyard to your hand."
)]
#[case::a_creature_card_for_a_spirit(
    r#"[{"In":"Graveyard"},{"OwnedBy":"You"},{"Subtype":"Spirit"}]"#,
    "Return target creature card from your graveyard to your hand."
)]
#[case::any_nonland_card_is_more_than_a_permanent_card(
    r#"[{"In":"Graveyard"},{"OwnedBy":"You"},{"Not":{"Type":"Land"}}]"#,
    "Return target nonland permanent card from your graveyard to your hand."
)]
#[case::a_nonland_permanent_card_is_less_than_a_nonland_card(
    r#"[{"In":"Graveyard"},{"OwnedBy":"You"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Planeswalker"}]}]"#,
    "Return target nonland card from your graveyard to your hand."
)]
fn another_subtype_or_type_does_not_read(#[case] selector: &str, #[case] clause: &str) {
    assert!(!written(&returns(selector), clause), "{clause}");
}

const MASK: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Pay":{"cost":[{"PayLife":{"Stat":{"of":"Triggering","stat":"Power"}}}],"paid":[{"Draw":{"count":{"Stat":{"of":"Triggering","stat":"Power"}}}}]}}]}}"#;

#[rstest]
#[case::mask_of_griselbrand(
    "Whenever equipped creature dies, you may pay X life, where X is its power. If you do, draw X cards.",
    true
)]
#[case::another_stat(
    "Whenever equipped creature dies, you may pay X life, where X is its toughness. If you do, draw X cards.",
    false
)]
#[case::a_fixed_payment(
    "Whenever equipped creature dies, you may pay 2 life. If you do, draw 2 cards.",
    false
)]
#[case::another_draw(
    "Whenever equipped creature dies, you may pay X life, where X is its power. If you do, draw a card.",
    false
)]
fn life_paid_as_x(#[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(MASK, clause), reads, "{clause}");
}
