//! Goal: the writers the fourth sample of the names G to L needed give the oracle wording of a loot by each player, a
//! creature that cannot be blocked after a counter, an entering group counted before the card, a spell of an opponent,
//! the pronoun of a legend, a look at the top cards, damage the card may deal, energy then a payment, a 1/1 creature,
//! an investigation counted by X, counters doubled under a trailing "if", support, counters put by the controller, a
//! prevention of combat damage to a group, a token that attacks and is exiled, and an Aura that sets the kind; a
//! changed count, player, kind, group, step or duration never reads back.
use super::*;
use rstest::rstest;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const GEIER: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap"],"effects":[{"Draw":{"who":"EachPlayer"}},{"Discard":{"who":"EachPlayer"}}]}}"#;
const KAPPA: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}},{"Continuous":{"affected":"This","mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}}"#;
const GIADA: &str = r#"{"Replacement":{"when":{"Enters":{"what":["Other",{"Subtype":"Angel"},{"ControlledBy":"You"}]}},"by":{"WithCounters":{"counter":"+1/+1","count":{"Count":[{"Subtype":"Angel"},{"ControlledBy":"You"}]}}}}}"#;
const PILFERER: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Not":{"CastFrom":"Hand"}}],"by":"Opponents"}},"effects":[{"Draw":{}}]}}"#;
const KILLMONGER: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Artifact"},{"ControlledBy":"Defending"}]},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#;
const REVENANT: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"if":{"Compare":{"left":{"Count":["Other",{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Eq","right":0}},"effects":[{"LookAtTop":{"count":4,"as":"top"}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}}]},"as":"kept"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}]}}"#;
const RAZERUNNERS: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":{"Either":["Player",{"Object":[{"Type":"Planeswalker"}]}]}}],"effects":[{"May":{"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":{"Counters":{"on":"This","counter":"+1/+1"}}}}]}}]}}"#;
const RUNNER: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":2}},{"Pay":{"cost":[{"PayEnergy":8}],"paid":[{"Untap":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},{"AddPhase":{"phases":["Combat"]}}]}}]}}"#;
const BEAST: &str = r#"{"Static":{"affected":"This","mods":["CantAttack"],"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Compare":{"stat":"Power","op":"Eq","to":1}},{"Compare":{"stat":"Toughness","op":"Eq","to":1}}]},"op":"Lt","right":1}}}}"#;
const FOLLOW_HIM: &str =
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"clue"},"count":"X"}}]}}"#;
const OOZE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}]},"bounds":{"min":0,"max":1}},{"what":{"Object":["Other",{"Type":"Creature"}]},"bounds":{"min":0,"max":null}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"If":{"condition":{"Compare":{"left":"X","op":"Ge","right":5}},"then":[{"Draw":{}}]}},{"If":{"condition":{"Compare":{"left":"X","op":"Ge","right":10}},"then":[{"MultiplyCounters":{"on":{"Target":1},"counter":"+1/+1"}}]}}]}}"#;
const PATRON: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":["Other",{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#;
const PATRON_DRAWS: &str = r#"{"Triggered":{"when":{"CountersPut":{"what":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}},"effects":[{"Draw":{}}]}}"#;
const HAPATRA: &str = r#"{"Triggered":{"when":{"CountersPut":{"what":[{"Type":"Creature"}],"counter":"-1/-1"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Snake","colors":["Green"],"types":["Creature"],"subtypes":["Snake"],"power":1,"toughness":1,"abilities":[{"Keyword":"Deathtouch"}]}}}}]}}"#;
const LOSHEEL: &str = r#"{"Replacement":{"when":{"Damage":{"to":{"Each":[{"Type":"Artifact"},{"Type":"Creature"},"Attacking",{"ControlledBy":"You"}]},"combat":true}},"by":"Prevent"}}"#;
const GEIST: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"}]}},"tapped":true,"attacking":true,"as":"angel"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"EndCombat"}},"effects":[{"Exile":{"what":{"Bound":"angel"}}}]}}]}}"#;
const GEISTS: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4}},"count":2,"tapped":true,"attacking":true,"as":"angel"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"EndCombat"}},"effects":[{"Exile":{"what":{"Bound":"angel"}}}]}}]}}"#;
const MOON: &str = r#"{"Static":{"affected":"Attached","mods":[{"SetTypes":["Land"]},{"SetColors":[]},"LoseAbilities",{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}}]}}"#;
const DEEP: &str = r#"{"Static":{"affected":"Attached","mods":[{"SetTypes":["Artifact"]},{"AddSubtype":"Clue"},{"SetColors":[]},"LoseAbilities",{"GrantAbility":{"Activated":{"cost":[{"Mana":"{2}"},"SacrificeSelf"],"effects":[{"Draw":{}}]}}}]}}"#;

#[rstest]
#[case::loot_by_each_player(GEIER, "{2}, {T}: Each player draws a card, then discards a card.")]
#[case::it_cannot_be_blocked(
    KAPPA,
    "When ~ enters, put a +1/+1 counter on ~. It can't be blocked this turn."
)]
#[case::angels_already_there(
    GIADA,
    "Each other Angel you control enters with an additional +1/+1 counter on it for each Angel you already control."
)]
#[case::their_hand(
    PILFERER,
    "Whenever an opponent casts a spell from anywhere other than their hand, draw a card."
)]
#[case::he_gets(
    KILLMONGER,
    "Whenever ~ attacks, he gets +1/+0 until end of turn for each artifact defending player controls."
)]
#[case::one_of_them(
    REVENANT,
    "Whenever ~ deals combat damage to a player, if you control no other creatures, look at the top four cards of your library. Put one of them into your hand and the rest on the bottom of your library in any order."
)]
#[case::have_this_creature_deal(
    RAZERUNNERS,
    "At the beginning of your end step, you may have ~ deal damage equal to the number of +1/+1 counters on it to target player or planeswalker."
)]
#[case::energy_then_pay(
    RUNNER,
    "Whenever ~ attacks, you get {E}{E}, then you may pay eight {E}. If you pay, untap all creatures you control, and after this phase, there is an additional combat phase."
)]
#[case::a_one_one_creature(BEAST, "~ can't attack unless you control a 1/1 creature.")]
#[case::investigate_x_times(FOLLOW_HIM, "Investigate X times.")]
#[case::counters_doubled_under_an_if(
    OOZE,
    "When ~ enters, destroy up to one target artifact or enchantment with mana value X or less. If X is 5 or more, you draw a card. If X is 10 or more, double the number of +1/+1 counters on any number of other target creatures."
)]
#[case::support(PATRON, "When ~ enters, support 2.")]
#[case::counters_you_put(
    PATRON_DRAWS,
    "Whenever you put one or more counters on a creature you don't control, draw a card."
)]
#[case::counters_of_a_kind_you_put(
    HAPATRA,
    "Whenever you put one or more -1/-1 counters on a creature, create a 1/1 green Snake creature token with deathtouch."
)]
#[case::combat_damage_prevented_to_a_group(
    LOSHEEL,
    "Prevent all combat damage that would be dealt to attacking artifact creatures you control."
)]
#[case::token_exiled_at_end_of_combat(
    GEIST,
    "Whenever ~ attacks, create a 4/4 white Angel creature token with flying that's tapped and attacking. Exile that token at end of combat."
)]
#[case::aura_makes_a_land(
    MOON,
    "Enchanted permanent is a colorless land with \"{T}: Add {C}\" and loses all other card types and abilities."
)]
#[case::aura_makes_a_clue(
    DEEP,
    "Enchanted permanent is a colorless Clue artifact with \"{2}, Sacrifice ~: Draw a card\" and loses all other abilities."
)]
fn the_sample_wordings_are_written_as_the_oracle_words_them(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::loot_draws_two(GEIER, "{2}, {T}: Each player draws two cards, then discards a card.")]
#[case::loot_in_the_other_order(GEIER, "{2}, {T}: Each player discards a card, then draws a card.")]
#[case::loot_by_each_opponent(GEIER, "{2}, {T}: Each opponent draws a card, then discards a card.")]
#[case::blocked_until_your_next_turn(
    KAPPA,
    "When ~ enters, put a +1/+1 counter on ~. It can't be blocked until your next turn."
)]
#[case::cannot_block(
    KAPPA,
    "When ~ enters, put a +1/+1 counter on ~. It can't block this turn."
)]
#[case::demons_already_there(
    GIADA,
    "Each other Angel you control enters with an additional +1/+1 counter on it for each Demon you already control."
)]
#[case::two_counters_for_each_angel(
    GIADA,
    "Each other Angel you control enters with two additional +1/+1 counters on it for each Angel you already control."
)]
#[case::their_graveyard(
    PILFERER,
    "Whenever an opponent casts a spell from anywhere other than their graveyard, draw a card."
)]
#[case::a_player_casts(
    PILFERER,
    "Whenever a player casts a spell from anywhere other than their hand, draw a card."
)]
#[case::she_gets_two(
    KILLMONGER,
    "Whenever ~ attacks, he gets +2/+0 until end of turn for each artifact defending player controls."
)]
#[case::for_each_creature(
    KILLMONGER,
    "Whenever ~ attacks, he gets +1/+0 until end of turn for each creature defending player controls."
)]
#[case::two_of_them(
    REVENANT,
    "Whenever ~ deals combat damage to a player, if you control no other creatures, look at the top four cards of your library. Put two of them into your hand and the rest on the bottom of your library in any order."
)]
#[case::in_a_random_order(
    REVENANT,
    "Whenever ~ deals combat damage to a player, if you control no other creatures, look at the top four cards of your library. Put one of them into your hand and the rest on the bottom of your library in a random order."
)]
#[case::damage_to_a_creature(
    RAZERUNNERS,
    "At the beginning of your end step, you may have ~ deal damage equal to the number of +1/+1 counters on it to target creature."
)]
#[case::damage_by_other_counters(
    RAZERUNNERS,
    "At the beginning of your end step, you may have ~ deal damage equal to the number of -1/-1 counters on it to target player or planeswalker."
)]
#[case::seven_energy(
    RUNNER,
    "Whenever ~ attacks, you get {E}{E}, then you may pay seven {E}. If you pay, untap all creatures you control, and after this phase, there is an additional combat phase."
)]
#[case::untap_the_attackers(
    RUNNER,
    "Whenever ~ attacks, you get {E}{E}, then you may pay eight {E}. If you pay, untap all attacking creatures, and after this phase, there is an additional combat phase."
)]
#[case::a_two_two_creature(BEAST, "~ can't attack unless you control a 2/2 creature.")]
#[case::a_one_two_creature(BEAST, "~ can't attack unless you control a 1/2 creature.")]
#[case::investigate_three_times(FOLLOW_HIM, "Investigate three times.")]
#[case::investigate_twice_x(FOLLOW_HIM, "Investigate X times, then investigate X times.")]
#[case::draw_at_six_or_more(
    OOZE,
    "When ~ enters, destroy up to one target artifact or enchantment with mana value X or less. If X is 6 or more, you draw a card. If X is 10 or more, double the number of +1/+1 counters on any number of other target creatures."
)]
#[case::double_at_nine_or_more(
    OOZE,
    "When ~ enters, destroy up to one target artifact or enchantment with mana value X or less. If X is 5 or more, you draw a card. If X is 9 or more, double the number of +1/+1 counters on any number of other target creatures."
)]
#[case::support_three(PATRON, "When ~ enters, support 3.")]
#[case::support_one(PATRON, "When ~ enters, support 1.")]
#[case::counters_on_a_creature_you_control(
    PATRON_DRAWS,
    "Whenever you put one or more counters on a creature you control, draw a card."
)]
#[case::counters_on_a_land(
    PATRON_DRAWS,
    "Whenever you put one or more counters on a land you don't control, draw a card."
)]
#[case::plus_counters_you_put(
    HAPATRA,
    "Whenever you put one or more +1/+1 counters on a creature, create a 1/1 green Snake creature token with deathtouch."
)]
#[case::damage_of_all_kinds_prevented(
    LOSHEEL,
    "Prevent all damage that would be dealt to attacking artifact creatures you control."
)]
#[case::combat_damage_to_any_attacker(
    LOSHEEL,
    "Prevent all combat damage that would be dealt to attacking creatures you control."
)]
#[case::combat_damage_to_artifact_creatures(
    LOSHEEL,
    "Prevent all combat damage that would be dealt to artifact creatures you control."
)]
#[case::exiled_at_the_next_end_step(
    GEIST,
    "Whenever ~ attacks, create a 4/4 white Angel creature token with flying that's tapped and attacking. Exile that token at the beginning of the next end step."
)]
#[case::sacrificed_at_end_of_combat(
    GEIST,
    "Whenever ~ attacks, create a 4/4 white Angel creature token with flying that's tapped and attacking. Sacrifice that token at end of combat."
)]
#[case::a_3_3_token(
    GEIST,
    "Whenever ~ attacks, create a 3/3 white Angel creature token with flying that's tapped and attacking. Exile that token at end of combat."
)]
#[case::two_tokens_exiled_as_one(
    GEISTS,
    "Whenever ~ attacks, create two 4/4 white Angel creature tokens that are tapped and attacking. Exile that token at end of combat."
)]
#[case::a_red_land(
    MOON,
    "Enchanted permanent is a red land with \"{T}: Add {C}\" and loses all other card types and abilities."
)]
#[case::only_the_abilities_are_lost(
    MOON,
    "Enchanted permanent is a colorless land with \"{T}: Add {C}\" and loses all other abilities."
)]
#[case::other_mana(
    MOON,
    "Enchanted permanent is a colorless land with \"{T}: Add {G}\" and loses all other card types and abilities."
)]
#[case::a_creature_clue(
    DEEP,
    "Enchanted permanent is a colorless Clue creature with \"{2}, Sacrifice ~: Draw a card\" and loses all other abilities."
)]
fn a_changed_count_player_kind_group_step_or_duration_never_reads_back(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}
