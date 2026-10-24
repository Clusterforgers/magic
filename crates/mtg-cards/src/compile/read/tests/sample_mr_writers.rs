//! Goal: the writers of the proposal checker give the oracle wording of a sacrifice without "you", of two steps a payment buys joined by "and", of what follows an optional destruction, of conditions placed after their steps, of the colors of a spell, of "up to X" targets and "twice X" tokens, of targets of different kinds, of a group compared with a chosen target, of cards revealed for an opponent to pick from, of all graveyards, of a counter or not on the creature of an event, of two targets in one sentence, of the announced X and the kind of the one target as conditions, of counters put on each of several targets, of damage prevented for you and of a card put on top of its owner's library, and a wrong object, count, condition or step never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// "Draw two cards, then sacrifice a permanent."
const PERILOUS_RESEARCH: &str =
    r#"{"Spell":{"effects":[{"Draw":{"count":2}},{"Sacrifice":{"filter":[]}}]}}"#;

/// "Sacrifice a land. Search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle."
const ROILING_REGROWTH: &str = r#"{"Spell":{"effects":[{"Sacrifice":{"filter":[{"Type":"Land"}]}},{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"count":{"min":0,"max":2},"as":"lands"}},{"Move":{"what":{"Bound":"lands"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#;

#[rstest]
#[case::then_after_another_step(PERILOUS_RESEARCH, "Draw two cards, then sacrifice a permanent.")]
#[case::as_a_sentence_of_its_own(PERILOUS_RESEARCH, "Draw two cards. Sacrifice a permanent.")]
#[case::opening_the_spell(
    ROILING_REGROWTH,
    "Sacrifice a land. Search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle."
)]
fn a_sacrifice_of_the_controller_leaves_out_you(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_kind(PERILOUS_RESEARCH, "Draw two cards, then sacrifice a creature.")]
#[case::another_count(PERILOUS_RESEARCH, "Draw three cards, then sacrifice a permanent.")]
#[case::another_verb(PERILOUS_RESEARCH, "Draw two cards, then discard a permanent.")]
#[case::another_player(
    PERILOUS_RESEARCH,
    "Draw two cards, then each opponent sacrifices a permanent."
)]
#[case::another_land_kind(
    ROILING_REGROWTH,
    "Sacrifice a creature. Search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::a_land_for_the_opponent(
    ROILING_REGROWTH,
    "Each opponent sacrifices a land. Search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle."
)]
fn another_sacrifice_does_not_read_as_the_controllers(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "Whenever you draw a card, you may pay {1}. If you do, put a +1/+1 counter on this creature and create a 1/1 blue Tentacle creature token."
const NADIR_KRAKEN: &str = r#"{"Triggered":{"when":{"Drew":{"who":"You"}},"effects":[{"Pay":{"cost":"{1}","paid":[{"PutCounters":{"on":"This","counter":"+1/+1"}},{"CreateToken":{"token":{"Custom":{"name":"Tentacle","colors":["Blue"],"types":["Creature"],"subtypes":["Tentacle"],"power":1,"toughness":1}}}}]}}]}}"#;

/// "When this creature enters, you may destroy another target creature. If a creature is destroyed this way, you gain life equal to its toughness."
const NOXIOUS_GEARHULK: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},"Other"]}}],"effects":[{"May":{"effects":[{"Destroy":{"what":{"Target":0}}}]}},{"If":{"condition":"Performed","then":[{"GainLife":{"amount":{"Stat":{"of":{"Target":0},"stat":"Toughness"}}}}]}}]}}"#;

#[rstest]
#[case::the_steps_of_a_payment_joined_by_and(
    NADIR_KRAKEN,
    "Whenever you draw a card, you may pay {1}. If you do, put a +1/+1 counter on ~ and create a 1/1 blue Tentacle creature token."
)]
#[case::the_creature_destroyed_this_way(
    NOXIOUS_GEARHULK,
    "When ~ enters, you may destroy another target creature. If a creature is destroyed this way, you gain life equal to its toughness."
)]
fn the_steps_after_a_payment_or_a_destruction_are_worded_as_the_cards_do(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_cost(
    NADIR_KRAKEN,
    "Whenever you draw a card, you may pay {2}. If you do, put a +1/+1 counter on ~ and create a 1/1 blue Tentacle creature token."
)]
#[case::another_token(
    NADIR_KRAKEN,
    "Whenever you draw a card, you may pay {1}. If you do, put a +1/+1 counter on ~ and create a 2/2 blue Tentacle creature token."
)]
#[case::no_payment(
    NADIR_KRAKEN,
    "Whenever you draw a card, put a +1/+1 counter on ~ and create a 1/1 blue Tentacle creature token."
)]
#[case::another_object_destroyed(
    NOXIOUS_GEARHULK,
    "When ~ enters, you may destroy another target creature. If a permanent is destroyed this way, you gain life equal to its toughness."
)]
#[case::another_characteristic(
    NOXIOUS_GEARHULK,
    "When ~ enters, you may destroy another target creature. If a creature is destroyed this way, you gain life equal to its power."
)]
fn another_cost_token_object_or_characteristic_does_not_read(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}

/// "At the beginning of your upkeep, put a +1/+1 counter on each creature you control if you control a creature with power 5 or greater. Then you gain 10 life if ... 10 or greater. Then you win the game if ... 20 or greater."
const MAYAELS_ARIA: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Compare":{"stat":"Power","op":"Ge","to":5}}]},"op":"Ge","right":1}},"then":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Compare":{"stat":"Power","op":"Ge","to":10}}]},"op":"Ge","right":1}},"then":[{"GainLife":{"amount":10}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Compare":{"stat":"Power","op":"Ge","to":20}}]},"op":"Ge","right":1}},"then":[{"WinGame":{}}]}}]}}"#;

#[rstest]
#[case::every_condition_after_its_step(
    MAYAELS_ARIA,
    "At the beginning of your upkeep, put a +1/+1 counter on each creature you control if you control a creature with power 5 or greater. Then you gain 10 life if you control a creature with power 10 or greater. Then you win the game if you control a creature with power 20 or greater."
)]
#[case::the_first_condition_first(
    MAYAELS_ARIA,
    "At the beginning of your upkeep, if you control a creature with power 5 or greater, put a +1/+1 counter on each creature you control. Then you gain 10 life if you control a creature with power 10 or greater. Then you win the game if you control a creature with power 20 or greater."
)]
fn a_condition_after_its_step_reads(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::conditions_swapped(
    MAYAELS_ARIA,
    "At the beginning of your upkeep, put a +1/+1 counter on each creature you control if you control a creature with power 10 or greater. Then you gain 10 life if you control a creature with power 5 or greater. Then you win the game if you control a creature with power 20 or greater."
)]
#[case::a_step_without_its_condition(
    MAYAELS_ARIA,
    "At the beginning of your upkeep, put a +1/+1 counter on each creature you control if you control a creature with power 5 or greater. Then you gain 10 life. Then you win the game if you control a creature with power 20 or greater."
)]
#[case::another_prize(
    MAYAELS_ARIA,
    "At the beginning of your upkeep, put a +1/+1 counter on each creature you control if you control a creature with power 5 or greater. Then you gain 10 life if you control a creature with power 10 or greater. Then you lose the game if you control a creature with power 20 or greater."
)]
fn a_changed_condition_or_step_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "Whenever you cast a multicolored spell, ~ deals X damage to any target, where X is the number of colors that spell is."
const MANA_CANNONS: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Compare":{"stat":"Colors","op":"Ge","to":2}}],"by":"You"}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":"Triggering","stat":"Colors"}}}}]}}"#;

/// "When ~ enters, return up to X target Zombie cards from your graveyard to the battlefield, where X is the number of opponents you have."
const ROT_HULK: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Subtype":"Zombie"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":{"Players":"Opponents"}}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}"#;

/// "Destroy up to X target artifacts and/or enchantments. Create twice X 1/1 black and green Pest creature tokens with ..."
const PEST_INFESTATION: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]},"bounds":{"min":0,"max":"X"}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"CreateToken":{"token":{"Custom":{"name":"Pest","colors":["Black","Green"],"types":["Creature"],"subtypes":["Pest"],"power":1,"toughness":1}},"count":{"Times":{"by":2,"of":"X"}}}}]}}"#;

/// "Destroy target artifact, enchantment, or creature with power 4 or greater."
const MAKE_YOUR_MOVE: &str = r#"{"Spell":{"targets":[{"what":{"Either":[{"Object":[{"Type":"Artifact"}]},{"Object":[{"Type":"Enchantment"}]},{"Object":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":4}}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;

#[rstest]
#[case::the_colors_of_the_spell(
    MANA_CANNONS,
    "Whenever you cast a multicolored spell, ~ deals X damage to any target, where X is the number of colors that spell is."
)]
#[case::up_to_x_targets_with_the_count_named(
    ROT_HULK,
    "When ~ enters, return up to X target Zombie cards from your graveyard to the battlefield, where X is the number of opponents you have."
)]
#[case::up_to_x_targets_and_twice_x_tokens(
    PEST_INFESTATION,
    "Destroy up to X target artifacts and/or enchantments. Create twice X 1/1 black and green Pest creature tokens."
)]
#[case::targets_of_different_kinds(
    MAKE_YOUR_MOVE,
    "Destroy target artifact, enchantment, or creature with power 4 or greater."
)]
fn amounts_and_targets_of_the_kinds_below_read(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::the_colors_of_a_creature(
    MANA_CANNONS,
    "Whenever you cast a multicolored spell, ~ deals X damage to any target, where X is the number of colors that creature is."
)]
#[case::the_mana_value(
    MANA_CANNONS,
    "Whenever you cast a multicolored spell, ~ deals X damage to any target, where X is that spell's mana value."
)]
#[case::up_to_two_targets(
    ROT_HULK,
    "When ~ enters, return up to two target Zombie cards from your graveyard to the battlefield, where X is the number of opponents you have."
)]
#[case::the_count_of_players(
    ROT_HULK,
    "When ~ enters, return up to X target Zombie cards from your graveyard to the battlefield, where X is the number of players you have."
)]
#[case::three_times_x(
    PEST_INFESTATION,
    "Destroy up to X target artifacts and/or enchantments. Create three times X 1/1 black and green Pest creature tokens."
)]
#[case::exactly_x_tokens(
    PEST_INFESTATION,
    "Destroy up to X target artifacts and/or enchantments. Create X 1/1 black and green Pest creature tokens."
)]
#[case::another_power(
    MAKE_YOUR_MOVE,
    "Destroy target artifact, enchantment, or creature with power 5 or greater."
)]
#[case::a_missing_kind(
    MAKE_YOUR_MOVE,
    "Destroy target artifact or creature with power 4 or greater."
)]
fn a_wrong_count_object_or_kind_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "Choose target creature you control. Destroy all creatures with power less than that creature's power."
const MANDATE_OF_ABADDON: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Lt","to":{"Stat":{"of":{"Target":0},"stat":"Power"}}}}]}}}]}}"#;

/// "Reveal the top three cards of your library. An opponent chooses one of them. Put that card into your graveyard and the rest into your hand."
const MURMURS_FROM_BEYOND: &str = r#"{"Spell":{"effects":[{"LookAtTop":{"count":3,"as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"Choose":{"who":"Opponents","from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}}]},"as":"chosen","rest_as":"rest"}},{"Move":{"what":{"Bound":"chosen"},"to":"Graveyard"}},{"Move":{"what":{"Bound":"rest"},"to":"Hand"}}]}}"#;

/// "Put all creature cards from all graveyards onto the battlefield under your control."
const RISE_OF_THE_DARK_REALMS: &str = r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"Type":"Creature"},{"In":"Graveyard"}]},"to":{"Battlefield":{"under":"You"}}}}]}}"#;

/// "Whenever a creature you control deals combat damage to a player, draw a card if that creature has a +1/+1 counter on it. If it doesn't, put a +1/+1 counter on it."
const MARCUS: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}},"effects":[{"If":{"condition":{"Matches":{"subject":"Triggering","selector":[{"HasCounter":"+1/+1"}]}},"then":[{"Draw":{}}],"otherwise":[{"PutCounters":{"on":"Triggering","counter":"+1/+1"}}]}}]}}"#;

/// "When ~ enters, destroy up to one target creature and target player mills three cards."
const NECRON_DEATHMARK: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}},{"what":"Player"}],"effects":[{"Destroy":{"what":{"Target":0}}},{"Mill":{"who":{"Target":1},"count":3}}]}}"#;

#[rstest]
#[case::a_group_compared_with_the_chosen_target(
    MANDATE_OF_ABADDON,
    "Choose target creature you control. Destroy all creatures with power less than that creature's power."
)]
#[case::the_card_an_opponent_picks(
    MURMURS_FROM_BEYOND,
    "Reveal the top three cards of your library. An opponent chooses one of them. Put that card into your graveyard and the rest into your hand."
)]
#[case::every_graveyard(
    RISE_OF_THE_DARK_REALMS,
    "Put all creature cards from all graveyards onto the battlefield under your control."
)]
#[case::a_counter_or_not(
    MARCUS,
    "Whenever a creature you control deals combat damage to a player, draw a card if that creature has a +1/+1 counter on it. If it doesn't, put a +1/+1 counter on it."
)]
#[case::two_targets_in_one_sentence(
    NECRON_DEATHMARK,
    "When ~ enters, destroy up to one target creature and target player mills three cards."
)]
fn steps_that_name_a_chosen_object_or_a_pick_read(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::greater_power(
    MANDATE_OF_ABADDON,
    "Choose target creature you control. Destroy all creatures with power greater than that creature's power."
)]
#[case::another_characteristic(
    MANDATE_OF_ABADDON,
    "Choose target creature you control. Destroy all creatures with toughness less than that creature's toughness."
)]
#[case::the_target_not_chosen_first(
    MANDATE_OF_ABADDON,
    "Destroy all creatures with power less than that creature's power."
)]
#[case::the_opponent_not_you(
    MURMURS_FROM_BEYOND,
    "Reveal the top three cards of your library. You choose one of them. Put that card into your graveyard and the rest into your hand."
)]
#[case::another_count_of_cards(
    MURMURS_FROM_BEYOND,
    "Reveal the top four cards of your library. An opponent chooses one of them. Put that card into your graveyard and the rest into your hand."
)]
#[case::the_places_swapped(
    MURMURS_FROM_BEYOND,
    "Reveal the top three cards of your library. An opponent chooses one of them. Put that card into your hand and the rest into your graveyard."
)]
#[case::only_your_graveyard(
    RISE_OF_THE_DARK_REALMS,
    "Put all creature cards from your graveyard onto the battlefield under your control."
)]
#[case::under_its_owners_control(
    RISE_OF_THE_DARK_REALMS,
    "Put all creature cards from all graveyards onto the battlefield."
)]
#[case::the_counter_on_another_kind(
    MARCUS,
    "Whenever a creature you control deals combat damage to a player, draw a card if that creature has a -1/-1 counter on it. If it doesn't, put a +1/+1 counter on it."
)]
#[case::the_steps_swapped(
    MARCUS,
    "Whenever a creature you control deals combat damage to a player, put a +1/+1 counter on it if that creature has a +1/+1 counter on it. If it doesn't, draw a card."
)]
#[case::three_mills(
    NECRON_DEATHMARK,
    "When ~ enters, destroy up to one target creature and target player mills four cards."
)]
#[case::an_opponent_not_a_player(
    NECRON_DEATHMARK,
    "When ~ enters, destroy up to one target creature and target opponent mills three cards."
)]
fn a_changed_object_count_place_or_step_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "Create X 1/1 white Soldier creature tokens. If X is 5 or more, destroy all other creatures."
const MARTIAL_COUP: &str = r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"count":"X"}},{"If":{"condition":{"Compare":{"left":"X","op":"Ge","right":5}},"then":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},"Other"]}}}]}}]}}"#;

/// "Return target creature card from your graveyard to the battlefield. If it's an Elf, create two 1/1 green Elf Warrior creature tokens."
const RETURN_UPON_THE_TIDE: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}},{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Subtype":"Elf"}]}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Elf Warrior","colors":["Green"],"types":["Creature"],"subtypes":["Elf","Warrior"],"power":1,"toughness":1}},"count":2}}]}}]}}"#;

/// "You gain 1 life for each creature your opponents control. Prevent all damage that would be dealt to you this turn."
const RIOT_CONTROL: &str = r#"{"Spell":{"effects":[{"GainLife":{"amount":{"Count":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}},{"Shield":{"when":{"Damage":{"to":"You"}},"by":"Prevent","duration":"UntilEndOfTurn"}}]}}"#;

/// "Put target card from a graveyard on top of its owner's library."
const NOXIOUS_REVIVAL: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Graveyard"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Top"}}}}]}}"#;

/// "At the beginning of each upkeep, if you control no Snakes, create a 1/1 black Snake creature token with deathtouch."
const OPHIOMANCER: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep"}},"if":{"Compare":{"left":{"Count":[{"Subtype":"Snake"},{"ControlledBy":"You"}]},"op":"Eq","right":0}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Snake","colors":["Black"],"types":["Creature"],"subtypes":["Snake"],"power":1,"toughness":1,"abilities":[{"Keyword":"Deathtouch"}]}}}}]}}"#;

/// "Put a shield counter on each of up to three target creatures."
const PROTECTION_MAGIC: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":3}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":{"Other":"shield"}}}]}}"#;

#[rstest]
#[case::counters_on_each_of_several_targets(
    PROTECTION_MAGIC,
    "Put a shield counter on each of up to three target creatures."
)]
fn counters_on_several_targets_are_put_on_each_of_them(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_number(
    PROTECTION_MAGIC,
    "Put a shield counter on each of up to two target creatures."
)]
#[case::another_counter(
    PROTECTION_MAGIC,
    "Put a stun counter on each of up to three target creatures."
)]
#[case::another_kind_of_target(
    PROTECTION_MAGIC,
    "Put a shield counter on each of up to three target artifacts."
)]
#[case::exactly_three(
    PROTECTION_MAGIC,
    "Put a shield counter on each of three target creatures."
)]
fn another_number_counter_or_target_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

#[rstest]
#[case::the_announced_x_against_a_number(
    MARTIAL_COUP,
    "Create X 1/1 white Soldier creature tokens. If X is 5 or more, destroy all other creatures."
)]
#[case::the_kind_of_the_one_target(
    RETURN_UPON_THE_TIDE,
    "Return target creature card from your graveyard to the battlefield. If it's an Elf, create two 1/1 green Elf Warrior creature tokens."
)]
#[case::damage_to_you_prevented(
    RIOT_CONTROL,
    "You gain 1 life for each creature your opponents control. Prevent all damage that would be dealt to you this turn."
)]
#[case::a_card_of_any_graveyard_on_top_of_its_owners_library(
    NOXIOUS_REVIVAL,
    "Put target card from a graveyard on top of its owner's library."
)]
#[case::no_subtype_controlled(
    OPHIOMANCER,
    "At the beginning of each upkeep, if you control no Snakes, create a 1/1 black Snake creature token with deathtouch."
)]
fn conditions_and_destinations_of_the_kinds_below_read(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_number(
    MARTIAL_COUP,
    "Create X 1/1 white Soldier creature tokens. If X is 6 or more, destroy all other creatures."
)]
#[case::the_wrong_direction(
    MARTIAL_COUP,
    "Create X 1/1 white Soldier creature tokens. If X is 5 or less, destroy all other creatures."
)]
#[case::another_kind_of_target(
    RETURN_UPON_THE_TIDE,
    "Return target creature card from your graveyard to the battlefield. If it's a Goblin, create two 1/1 green Elf Warrior creature tokens."
)]
#[case::another_count_of_tokens(
    RETURN_UPON_THE_TIDE,
    "Return target creature card from your graveyard to the battlefield. If it's an Elf, create three 1/1 green Elf Warrior creature tokens."
)]
#[case::combat_damage_only(
    RIOT_CONTROL,
    "You gain 1 life for each creature your opponents control. Prevent all combat damage that would be dealt to you this turn."
)]
#[case::damage_to_each_player(
    RIOT_CONTROL,
    "You gain 1 life for each creature your opponents control. Prevent all damage that would be dealt this turn."
)]
#[case::the_bottom_of_the_library(
    NOXIOUS_REVIVAL,
    "Put target card from a graveyard on the bottom of its owner's library."
)]
#[case::your_graveyard_only(
    NOXIOUS_REVIVAL,
    "Put target card from your graveyard on top of its owner's library."
)]
#[case::another_kind_of_subtype(
    OPHIOMANCER,
    "At the beginning of each upkeep, if you control no Goblins, create a 1/1 black Snake creature token with deathtouch."
)]
#[case::at_least_one_subtype(
    OPHIOMANCER,
    "At the beginning of each upkeep, if you control a Snake, create a 1/1 black Snake creature token with deathtouch."
)]
fn another_condition_or_destination_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}
