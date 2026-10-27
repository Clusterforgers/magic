//! Goal: the writers the fourth sample of the names A to F needed give the oracle wording of a count of tokens said with
//! "for each", a group named by "They" after its counters, a pump by "where X is the number of", a destruction by a
//! count of counters, a card in its graveyard, a discard of one kind of card, the commander creatures a player owns and
//! the phrases of a controller, a draw by two players, a divided damage, a play of the exiled card and the source of
//! damage; a changed count, kind, player, group, zone or duration never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const BRASS: &str = r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"treasure"},"count":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}]}}"#;
const EUMIDIAN: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Insect","colors":["Black"],"types":["Creature"],"subtypes":["Insect"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}},"count":{"Counters":{"on":"This","counter":{"Other":"hatchling"}}}}}]}}"#;
const ARDBERT: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Color":"White"}],"by":"You"}},"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"Supertype":"Legendary"},{"ControlledBy":"You"}]},"counter":"+1/+1"}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"Supertype":"Legendary"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Vigilance"}],"duration":"UntilEndOfTurn"}}]}}"#;
const ARAGORN: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":["CantBlock"],"duration":"UntilEndOfTurn"}},{"If":{"condition":{"Monarch":"You"},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":["CantBlock"],"duration":"UntilEndOfTurn"}}]}}]}}"#;
const BOUNTY: &str = r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"targets":[{"what":{"Object":[{"Supertype":"Legendary"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;
const SOFIA: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Supertype":"Legendary"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"CreateToken":{"who":{"ControllerOf":{"Target":0}},"token":{"Named":"junk"}}}]}}"#;
const DIVINER: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Draw":{"count":"Triggering"}},{"Draw":{"who":"Triggering","count":"Triggering"}}]}}"#;
const FURY: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]},"bounds":{"min":1,"max":null},"divide":4}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#;
const PALADIN: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"}]}],"by":"You"}},"effects":[{"ExileTop":{"as":"impulse"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"impulse"},"play":true}},"duration":"UntilEndOfYourNextTurn"}}]}}"#;
const AWAKEN: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"ControlledBy":"Opponents"}],"to":"You"}},"effects":[{"Sacrifice":{"filter":["This"]}},{"If":{"condition":"Performed","then":[{"CreateToken":{"token":{"Custom":{"name":"Dragon","colors":["Red"],"types":["Creature"],"subtypes":["Dragon"],"power":5,"toughness":5,"abilities":[{"Keyword":"Flying"}]}}}}]}}]}}"#;
const DALEK: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"LoseLife":{"who":{"ControllerOf":{"Target":0}},"amount":3}}]}}"#;
const AVALANCHE: &str = r#"{"Spell":{"effects":[{"Draw":{"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Compare":{"stat":"Power","op":"Ge","to":4}}]}}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"toughness":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}],"duration":"UntilEndOfTurn"}}]}}"#;
const BLAST: &str = r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap","SacrificeSelf"],"effects":[{"Destroy":{"what":{"Each":[{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Eq","to":{"Counters":{"on":"This","counter":"charge"}}}}]}}}]}}"#;
const CARMEN: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]},{"Compare":{"stat":"ManaValue","op":"Le","to":{"Stat":{"of":"This","stat":"Power"}}}}]},"bounds":{"min":0,"max":1}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}"#;
const MISER_CREATURE: &str = r#"{"Triggered":{"when":{"Discarded":{"what":[{"Type":"Creature"}],"by":"You"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Zombie","colors":["Black"],"types":["Creature"],"subtypes":["Zombie"],"power":2,"toughness":2}}}}]}}"#;
const MISER_LAND: &str = r#"{"Triggered":{"when":{"Discarded":{"what":[{"Type":"Land"}],"by":"You"}},"effects":[{"AddMana":{"mana":{"Fixed":["Black","Black"]}}}]}}"#;
const BRAWN: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Trample"}],"condition":{"Compare":{"left":{"Count":[{"Subtype":"Forest"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"from":"Graveyard"}}"#;
const DEPARTED: &str = r#"{"Replacement":{"when":{"Enters":{"what":[{"Type":"Creature"},{"Subtype":"Human"},{"ControlledBy":"You"}]}},"by":{"WithCounters":{"counter":"+1/+1","count":1}},"from":"Graveyard"}}"#;
const CRAFTER: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Commander",{"OwnedBy":"You"}]},"mods":[{"GrantAbility":{"Activated":{"cost":[{"Mana":"{2}"},{"Sacrifice":{"filter":[{"Type":"Artifact"}]}}],"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}},{"Draw":{}}]}}}]}}"#;
const COUNTERS_THEN_KEYWORD: &str = r#"{"Spell":{"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Vigilance"}],"duration":"UntilEndOfTurn"}}]}}"#;
const COUNTERS_THEN_OTHERS: &str = r#"{"Spell":{"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":[{"AddKeyword":"Vigilance"}],"duration":"UntilEndOfTurn"}}]}}"#;

#[rstest]
#[case::for_each_land(BRASS, "For each land you control, create a Treasure token.")]
#[case::for_each_counter(
    EUMIDIAN,
    "When ~ is put into a graveyard from the battlefield, for each hatchling counter on it, create a 1/1 black Insect creature token with flying."
)]
#[case::they_gain(
    ARDBERT,
    "Whenever you cast a white spell, put a +1/+1 counter on each legendary creature you control. They gain vigilance until end of turn."
)]
#[case::those_creatures_gain(
    COUNTERS_THEN_KEYWORD,
    "Put a +1/+1 counter on each creature you control. Those creatures gain vigilance until end of turn."
)]
#[case::they_gain_for_the_group(
    COUNTERS_THEN_KEYWORD,
    "Put a +1/+1 counter on each creature you control. They gain vigilance until end of turn."
)]
#[case::creatures_cannot_block(
    ARAGORN,
    "Whenever ~ attacks, up to one target creature can't block this turn. If you're the monarch, creatures can't block this turn."
)]
#[case::legendary_permanent_that_is(
    BOUNTY,
    "{T}, Sacrifice this creature: Destroy target legendary permanent that's an artifact, creature, or enchantment."
)]
#[case::that_permanents_controller(
    SOFIA,
    "When ~ enters, destroy up to one target legendary permanent. That permanent's controller creates a Junk token."
)]
#[case::you_and_that_player_each_draw(
    DIVINER,
    "Whenever this creature deals combat damage to a player, you and that player each draw that many cards."
)]
#[case::it_deals_divided(
    FURY,
    "When this creature enters, it deals 4 damage divided as you choose among any number of target creatures and/or planeswalkers."
)]
#[case::play_that_card_until(
    PALADIN,
    "Whenever you cast an Aura or Equipment spell, exile the top card of your library. You may play that card until the end of your next turn."
)]
#[case::a_source_an_opponent_controls(
    AWAKEN,
    "When a source an opponent controls deals damage to you, sacrifice this enchantment. If you do, create a 5/5 red Dragon creature token with flying."
)]
#[case::that_player_loses(
    DALEK,
    "When this creature enters, destroy target creature an opponent controls. That player loses 3 life."
)]
#[case::where_x_is_the_cards_in_hand(
    AVALANCHE,
    "Draw a card for each creature you control with power 4 or greater. Then creatures you control get +X/+X until end of turn, where X is the number of cards in your hand."
)]
#[case::destroy_each_by_counters(
    BLAST,
    "{3}, {T}, Sacrifice this land: Destroy each nonland permanent with mana value equal to the number of charge counters on this land."
)]
#[case::permanent_card_up_to_power(
    CARMEN,
    "Whenever ~ attacks, return up to one target permanent card with mana value less than or equal to ~'s power from your graveyard to the battlefield."
)]
#[case::discard_a_creature_card(
    MISER_CREATURE,
    "Whenever you discard a creature card, create a 2/2 black Zombie creature token."
)]
#[case::discard_a_land_card(MISER_LAND, "Whenever you discard a land card, add {B}{B}.")]
#[case::in_the_graveyard_and_a_forest(
    BRAWN,
    "As long as this card is in your graveyard and you control a Forest, creatures you control have trample."
)]
#[case::in_the_graveyard(
    DEPARTED,
    "As long as this creature is in your graveyard, each Human creature you control enters with an additional +1/+1 counter on it."
)]
#[case::commander_creatures_you_own(
    CRAFTER,
    "Commander creatures you own have \"{2}, Sacrifice an artifact: Put a +1/+1 counter on this creature and draw a card.\""
)]
fn the_wordings_of_the_cards_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::for_each_creature(BRASS, "For each creature you control, create a Treasure token.")]
#[case::two_tokens_for_each(BRASS, "For each land you control, create two Treasure tokens.")]
#[case::their_lands(
    BRASS,
    "For each land your opponents control, create a Treasure token."
)]
#[case::another_counter(
    EUMIDIAN,
    "When ~ is put into a graveyard from the battlefield, for each charge counter on it, create a 1/1 black Insect creature token with flying."
)]
#[case::no_flying(
    EUMIDIAN,
    "When ~ is put into a graveyard from the battlefield, for each hatchling counter on it, create a 1/1 black Insect creature token."
)]
#[case::another_keyword(
    ARDBERT,
    "Whenever you cast a white spell, put a +1/+1 counter on each legendary creature you control. They gain menace until end of turn."
)]
#[case::another_color(
    ARDBERT,
    "Whenever you cast a blue spell, put a +1/+1 counter on each legendary creature you control. They gain vigilance until end of turn."
)]
#[case::until_your_next_turn(
    ARDBERT,
    "Whenever you cast a white spell, put a +1/+1 counter on each legendary creature you control. They gain vigilance until your next turn."
)]
#[case::another_group_gains(
    COUNTERS_THEN_OTHERS,
    "Put a +1/+1 counter on each creature you control. They gain vigilance until end of turn."
)]
#[case::two_targets_cannot_block(
    ARAGORN,
    "Whenever ~ attacks, up to two target creatures can't block this turn. If you're the monarch, creatures can't block this turn."
)]
#[case::your_creatures_cannot_block(
    ARAGORN,
    "Whenever ~ attacks, up to one target creature can't block this turn. If you're the monarch, creatures you control can't block this turn."
)]
#[case::only_two_types(
    BOUNTY,
    "{T}, Sacrifice this creature: Destroy target legendary permanent that's an artifact or creature."
)]
#[case::not_legendary(
    BOUNTY,
    "{T}, Sacrifice this creature: Destroy target permanent that's an artifact, creature, or enchantment."
)]
#[case::another_token(
    SOFIA,
    "When ~ enters, destroy up to one target legendary permanent. That permanent's controller creates a Treasure token."
)]
#[case::its_owner(
    SOFIA,
    "When ~ enters, destroy up to one target legendary permanent. Its owner creates a Junk token."
)]
#[case::one_card_each(
    DIVINER,
    "Whenever this creature deals combat damage to a player, you and that player each draw a card."
)]
#[case::each_opponent_draws(
    DIVINER,
    "Whenever this creature deals combat damage to a player, you and each opponent each draw that many cards."
)]
#[case::five_damage(
    FURY,
    "When this creature enters, it deals 5 damage divided as you choose among any number of target creatures and/or planeswalkers."
)]
#[case::only_creatures(
    FURY,
    "When this creature enters, it deals 4 damage divided as you choose among any number of target creatures."
)]
#[case::until_this_turn(
    PALADIN,
    "Whenever you cast an Aura or Equipment spell, exile the top card of your library. You may play that card until end of turn."
)]
#[case::cast_not_play(
    PALADIN,
    "Whenever you cast an Aura or Equipment spell, exile the top card of your library. You may cast that card until the end of your next turn."
)]
#[case::a_smaller_dragon(
    AWAKEN,
    "When a source an opponent controls deals damage to you, sacrifice this enchantment. If you do, create a 4/4 red Dragon creature token with flying."
)]
#[case::your_own_source(
    AWAKEN,
    "When a source you control deals damage to you, sacrifice this enchantment. If you do, create a 5/5 red Dragon creature token with flying."
)]
#[case::less_life(
    DALEK,
    "When this creature enters, destroy target creature an opponent controls. That player loses 2 life."
)]
#[case::gains_life(
    DALEK,
    "When this creature enters, destroy target creature an opponent controls. That player gains 3 life."
)]
#[case::cards_in_the_graveyard(
    AVALANCHE,
    "Draw a card for each creature you control with power 4 or greater. Then creatures you control get +X/+X until end of turn, where X is the number of cards in your graveyard."
)]
#[case::creatures_counted(
    AVALANCHE,
    "Draw a card for each creature you control with power 4 or greater. Then creatures you control get +X/+X until end of turn, where X is the number of creatures you control."
)]
#[case::ice_counters(
    BLAST,
    "{3}, {T}, Sacrifice this land: Destroy each nonland permanent with mana value equal to the number of ice counters on this land."
)]
#[case::every_permanent(
    BLAST,
    "{3}, {T}, Sacrifice this land: Destroy each permanent with mana value equal to the number of charge counters on this land."
)]
#[case::toughness_limit(
    CARMEN,
    "Whenever ~ attacks, return up to one target permanent card with mana value less than or equal to ~'s toughness from your graveyard to the battlefield."
)]
#[case::creature_cards_only(
    CARMEN,
    "Whenever ~ attacks, return up to one target creature card with mana value less than or equal to ~'s power from your graveyard to the battlefield."
)]
#[case::wrong_kind_of_card(
    MISER_CREATURE,
    "Whenever you discard a land card, create a 2/2 black Zombie creature token."
)]
#[case::an_opponent_discards(
    MISER_CREATURE,
    "Whenever an opponent discards a creature card, create a 2/2 black Zombie creature token."
)]
#[case::one_black(MISER_LAND, "Whenever you discard a land card, add {B}.")]
#[case::another_keyword_in_the_graveyard(
    BRAWN,
    "As long as this card is in your graveyard and you control a Forest, creatures you control have flying."
)]
#[case::in_your_hand(
    BRAWN,
    "As long as this card is in your hand and you control a Forest, creatures you control have trample."
)]
#[case::another_land(
    BRAWN,
    "As long as this card is in your graveyard and you control an Island, creatures you control have trample."
)]
#[case::another_subtype_enters(
    DEPARTED,
    "As long as this creature is in your graveyard, each Elf creature you control enters with an additional +1/+1 counter on it."
)]
#[case::two_counters_enter(
    DEPARTED,
    "As long as this creature is in your graveyard, each Human creature you control enters with two additional +1/+1 counters on it."
)]
#[case::commander_creatures_you_control(
    CRAFTER,
    "Commander creatures you control have \"{2}, Sacrifice an artifact: Put a +1/+1 counter on this creature and draw a card.\""
)]
#[case::sacrifice_a_creature(
    CRAFTER,
    "Commander creatures you own have \"{2}, Sacrifice a creature: Put a +1/+1 counter on this creature and draw a card.\""
)]
fn a_changed_count_kind_player_group_zone_or_duration_never_reads_back(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}
