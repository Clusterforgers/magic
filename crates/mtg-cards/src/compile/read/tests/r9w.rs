//! Goal: the round 9 writers give the oracle wording of rules no template reads (the players a fact of the turn holds for, a spell cast out of its caster's turn, what a rule forbids, a manifest or cloak named "it", manifest dread, a shield of the next damage from a chosen source or redirected, and a countered spell's kept mana value), and a rule of another meaning (other players, other fact, other count, other zone, mana abilities kept or not) never reads back as the clause; "becomes an Island" reads as the land types it sets.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const END_STEP: &str = r#"{"StepBegins":{"step":"End","whose":"You"}}"#;

/// "the players `who` names for whom `test` holds" as a count.
fn players(who: &str, test: &str) -> String {
    format!(r#"{{"Players":{{"PlayersWhere":{{"who":"{who}","test":{test}}}}}}}"#)
}

fn tallied(what: &str, right: u32) -> String {
    format!(
        r#"{{"Compare":{{"left":{{"Tallied":{{"what":"{what}","who":"You"}}}},"op":"Ge","right":{right}}}}}"#
    )
}

fn at_end_step(effect: &str) -> String {
    format!(r#"{{"Triggered":{{"when":{END_STEP},"effects":[{effect}]}}}}"#)
}

fn counters_for(count: &str) -> String {
    at_end_step(&format!(
        r#"{{"PutCounters":{{"count":{count},"counter":"+1/+1","on":"This"}}}}"#
    ))
}

#[rstest]
#[case::dealt_damage(
    &counters_for(&players("Opponents", &tallied("DamageTaken", 1))),
    "At the beginning of your end step, put a +1/+1 counter on this creature for each opponent who was dealt damage this turn."
)]
#[case::lost_life(
    &counters_for(&players("EachPlayer", &tallied("LifeLost", 1))),
    "At the beginning of your end step, put a +1/+1 counter on this creature for each player who lost life this turn."
)]
#[case::graveyards(
    &counters_for(&players("EachPlayer", r#"{"Compare":{"left":{"Count":[{"OwnedBy":"You"},{"In":"Graveyard"}]},"op":"Ge","right":7}}"#)),
    "At the beginning of your end step, put a +1/+1 counter on this creature for each graveyard with seven or more cards in it."
)]
#[case::poison_x(
    &at_end_step(&format!(r#"{{"Draw":{{"count":{}}}}}"#, players("Opponents", r#"{"Compare":{"left":{"Counters":{"counter":"poison","on":"You"}},"op":"Ge","right":3}}"#))),
    "At the beginning of your end step, draw X cards, where X is the number of opponents who have three or more poison counters."
)]
#[case::hands_x(
    &at_end_step(&format!(r#"{{"Draw":{{"count":{}}}}}"#, players("Opponents", r#"{"Compare":{"left":{"Count":[{"OwnedBy":"You"},{"In":"Hand"}]},"op":"Ge","right":4}}"#))),
    "At the beginning of your end step, draw X cards, where X is the number of your opponents with four or more cards in hand."
)]
#[case::activate_only_if(
    &format!(r#"{{"Activated":{{"cost":"{{R}}{{R}}{{R}}","from":"Graveyard","condition":{{"Compare":{{"left":{},"op":"Ge","right":1}}}},"effects":[{{"Move":{{"to":"Hand","what":"This"}}}}]}}}}"#, players("Opponents", &tallied("DamageTaken", 1))),
    "{R}{R}{R}: Return this card from your graveyard to your hand. Activate only if an opponent was dealt damage this turn."
)]
#[case::sacrifice_unless(
    &format!(r#"{{"Triggered":{{"when":{{"ZoneChange":{{"to":"Battlefield","what":["This"]}}}},"effects":[{{"If":{{"condition":{{"Compare":{{"left":{},"op":"Eq","right":0}}}},"then":[{{"Sacrifice":{{"filter":["This"]}}}}]}}}}]}}}}"#, players("Opponents", &tallied("DamageTaken", 1))),
    "When this creature enters, sacrifice it unless an opponent was dealt damage this turn."
)]
#[case::off_turn_cast(
    r#"{"Triggered":{"when":{"SpellCast":{"by":{"PlayersWhere":{"test":{"Not":"YourTurn"},"who":"EachPlayer"}}}},"effects":[{"DealDamage":{"amount":4,"to":"Triggering"}}]}}"#,
    "Whenever a player casts a spell, if it's not their turn, this creature deals 4 damage to them."
)]
fn r9w_players_write_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        written(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::opponents_for_players(
    &counters_for(&players("Opponents", &tallied("LifeLost", 1))),
    "At the beginning of your end step, put a +1/+1 counter on this creature for each player who lost life this turn."
)]
#[case::other_fact(
    &counters_for(&players("Opponents", &tallied("LifeLost", 1))),
    "At the beginning of your end step, put a +1/+1 counter on this creature for each opponent who was dealt damage this turn."
)]
#[case::more_life(
    &counters_for(&players("Opponents", &tallied("LifeLost", 2))),
    "At the beginning of your end step, put a +1/+1 counter on this creature for each opponent who lost life this turn."
)]
#[case::fewer_cards(
    &counters_for(&players("EachPlayer", r#"{"Compare":{"left":{"Count":[{"OwnedBy":"You"},{"In":"Graveyard"}]},"op":"Le","right":7}}"#)),
    "At the beginning of your end step, put a +1/+1 counter on this creature for each graveyard with seven or more cards in it."
)]
#[case::hand_for_graveyard(
    &counters_for(&players("EachPlayer", r#"{"Compare":{"left":{"Count":[{"OwnedBy":"You"},{"In":"Hand"}]},"op":"Ge","right":7}}"#)),
    "At the beginning of your end step, put a +1/+1 counter on this creature for each graveyard with seven or more cards in it."
)]
#[case::two_opponents(
    &format!(r#"{{"Activated":{{"cost":"{{R}}{{R}}{{R}}","from":"Graveyard","condition":{{"Compare":{{"left":{},"op":"Ge","right":2}}}},"effects":[{{"Move":{{"to":"Hand","what":"This"}}}}]}}}}"#, players("Opponents", &tallied("DamageTaken", 1))),
    "{R}{R}{R}: Return this card from your graveyard to your hand. Activate only if an opponent was dealt damage this turn."
)]
#[case::on_their_turn(
    r#"{"Triggered":{"when":{"SpellCast":{"by":{"PlayersWhere":{"test":"YourTurn","who":"EachPlayer"}}}},"effects":[{"DealDamage":{"amount":4,"to":"Triggering"}}]}}"#,
    "Whenever a player casts a spell, if it's not their turn, this creature deals 4 damage to them."
)]
fn r9w_players_of_another_meaning_differ(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

fn rule(rule: &str) -> String {
    format!(r#"{{"Rule":{{"rule":{{"Cant":{rule}}}}}}}"#)
}

#[rstest]
#[case::attack_unless(
    r#"{"who":"This","action":{"Attack":{"unless":{"Compare":{"left":{"Count":[{"Subtype":"Island"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}}}"#,
    "This creature can't attack unless defending player controls an Island."
)]
#[case::attack_unless_monarch(
    r#"{"who":"This","action":{"Attack":{"unless":{"Monarch":"You"}}}}"#,
    "This creature can't attack unless defending player is the monarch."
)]
#[case::skip_draw(
    r#"{"who":"You","action":{"SkipStep":"Draw"}}"#,
    "Skip your draw step."
)]
#[case::skip_untap(
    r#"{"who":"EachPlayer","action":{"SkipStep":"Untap"}}"#,
    "Players skip their untap steps."
)]
#[case::one_spell(
    r#"{"who":"EachPlayer","action":{"Cast":{"more_than":1}}}"#,
    "Each player can't cast more than one spell each turn."
)]
#[case::additional(
    r#"{"who":"EachPlayer","action":{"Cast":{"filter":[{"Not":{"Type":"Artifact"}}],"more_than":1}}}"#,
    "Each player who has cast a nonartifact spell this turn can't cast additional nonartifact spells."
)]
#[case::blue_creature_spells(
    r#"{"who":"Opponents","action":{"Cast":{"filter":[{"Type":"Creature"},{"Color":"Blue"}]}}}"#,
    "Your opponents can't cast blue creature spells."
)]
#[case::chosen_name(
    r#"{"who":"EachPlayer","action":{"Cast":{"filter":["ChosenName"]}}}"#,
    "Spells with the chosen name can't be cast."
)]
#[case::own_turns(
    r#"{"who":{"PlayersWhere":{"test":{"Not":"YourTurn"},"who":"EachPlayer"}},"action":{"Cast":{}}}"#,
    "Players can cast spells only during their own turns."
)]
#[case::lands(
    r#"{"who":"EachPlayer","action":"PlayLand"}"#,
    "Players can't play lands."
)]
#[case::target_of_color(
    r#"{"who":"This","action":{"BeTargeted":[{"Color":"Black"}]}}"#,
    "This creature can't be the target of black spells or abilities from black sources."
)]
#[case::aura_spells(
    r#"{"who":"This","action":{"BeTargeted":[{"Subtype":"Aura"},"Spell"]}}"#,
    "This creature can't be the target of Aura spells."
)]
#[case::poison(
    r#"{"who":"You","action":{"PutCounterOf":"poison"}}"#,
    "You can't get poison counters."
)]
#[case::minus_counters(
    r#"{"who":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"action":{"PutCounterOf":"-1/-1"}}"#,
    "Creatures you control can't have -1/-1 counters put on them."
)]
#[case::every_ability(
    r#"{"who":"EachPlayer","action":{"ActivateKind":{"kind":"Every","sources":[{"Type":"Artifact"}]}}}"#,
    "Activated abilities of artifacts can't be activated."
)]
#[case::but_mana(
    r#"{"who":{"Each":["ChosenName"]},"action":{"Activate":[]}}"#,
    "Activated abilities of sources with the chosen name can't be activated unless they're mana abilities."
)]
#[case::enchanted(
    r#"{"who":"EachPlayer","action":{"ActivateKind":{"kind":"Every","sources":[{"Type":"Creature"},{"Is":"Attached"}]}}}"#,
    "Enchanted creature's activated abilities can't be activated."
)]
#[case::entering(
    r#"{"who":"EachPlayer","action":{"Trigger":{"ZoneChange":{"to":"Battlefield","what":[{"Type":"Creature"}]}}}}"#,
    "Creatures entering don't cause abilities to trigger."
)]
fn r9w_prohibitions_write_back(#[case] cant: &str, #[case] clause: &str) {
    let json = rule(cant);
    assert!(
        written(&json, clause),
        "{clause}: {:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::attack_if(
    r#"{"who":"This","action":{"Attack":{"unless":{"Not":{"Compare":{"left":{"Count":[{"Subtype":"Island"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}}}}"#,
    "This creature can't attack unless defending player controls an Island."
)]
#[case::opponents_island(
    r#"{"who":"This","action":{"Attack":{"unless":{"Compare":{"left":{"Count":[{"Subtype":"Island"},{"ControlledBy":"Opponents"}]},"op":"Ge","right":1}}}}}"#,
    "This creature can't attack unless defending player controls an Island."
)]
#[case::two_spells(
    r#"{"who":"EachPlayer","action":{"Cast":{"more_than":2}}}"#,
    "Each player can't cast more than one spell each turn."
)]
#[case::opponents_only(
    r#"{"who":"Opponents","action":{"Cast":{"filter":["ChosenName"]}}}"#,
    "Spells with the chosen name can't be cast."
)]
#[case::mana_kept(
    r#"{"who":{"Each":[{"Type":"Artifact"}]},"action":{"Activate":[]}}"#,
    "Activated abilities of artifacts can't be activated."
)]
#[case::mana_stopped(
    r#"{"who":"EachPlayer","action":{"ActivateKind":{"kind":"Every","sources":[{"Type":"Artifact"}]}}}"#,
    "Activated abilities of artifacts can't be activated unless they're mana abilities."
)]
#[case::dying(
    r#"{"who":"EachPlayer","action":{"Trigger":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Type":"Creature"}]}}}}"#,
    "Creatures entering don't cause abilities to trigger."
)]
#[case::their_draw(
    r#"{"who":"Opponents","action":{"SkipStep":"Draw"}}"#,
    "Skip your draw step."
)]
fn r9w_prohibitions_of_another_meaning_differ(#[case] cant: &str, #[case] clause: &str) {
    assert!(!written(&rule(cant), clause), "{clause}");
}

#[rstest]
#[case::counters(
    r#"{"Spell":{"effects":[{"Manifest":{"as":"manifest"}},{"PutCounters":{"count":2,"counter":"+1/+1","on":{"Bound":"manifest"}}}]}}"#,
    "Manifest the top card of your library, then put two +1/+1 counters on it."
)]
#[case::cloak_attach(
    r#"{"Triggered":{"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}},"effects":[{"Cloak":{"as":"cloak"}},{"Attach":{"to":{"Bound":"cloak"},"what":"This"}}]}}"#,
    "When this Equipment enters, cloak the top card of your library, then attach this Equipment to it."
)]
#[case::becomes_aura(
    r#"{"Triggered":{"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}},"effects":[{"Manifest":{"as":"manifest"}},{"Continuous":{"affected":"This","duration":"Forever","mods":[{"AddSubtype":"Aura"},{"AddKeyword":{"Enchant":[{"Type":"Creature"}]}}]}},{"Attach":{"to":{"Bound":"manifest"},"what":"This"}}]}}"#,
    "When this enchantment enters, it becomes an Aura with enchant creature. Manifest the top card of your library and attach this enchantment to it."
)]
#[case::dread(
    r#"{"Triggered":{"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}},"effects":[{"LookAtTop":{"as":"looked","count":2}},{"Choose":{"as":"kept","from":{"Bound":"looked"},"rest_as":"rest"}},{"Move":{"to":"Graveyard","what":{"Bound":"rest"}}},{"Manifest":{"as":"manifested"}},{"Attach":{"to":{"Bound":"manifested"},"what":"This"}}]}}"#,
    "When this Equipment enters, manifest dread, then attach this Equipment to that creature."
)]
#[case::their_dread(
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"LookAtTop":{"as":"looked","count":2,"who":{"ControllerOf":{"Target":0}}}},{"Choose":{"as":"kept","from":{"Bound":"looked"},"rest_as":"rest","who":{"ControllerOf":{"Target":0}}}},{"Move":{"to":"Graveyard","what":{"Bound":"rest"}}},{"Manifest":{"who":{"ControllerOf":{"Target":0}}}}]}}"#,
    "Destroy target creature. Its controller manifests dread."
)]
#[case::chosen_source(
    r#"{"Activated":{"cost":"{1}","effects":[{"Choose":{"as":"source","from":{"Each":[{"AnyOf":[{"In":"Battlefield"},{"In":"Stack"},{"In":"Command"}]},{"Color":"Black"}]}}},{"Shield":{"by":"PreventNext","duration":"UntilEndOfTurn","when":{"Damage":{"source":[{"Is":{"Bound":"source"}}],"to":"You"}}}}]}}"#,
    "{1}: The next time a black source of your choice would deal damage to you this turn, prevent that damage."
)]
#[case::redirected(
    r#"{"Activated":{"cost":"{0}","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Shield":{"by":{"RedirectUpTo":{"most":1,"to":{"Target":0}}},"duration":"UntilEndOfTurn","when":{"Damage":{"to":"This"}}}}]}}"#,
    "{0}: The next 1 damage that would be dealt to this creature this turn is dealt to target creature you control instead."
)]
#[case::countered_value(
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}},{"RememberValue":{"slot":"kept","value":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}},{"CreateToken":{"count":{"Remembered":{"on":"This","slot":"kept"}},"token":{"Named":"treasure"}}}]}}"#,
    "Counter target spell. Create X Treasure tokens, where X is that spell's mana value."
)]
fn r9w_named_steps_write_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        written(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::two_cards(
    r#"{"Spell":{"effects":[{"Manifest":{"as":"manifest","count":2}},{"PutCounters":{"count":2,"counter":"+1/+1","on":{"Bound":"manifest"}}}]}}"#,
    "Manifest the top card of your library, then put two +1/+1 counters on it."
)]
#[case::counters_elsewhere(
    r#"{"Spell":{"effects":[{"Manifest":{"as":"manifest"}},{"PutCounters":{"count":2,"counter":"+1/+1","on":"This"}}]}}"#,
    "Manifest the top card of your library, then put two +1/+1 counters on it."
)]
#[case::three_looked(
    r#"{"Spell":{"effects":[{"LookAtTop":{"as":"looked","count":3}},{"Choose":{"as":"kept","from":{"Bound":"looked"},"rest_as":"rest"}},{"Move":{"to":"Graveyard","what":{"Bound":"rest"}}},{"Manifest":{}}]}}"#,
    "Manifest dread."
)]
#[case::rest_to_hand(
    r#"{"Spell":{"effects":[{"LookAtTop":{"as":"looked","count":2}},{"Choose":{"as":"kept","from":{"Bound":"looked"},"rest_as":"rest"}},{"Move":{"to":"Hand","what":{"Bound":"rest"}}},{"Manifest":{}}]}}"#,
    "Manifest dread."
)]
#[case::any_zone_source(
    r#"{"Activated":{"cost":"{1}","effects":[{"Choose":{"as":"source","from":{"Each":[{"Color":"Black"}]}}},{"Shield":{"by":"PreventNext","duration":"UntilEndOfTurn","when":{"Damage":{"source":[{"Is":{"Bound":"source"}}],"to":"You"}}}}]}}"#,
    "{1}: The next time a black source of your choice would deal damage to you this turn, prevent that damage."
)]
#[case::all_damage(
    r#"{"Activated":{"cost":"{1}","effects":[{"Choose":{"as":"source","from":{"Each":[{"AnyOf":[{"In":"Battlefield"},{"In":"Stack"},{"In":"Command"}]},{"Color":"Black"}]}}},{"Shield":{"by":"Prevent","duration":"UntilEndOfTurn","when":{"Damage":{"source":[{"Is":{"Bound":"source"}}],"to":"You"}}}}]}}"#,
    "{1}: The next time a black source of your choice would deal damage to you this turn, prevent that damage."
)]
#[case::two_redirected(
    r#"{"Activated":{"cost":"{0}","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Shield":{"by":{"RedirectUpTo":{"most":2,"to":{"Target":0}}},"duration":"UntilEndOfTurn","when":{"Damage":{"to":"This"}}}}]}}"#,
    "{0}: The next 1 damage that would be dealt to this creature this turn is dealt to target creature you control instead."
)]
#[case::power_kept(
    r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}},{"RememberValue":{"slot":"kept","value":{"Stat":{"of":{"Target":0},"stat":"Power"}}}},{"CreateToken":{"count":{"Remembered":{"on":"This","slot":"kept"}},"token":{"Named":"treasure"}}}]}}"#,
    "Counter target spell. Create X Treasure tokens, where X is that spell's mana value."
)]
fn r9w_named_steps_of_another_meaning_differ(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

#[rstest]
#[case::island(
    "Target land becomes an Island until end of turn.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetLandTypes":["Island"]}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn r9w_a_basic_land_type_replaces_the_land_types(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}
