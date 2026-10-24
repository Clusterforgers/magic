//! Goal: the writers of the second sample of the names A to F give the oracle wording of a defending player who exiles permanents, of players who sacrifice several creatures, of a choice of one creature and the destruction of the rest, of a commander taken from the command zone, of a permission to flash, of the card and the graveyard shuffled in, of the top card revealed and sent to one of two places, of a group a target controls, of one effect between two targets, and of the variants of a bounce, a subtype card, a controller, a loss of life, a group compared with a target, a prevention, a pump of all others, every basic land type, an investigation repeated and a condition of none; a wrong count, kind, group, player, duration or order never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// A choice of permanents the defending player controls and their exile.
fn defender(count: &str, group: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Attacks":{{"what":["This"]}}}},"effects":[{{"Choose":{{"who":"Defending","from":{{"Each":[{group}{{"ControlledBy":"Defending"}}]}},"count":{{"min":{count},"max":{count}}},"as":"chosen"}}}},{{"Exile":{{"what":{{"Bound":"chosen"}}}}}}]}}}}"#
    )
}

/// A sacrifice by several players.
fn sacrifices(who: &str, kind: &str, count: u32) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Sacrifice":{{"who":"{who}","filter":[{{"Type":"{kind}"}}],"count":{count}}}}}]}}}}"#
    )
}

/// A choice of creatures and the destruction of the rest.
fn rest(count: &str) -> String {
    format!(
        r#"{{"Spell":{{"effects":[{{"Choose":{{"from":{{"Each":[{{"Type":"Creature"}}]}},"count":{count},"as":"kept","rest_as":"rest"}}}},{{"Destroy":{{"what":{{"Bound":"rest"}}}}}}]}}}}"#
    )
}

const COMMANDER: &str = r#"{"Spell":{"effects":[{"Move":{"what":{"Each":["Commander",{"OwnedBy":"You"},{"In":"Command"}]},"to":"Hand"}}]}}"#;

/// A permission to cast with flash.
fn flash(duration: &str, from: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{G}}{{U}}"}},"Tap"],"effects":[{{"Rule":{{"rule":{{"MayCast":{{{from}"flash":true}}}},"duration":"{duration}"}}}}]}}}}"#
    )
}

/// The card and a graveyard shuffled into a library.
fn shuffled(graveyard: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{2}}"}},"Tap"],"effects":[{{"Move":{{"what":"This","to":{{"Library":{{"place":"Shuffled"}}}}}}}},{{"Move":{{"what":{{"Each":[{{"In":"Graveyard"}},{{"OwnedBy":"{graveyard}"}}]}},"to":{{"Library":{{"place":"Shuffled"}}}}}}}}]}}}}"#
    )
}

/// The top card revealed and sent to one place if it is a kind of card and to another otherwise.
fn revealed(kind: &str, otherwise: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"LookAtTop":{{"count":1,"as":"top"}}}},{{"Reveal":{{"what":{{"Bound":"top"}}}}}},{{"If":{{"condition":{{"Matches":{{"subject":{{"Bound":"top"}},"selector":[{{"Type":"{kind}"}}]}}}},"then":[{{"Move":{{"what":{{"Bound":"top"}},"to":{{"Battlefield":{{}}}}}}}}],"otherwise":[{{"Move":{{"what":{{"Bound":"top"}},"to":{otherwise}}}}}]}}}}]}}}}"#
    )
}

/// An effect on the creatures one target player controls.
fn tap_group(effect: &str, who: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{8}}"}}],"targets":[{{"what":"{who}"}}],"effects":[{{"{effect}":{{"what":{{"Each":[{{"Type":"Creature"}},{{"ControlledBy":{{"Target":0}}}}]}}}}}}]}}}}"#
    )
}

/// A draw for the tapped creatures a target player controls.
fn arrows(who: &str, state: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":"{who}"}}],"effects":[{{"Draw":{{"count":{{"Count":[{{"Type":"Creature"}},{state}{{"ControlledBy":{{"Target":0}}}}]}}}}}}]}}}}"#
    )
}

const ATTACH: &str = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Subtype":"Equipment"},{"ControlledBy":"You"}]}},{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Attach":{"what":{"Target":0},"to":{"Target":1}}}]}}"#;

/// A creature that deals damage equal to its power to another creature.
fn hammer(second: &str, stat: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}},{{"what":{{"Object":[{{"Type":"Creature"}}{second}]}}}}],"effects":[{{"DealDamage":{{"source":{{"Target":0}},"to":{{"Target":1}},"amount":{{"Stat":{{"of":{{"Target":0}},"stat":"{stat}"}}}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::exiles_two_permanents(&defender("2", ""), "Whenever ~ attacks, defending player exiles two permanents they control.")]
#[case::exiles_three_creatures(&defender("3", r#"{"Type":"Creature"},"#), "Whenever ~ attacks, defending player exiles three creatures they control.")]
#[case::each_opponent_sacrifices_two(&sacrifices("EachPlayer", "Creature", 2), "Each player sacrifices two creatures of their choice.")]
#[case::each_opponent_three_lands(&sacrifices("Opponents", "Land", 3), "Each opponent sacrifices three lands of their choice.")]
#[case::choose_up_to_one(&rest(r#"{"min":0,"max":1}"#), "Choose up to one creature. Destroy the rest.")]
#[case::choose_one(&rest(r#"{"min":1,"max":1}"#), "Choose a creature. Destroy the rest.")]
#[case::commander_back(COMMANDER, "Put your commander into your hand from the command zone.")]
#[case::flash(&flash("UntilEndOfTurn", ""), "{G}{U}, {T}: You may cast spells this turn as though they had flash.")]
#[case::shuffled_in(&shuffled("You"), "{2}, {T}: Shuffle ~ and your graveyard into their owner's library.")]
#[case::revealed_to_hand(&revealed("Land", r#""Hand""#), "When ~ enters, reveal the top card of your library. If it's a land card, put it onto the battlefield. Otherwise, put that card into your hand.")]
#[case::tapped_group(&tap_group("Tap", "Player"), "{8}: Tap all creatures target player controls.")]
#[case::untapped_group(&tap_group("Untap", "Player"), "{8}: Untap all creatures target player controls.")]
#[case::opponent_group(&tap_group("Tap", "Opponent"), "{8}: Tap all creatures target opponent controls.")]
#[case::tapped_draw(&arrows("Opponent", r#""Tapped","#), "Draw a card for each tapped creature target opponent controls.")]
#[case::attach(
    ATTACH,
    "{T}: Attach target Equipment you control to target creature you control."
)]
#[case::damage_by_power(&hammer(r#",{"Not":{"Is":{"Target":0}}}"#, "Power"), "Target creature you control deals damage equal to its power to another target creature.")]
fn the_steps_of_the_second_sample_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_count(&defender("2", ""), "Whenever ~ attacks, defending player exiles three permanents they control.")]
#[case::another_group(&defender("2", ""), "Whenever ~ attacks, defending player exiles two creatures they control.")]
#[case::another_player(&defender("2", ""), "Whenever ~ attacks, each opponent exiles two permanents they control.")]
#[case::another_group_in_the_data(&defender("2", r#"{"Type":"Creature"},"#), "Whenever ~ attacks, defending player exiles two permanents they control.")]
#[case::another_sacrifice_count(&sacrifices("EachPlayer", "Creature", 2), "Each player sacrifices three creatures of their choice.")]
#[case::another_sacrificer(&sacrifices("EachPlayer", "Creature", 2), "Each opponent sacrifices two creatures of their choice.")]
#[case::another_sacrificed_kind(&sacrifices("EachPlayer", "Creature", 2), "Each player sacrifices two lands of their choice.")]
#[case::one_sacrifice_for_two(&sacrifices("EachPlayer", "Creature", 1), "Each player sacrifices two creatures of their choice.")]
#[case::another_chosen_count(&rest(r#"{"min":0,"max":1}"#), "Choose up to two creatures. Destroy the rest.")]
#[case::another_verb(&rest(r#"{"min":0,"max":1}"#), "Choose up to one creature. Exile the rest.")]
#[case::exactly_one_for_up_to_one(&rest(r#"{"min":0,"max":1}"#), "Choose a creature. Destroy the rest.")]
#[case::another_zone_of_the_commander(
    COMMANDER,
    "Put your commander into your graveyard from the command zone."
)]
#[case::an_opponents_commander(
    COMMANDER,
    "Put each opponent's commander into your hand from the command zone."
)]
#[case::another_spell_kind(&flash("UntilEndOfTurn", ""), "{G}{U}, {T}: You may cast creature spells this turn as though they had flash.")]
#[case::another_duration(&flash("UntilEndOfYourNextTurn", ""), "{G}{U}, {T}: You may cast spells this turn as though they had flash.")]
#[case::another_zone(&flash("UntilEndOfTurn", r#""from":"Graveyard","#), "{G}{U}, {T}: You may cast spells this turn as though they had flash.")]
#[case::an_opponents_graveyard(&shuffled("Opponents"), "{2}, {T}: Shuffle ~ and your graveyard into their owner's library.")]
#[case::the_card_alone(&shuffled("You"), "{2}, {T}: Shuffle ~ into its owner's library.")]
#[case::the_hand_instead(&shuffled("You"), "{2}, {T}: Shuffle ~ and your hand into their owner's library.")]
#[case::another_kind_revealed(&revealed("Land", r#""Hand""#), "When ~ enters, reveal the top card of your library. If it's a creature card, put it onto the battlefield. Otherwise, put that card into your hand.")]
#[case::another_place(&revealed("Land", r#""Graveyard""#), "When ~ enters, reveal the top card of your library. If it's a land card, put it onto the battlefield. Otherwise, put that card into your hand.")]
#[case::the_other_order(&revealed("Land", r#""Hand""#), "When ~ enters, reveal the top card of your library. If it's a land card, put that card into your hand. Otherwise, put it onto the battlefield.")]
#[case::another_verb_on_the_group(&tap_group("Tap", "Player"), "{8}: Untap all creatures target player controls.")]
#[case::another_player_of_the_group(&tap_group("Tap", "Player"), "{8}: Tap all creatures target opponent controls.")]
#[case::your_own_group(&tap_group("Tap", "Player"), "{8}: Tap all creatures you control.")]
#[case::an_opponent_for_a_player(&arrows("Player", r#""Tapped","#), "Draw a card for each tapped creature target opponent controls.")]
#[case::untapped_for_tapped(&arrows("Opponent", r#""Tapped","#), "Draw a card for each untapped creature target opponent controls.")]
#[case::any_state_for_tapped(&arrows("Opponent", r#""Tapped","#), "Draw a card for each creature target opponent controls.")]
#[case::the_order_of_the_objects(
    ATTACH,
    "{T}: Attach target creature you control to target Equipment you control."
)]
#[case::another_attached_kind(
    ATTACH,
    "{T}: Attach target Aura you control to target creature you control."
)]
#[case::a_second_target_that_is_not_another(&hammer("", "Power"), "Target creature you control deals damage equal to its power to another target creature.")]
#[case::another_stat(&hammer(r#",{"Not":{"Is":{"Target":0}}}"#, "Toughness"), "Target creature you control deals damage equal to its power to another target creature.")]
fn the_steps_of_the_second_sample_do_not_read_as_another_count_kind_player_order_or_duration(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}

const AETHERIZE: &str = r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"Type":"Creature"},"Attacking"]},"to":"Hand"}}]}}"#;
const RECRUITMENT: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}},{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Subtype":"Zombie"}]}},"then":[{"Draw":{}}]}}]}}"#;
const FECUNDITY: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"May":{"who":{"ControllerOf":"Triggering"},"effects":[{"Draw":{"who":{"ControllerOf":"Triggering"}}}]}}]}}"#;
const SEEKER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"LoseLife":{"who":{"ControllerOf":"Triggering"},"amount":1}}]}}]}}"#;
const MIGHTY: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Gt","to":{"Stat":{"of":{"Target":0},"stat":"Power"}}}}]}}}]}}"#;
const EMMARA: &str = r#"{"Replacement":{"when":{"Damage":{"to":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]}}},"by":"Prevent"}}"#;
const CREEP: &str = r#"{"Activated":{"cost":[{"Mana":"{1}{B}"},{"RemoveCounters":{"counter":"+1/+1","count":1}}],"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},"Other"]},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]}}"#;
const DRYAD: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Land"},{"ControlledBy":"You"}]},"mods":[{"AddSubtype":"Plains"},{"AddSubtype":"Island"},{"AddSubtype":"Swamp"},{"AddSubtype":"Mountain"},{"AddSubtype":"Forest"}]}}"#;
const CONFIRM: &str = r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}},{"CreateToken":{"token":{"Named":"clue"},"count":3}}]}}"#;
const BUTTERBUR: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"Subtype":"Food"},{"ControlledBy":"You"}]},"op":"Eq","right":0}},"effects":[{"CreateToken":{"token":{"Named":"food"}}}]}}"#;
const MUTATION: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"Color":"Black"}}]}}],"effects":[{"Destroy":{"what":{"Target":0},"regenerate":false}},{"CreateToken":{"token":{"Custom":{"name":"Saproling","colors":["Green"],"types":["Creature"],"subtypes":["Saproling"],"power":1,"toughness":1}},"count":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}"#;

#[rstest]
#[case::owner_hand(AETHERIZE, "Return all attacking creatures to their owner's hand.")]
#[case::owners_hands(AETHERIZE, "Return all attacking creatures to their owners' hands.")]
#[case::subtype_card(
    RECRUITMENT,
    "Return target creature card from your graveyard to your hand. If it's a Zombie card, draw a card."
)]
#[case::subtype(
    RECRUITMENT,
    "Return target creature card from your graveyard to your hand. If it's a Zombie, draw a card."
)]
#[case::that_creatures_controller(
    FECUNDITY,
    "Whenever a creature dies, that creature's controller may draw a card."
)]
#[case::its_controller(FECUNDITY, "Whenever a creature dies, its controller may draw a card.")]
#[case::have_that_player_lose(
    SEEKER,
    "Whenever a creature an opponent controls enters, you may have that player lose 1 life."
)]
#[case::target_named_where_compared(
    MIGHTY,
    "Destroy all creatures with power greater than target creature's power."
)]
#[case::prevent_all(
    EMMARA,
    "Prevent all damage that would be dealt to creature tokens you control."
)]
#[case::all_others(
    CREEP,
    "{1}{B}, Remove a +1/+1 counter from ~: All other creatures get -1/-1 until end of turn."
)]
#[case::every_basic_land_type(
    DRYAD,
    "Lands you control are every basic land type in addition to their other types."
)]
#[case::investigate_times(CONFIRM, "Counter target spell. Investigate three times.")]
#[case::dont_control(
    BUTTERBUR,
    "At the beginning of your end step, if you don't control a Food, create a Food token."
)]
#[case::that_creatures_mana_value(
    MUTATION,
    "Destroy target nonblack creature. It can't be regenerated. Create X 1/1 green Saproling creature tokens, where X is that creature's mana value."
)]
fn the_variants_of_the_second_sample_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::one_hand_for_other_cards(AETHERIZE, "Return all creatures to their owner's hand.")]
#[case::your_hand(AETHERIZE, "Return all attacking creatures to your hand.")]
#[case::another_draw_after_the_subtype(
    RECRUITMENT,
    "Return target creature card from your graveyard to your hand. If it's a Zombie card, draw two cards."
)]
#[case::another_subtype(
    RECRUITMENT,
    "Return target creature card from your graveyard to your hand. If it's a Vampire card, draw a card."
)]
#[case::a_permanent_instead_of_the_creature(
    FECUNDITY,
    "Whenever a creature dies, that permanent's controller may draw a card."
)]
#[case::another_kind_instead_of_the_creature(
    FECUNDITY,
    "Whenever a creature dies, that artifact's controller may draw a card."
)]
#[case::you_instead_of_the_controller(FECUNDITY, "Whenever a creature dies, you may draw a card.")]
#[case::another_loss(
    SEEKER,
    "Whenever a creature an opponent controls enters, you may have that player lose 2 life."
)]
#[case::each_opponent_for_that_player(
    SEEKER,
    "Whenever a creature an opponent controls enters, you may have each opponent lose 1 life."
)]
#[case::less_for_greater(
    MIGHTY,
    "Destroy all creatures with power less than target creature's power."
)]
#[case::another_stat_compared(
    MIGHTY,
    "Destroy all creatures with toughness greater than target creature's toughness."
)]
#[case::creatures_for_tokens(
    EMMARA,
    "Prevent all damage that would be dealt to creatures you control."
)]
#[case::opponents_tokens(
    EMMARA,
    "Prevent all damage that would be dealt to creature tokens your opponents control."
)]
#[case::combat_damage_only(
    EMMARA,
    "Prevent all combat damage that would be dealt to creature tokens you control."
)]
#[case::another_pump(
    CREEP,
    "{1}{B}, Remove a +1/+1 counter from ~: All other creatures get -2/-2 until end of turn."
)]
#[case::your_creatures_only(
    CREEP,
    "{1}{B}, Remove a +1/+1 counter from ~: All other creatures you control get -1/-1 until end of turn."
)]
#[case::another_duration_of_the_pump(
    CREEP,
    "{1}{B}, Remove a +1/+1 counter from ~: All other creatures get -1/-1 until your next turn."
)]
#[case::creatures_for_lands(
    DRYAD,
    "Creatures you control are every basic land type in addition to their other types."
)]
#[case::your_opponents_lands(
    DRYAD,
    "Lands your opponents control are every basic land type in addition to their other types."
)]
#[case::four_times(CONFIRM, "Counter target spell. Investigate four times.")]
#[case::twice_for_three(CONFIRM, "Counter target spell. Investigate twice.")]
#[case::another_token_kind_investigated(
    BUTTERBUR,
    "At the beginning of your end step, if you don't control a Treasure, create a Food token."
)]
#[case::an_opponent_for_you(
    BUTTERBUR,
    "At the beginning of your end step, if an opponent doesn't control a Food, create a Food token."
)]
#[case::that_artifact_for_that_creature(
    MUTATION,
    "Destroy target nonblack creature. It can't be regenerated. Create X 1/1 green Saproling creature tokens, where X is that artifact's mana value."
)]
fn the_variants_of_the_second_sample_do_not_read_as_another_kind_count_group_or_duration(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}
