//! Goal: the writers of the second sample of the names M to R give the oracle wording of steps joined by "and" or ", then", of a return from the graveyard for a payment, of a spell of the chosen color, of life over the starting life total, of a group "that are Zombies and/or tokens", of an attach to the returned object, of a search by a characteristic of the card, of a step on the discarded card, of the tokens of any player, of a token for each spell cast, of converge and of the defending player's mill, and a wrong count, zone, kind, player or step never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// "Choose one — / Look at the top five cards of your library. Put one of those cards into your hand and the rest into your graveyard. / Each opponent loses 3 life and you gain 3 life. / ~ deals 5 damage to target creature or planeswalker."
const MAESTROS_CHARM: &str = r#"{"Spell":{"effects":[{"Modal":{"modes":[{"effects":[{"LookAtTop":{"count":5,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":"Graveyard"}}]},{"effects":[{"LoseLife":{"who":"Opponents","amount":3}},{"GainLife":{"amount":3}}]},{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":5}}]}]}}]}}"#;

/// "Whenever a nontoken creature you control dies, this creature deals 1 damage to you and you draw a card."
const MIDNIGHT_REAPER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"DealDamage":{"to":"You","amount":1}},{"Draw":{}}]}}"#;

/// "When Roalesk dies, proliferate, then proliferate again."
const ROALESK_DIES: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Proliferate":{}},{"Proliferate":{}}]}}"#;

/// "Choose two — ..."
const PRISMARI_COMMAND: &str = r#"{"Spell":{"effects":[{"Modal":{"choose":{"min":2,"max":2},"modes":[{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":2}}]},{"targets":[{"what":"Player"}],"effects":[{"Draw":{"who":{"Target":0},"count":2}},{"Discard":{"who":{"Target":0},"count":2}}]},{"targets":[{"what":"Player"}],"effects":[{"CreateToken":{"who":{"Target":0},"token":{"Named":"treasure"}}}]},{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}]}}]}}"#;

/// "Tap an untapped legendary creature you control: Add one mana of any color."
const RELIC_OF_LEGENDS: &str = r#"{"Activated":{"cost":[{"TapPermanents":{"filter":[{"Type":"Creature"},{"Supertype":"Legendary"}]}}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#;

/// "{2}, {T}, Sacrifice this land: Draw a card if you control an artifact. Draw a card if you control an enchantment."
const ROADSIDE_RELIQUARY: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap","SacrificeSelf"],"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"then":[{"Draw":{}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Enchantment"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"then":[{"Draw":{}}]}}]}}"#;

/// "Whenever you cast a creature spell, you may pay {G}{G}. If you do, return this card from your graveyard to your hand."
const MASKED_ADMIRERS: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Type":"Creature"}],"by":"You"}},"from":"Graveyard","effects":[{"Pay":{"cost":"{G}{G}","paid":[{"Move":{"what":"This","to":"Hand"}}]}}]}}"#;

/// "Whenever a player casts a spell of the chosen color, you may gain 1 life."
const PARADISE_PLUME: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":["ChosenColor"]}},"effects":[{"May":{"effects":[{"GainLife":{"amount":1}}]}}]}}"#;

/// "As long as you have at least 7 life more than your starting life total, creatures you control get +2/+2."
const RIGHTEOUS_VALKYRIE: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":2,"toughness":2}}],"condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":{"Sum":[{"StartingLife":"You"},7]}}}}}"#;

/// "Creatures you control that are Zombies and/or tokens get +1/+1 and have flying."
const ON_WINGS_OF_GOLD: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"AnyOf":[{"Subtype":"Zombie"},"Token"]}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Flying"}]}}"#;

/// "When this Equipment enters, return target creature card with mana value 3 or less from your graveyard to the battlefield and attach this Equipment to it."
const PRE_WAR_FORMALWEAR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"},{"Compare":{"stat":"ManaValue","op":"Le","to":3}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}},{"Attach":{"what":"This","to":{"Target":0}}}]}}"#;

/// "When this creature dies, search your library for up to X basic land cards, where X is this creature's power, put them onto the battlefield, then shuffle."
const RAMPANT_REJUVENATOR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"count":{"min":0,"max":{"Stat":{"of":"This","stat":"Power"}}},"as":"lands"}},{"Move":{"what":{"Bound":"lands"},"to":{"Battlefield":{}}}},{"Shuffle":{}}]}}"#;

/// "{T}, Discard a card: Draw a card. If the discarded card was a creature card, amass Orcs 1."
const MORIA_SCAVENGER: &str = r#"{"Activated":{"cost":["Tap",{"Discard":{}}],"effects":[{"Draw":{}},{"If":{"condition":{"Matches":{"subject":"Paid","selector":[{"Type":"Creature"}]}},"then":[{"Amass":{"subtype":"Orc","count":1}}]}}]}}"#;

/// "If one or more tokens would be created, twice that many of those tokens are created instead."
const PRIMAL_VIGOR: &str =
    r#"{"Replacement":{"when":{"TokenCreation":{"controller":"EachPlayer"}},"by":{"Multiply":2}}}"#;

/// "At the beginning of your end step, for each spell you've cast this turn, create a 1/2 blue Bird creature token with flying named Storm Crow."
const MURMURATION: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Storm Crow","colors":["Blue"],"types":["Creature"],"subtypes":["Bird"],"power":1,"toughness":2,"abilities":[{"Keyword":"Flying"}]}},"count":{"Tallied":{"who":"You","what":"SpellsCast"}}}}]}}"#;

/// "Converge — ~ deals X damage to each creature, where X is the number of colors of mana spent to cast this spell."
const RADIANT_FLAMES: &str = r#"{"Spell":{"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"}]},"amount":"ColorsSpent"}}]}}"#;

/// "Whenever this creature attacks, defending player mills ten cards."
const NEMESIS_OF_REASON: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Mill":{"who":"Defending","count":10}}]}}"#;

#[rstest]
#[case::maestros_charm(
    MAESTROS_CHARM,
    "Choose one \u{2014}\n\u{2022} Look at the top five cards of your library. Put one of those cards into your hand and the rest into your graveyard.\n\u{2022} Each opponent loses 3 life and you gain 3 life.\n\u{2022} ~ deals 5 damage to target creature or planeswalker."
)]
#[case::damage_and_draw(
    MIDNIGHT_REAPER,
    "Whenever a nontoken creature you control dies, this creature deals 1 damage to you and you draw a card."
)]
#[case::proliferate_again(ROALESK_DIES, "When ~ dies, proliferate, then proliferate again.")]
#[case::draws_then_discards(
    PRISMARI_COMMAND,
    "Choose two \u{2014}\n\u{2022} ~ deals 2 damage to any target.\n\u{2022} Target player draws two cards, then discards two cards.\n\u{2022} Target player creates a Treasure token.\n\u{2022} Destroy target artifact."
)]
#[case::untapped_before_the_type(
    RELIC_OF_LEGENDS,
    "Tap an untapped legendary creature you control: Add one mana of any color."
)]
#[case::two_sentences_with_the_condition_last(
    ROADSIDE_RELIQUARY,
    "{2}, {T}, Sacrifice this land: Draw a card if you control an artifact. Draw a card if you control an enchantment."
)]
fn steps_are_joined_or_ordered_as_the_cards_word_them(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_count(
    MIDNIGHT_REAPER,
    "Whenever a nontoken creature you control dies, this creature deals 2 damage to you and you draw a card."
)]
#[case::another_player(
    MIDNIGHT_REAPER,
    "Whenever a nontoken creature you control dies, this creature deals 1 damage to each opponent and you draw a card."
)]
#[case::proliferate_once(ROALESK_DIES, "When ~ dies, proliferate.")]
#[case::another_kind_of_card(
    RELIC_OF_LEGENDS,
    "Tap an untapped legendary artifact you control: Add one mana of any color."
)]
#[case::another_condition(
    ROADSIDE_RELIQUARY,
    "{2}, {T}, Sacrifice this land: Draw a card if you control an artifact. Draw a card if you control a land."
)]
#[case::the_gain_before_the_loss(
    MAESTROS_CHARM,
    "Choose one \u{2014}\n\u{2022} Look at the top five cards of your library. Put one of those cards into your hand and the rest into your graveyard.\n\u{2022} You gain 3 life and each opponent loses 3 life.\n\u{2022} ~ deals 5 damage to target creature or planeswalker."
)]
fn other_steps_do_not_read_as_the_joined_ones(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

#[rstest]
#[case::pays_to_return(
    MASKED_ADMIRERS,
    "Whenever you cast a creature spell, you may pay {G}{G}. If you do, return this card from your graveyard to your hand."
)]
#[case::spell_of_the_chosen_color(
    PARADISE_PLUME,
    "Whenever a player casts a spell of the chosen color, you may gain 1 life."
)]
#[case::over_the_starting_life(
    RIGHTEOUS_VALKYRIE,
    "As long as you have at least 7 life more than your starting life total, creatures you control get +2/+2."
)]
#[case::zombies_and_or_tokens(
    ON_WINGS_OF_GOLD,
    "Creatures you control that are Zombies and/or tokens get +1/+1 and have flying."
)]
#[case::attached_to_the_returned_card(
    PRE_WAR_FORMALWEAR,
    "When this Equipment enters, return target creature card with mana value 3 or less from your graveyard to the battlefield and attach this Equipment to it."
)]
#[case::search_by_the_power(
    RAMPANT_REJUVENATOR,
    "When this creature dies, search your library for up to X basic land cards, where X is this creature's power, put them onto the battlefield, then shuffle."
)]
#[case::discarded_creature_card(
    MORIA_SCAVENGER,
    "{T}, Discard a card: Draw a card. If the discarded card was a creature card, amass Orcs 1."
)]
#[case::tokens_of_any_player(
    PRIMAL_VIGOR,
    "If one or more tokens would be created, twice that many of those tokens are created instead."
)]
#[case::a_token_for_each_spell(
    MURMURATION,
    "At the beginning of your end step, for each spell you've cast this turn, create a 1/2 blue Bird creature token with flying named Storm Crow."
)]
#[case::converge(
    RADIANT_FLAMES,
    "~ deals X damage to each creature, where X is the number of colors of mana spent to cast this spell."
)]
#[case::defending_player_mills(
    NEMESIS_OF_REASON,
    "Whenever this creature attacks, defending player mills ten cards."
)]
fn the_wording_of_each_sample_card_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::to_the_hand_of_another_zone(
    MASKED_ADMIRERS,
    "Whenever you cast a creature spell, you may pay {G}{G}. If you do, return this card from exile to your hand."
)]
#[case::another_color_rule(
    PARADISE_PLUME,
    "Whenever a player casts a spell of the chosen type, you may gain 1 life."
)]
#[case::another_margin(
    RIGHTEOUS_VALKYRIE,
    "As long as you have at least 8 life more than your starting life total, creatures you control get +2/+2."
)]
#[case::another_group(
    ON_WINGS_OF_GOLD,
    "Creatures you control that are Elves and/or tokens get +1/+1 and have flying."
)]
#[case::attached_to_another_object(
    PRE_WAR_FORMALWEAR,
    "When this Equipment enters, return target creature card with mana value 3 or less from your graveyard to the battlefield and attach this Equipment to target creature."
)]
#[case::another_characteristic(
    RAMPANT_REJUVENATOR,
    "When this creature dies, search your library for up to X basic land cards, where X is this creature's toughness, put them onto the battlefield, then shuffle."
)]
#[case::another_card_type(
    MORIA_SCAVENGER,
    "{T}, Discard a card: Draw a card. If the discarded card was a land card, amass Orcs 1."
)]
#[case::tokens_under_your_control_only(
    PRIMAL_VIGOR,
    "If one or more tokens would be created under your control, twice that many of those tokens are created instead."
)]
#[case::for_each_land(
    MURMURATION,
    "At the beginning of your end step, for each land you control, create a 1/2 blue Bird creature token with flying named Storm Crow."
)]
#[case::another_stat_for_x(
    RADIANT_FLAMES,
    "~ deals X damage to each creature, where X is the number of creatures you control."
)]
#[case::another_count_milled(
    NEMESIS_OF_REASON,
    "Whenever this creature attacks, defending player mills nine cards."
)]
#[case::another_player_mills(
    NEMESIS_OF_REASON,
    "Whenever this creature attacks, each opponent mills ten cards and you mill ten cards."
)]
fn another_part_does_not_read_as_the_sample_cards(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}
