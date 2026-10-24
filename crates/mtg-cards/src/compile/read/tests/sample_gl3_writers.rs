//! Goal: the writers the third sample of the names G to L needed give the oracle wording of a trigger that works only
//! for a kicked card, a gain of life for each kick, a creature that entered under a player's control this turn, a
//! cycled card's trigger, counters removed with the sacrifice, an untapped legendary permanent tapped as a cost, a
//! group that regenerates or gains a keyword, counters on the creature of an event, a draw by you and a target, a
//! comparison with the spell's mana value, "Do this only once each turn", a group attack at a player, two targets
//! destroyed together and "another permanent" sacrificed; a changed count, player, kind, group, zone or duration never
//! reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const ZOMBIES: &str = r#"{"CreateToken":{"token":{"Custom":{"name":"Zombie Knight","colors":["Black"],"types":["Creature"],"subtypes":["Zombie","Knight"],"power":2,"toughness":2,"abilities":[{"Keyword":"Menace"}]}},"count":8}}"#;
const JOSU: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":"Kicked","effects":[{"CreateToken":{"token":{"Custom":{"name":"Zombie Knight","colors":["Black"],"types":["Creature"],"subtypes":["Zombie","Knight"],"power":2,"toughness":2,"abilities":[{"Keyword":"Menace"}]}},"count":8}}]}}"#;
const SANCTIFIERS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":"Kicked","targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;
const LIGHTKEEPER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"GainLife":{"amount":{"Times":{"by":2,"of":"Kicks"}}}}]}}"#;
const FLOCK: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},"EnteredThisTurn"]},"op":"Ge","right":1}},"effects":[{"Draw":{"count":1}}]}}"#;
const LICTOR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"Opponents"},"EnteredThisTurn"]},"op":"Ge","right":1}},"effects":[{"Draw":{"count":1}}]}}"#;
const POLLUTER: &str = r#"{"Triggered":{"when":{"Cycled":{"what":["This"],"by":"You"}},"from":"Graveyard","targets":[{"what":"Player"}],"effects":[{"May":{"effects":[{"LoseLife":{"who":{"Target":0},"amount":{"Count":[{"Subtype":"Zombie"}]}}}]}}]}}"#;
const INCINERATOR: &str = r#"{"Triggered":{"when":{"Cycled":{"what":["This"],"by":"You"}},"from":"Graveyard","targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"May":{"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":{"Count":[{"Subtype":"Goblin"}]}}}]}}]}}"#;
const QUEST: &str = r#"{"Activated":{"cost":[{"RemoveCounters":{"counter":{"Other":"quest"},"count":3}},"SacrificeSelf"],"effects":[{"Draw":{"count":2}}]}}"#;
const SHAKU: &str = r#"{"Activated":{"cost":[{"TapPermanents":{"filter":[{"Supertype":"Legendary"}]}}],"effects":[{"Untap":{"what":"This"}}]}}"#;
const REGENERATE: &str = r#"{"Spell":{"effects":[{"Regenerate":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}]}}"#;
const ACCURSED: &str = r#"{"Activated":{"cost":[{"Mana":"{1}{B}"},"Tap"],"effects":[{"Continuous":{"affected":{"Each":[{"Subtype":"Zombie"}]},"mods":[{"AddKeyword":"Menace"}],"duration":"UntilEndOfTurn"}}]}}"#;
const UNICORN: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"Triggering","counter":"+1/+1"}}]}}"#;
const LORAN: &str = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":"Opponent"}],"effects":[{"Draw":{"count":1}},{"Draw":{"who":{"Target":0},"count":1}}]}}"#;
const TYRANT: &str = r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}},{"ControlledBy":"Opponents"},{"Compare":{"stat":"ManaValue","op":"Le","to":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]},"bounds":{"min":0,"max":1}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#;
const KRILE: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Not":{"Type":"Creature"}}],"by":"You"}},"limit":"OncePerTurn","targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"},{"Compare":{"stat":"ManaValue","op":"Eq","to":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]}}],"effects":[{"May":{"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}]}}"#;
const LANDROVAL: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at_least":2}},"targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking",{"Not":{"HasKeyword":"Flying"}}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Flying"}],"duration":"UntilEndOfTurn"}}]}}"#;
const BREACH: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}},{"what":{"Object":[{"Type":"Enchantment"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"Destroy":{"what":{"Target":1}}}]}}"#;
const KORVOLD: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Sacrifice":{"filter":["Other"]}}]}}"#;

#[rstest]
#[case::he_was_kicked(
    JOSU,
    "When ~ enters, if he was kicked, create eight 2/2 black Zombie Knight creature tokens with menace."
)]
#[case::it_was_kicked(
    JOSU,
    "When ~ enters, if it was kicked, create eight 2/2 black Zombie Knight creature tokens with menace."
)]
#[case::kicked_destroy(
    SANCTIFIERS,
    "When ~ enters, if it was kicked, destroy target artifact or enchantment."
)]
#[case::life_for_each_kick(
    LIGHTKEEPER,
    "When ~ enters, you gain 2 life for each time it was kicked."
)]
#[case::entered_under_your_control(
    FLOCK,
    "At the beginning of your end step, if a creature entered the battlefield under your control this turn, draw a card."
)]
#[case::entered_under_an_opponents_control(
    LICTOR,
    "When ~ enters, if a creature entered the battlefield under an opponent's control this turn, draw a card."
)]
#[case::cycled_player_loses_life(
    POLLUTER,
    "When you cycle ~, you may have target player lose life equal to the number of Zombies on the battlefield."
)]
#[case::cycled_damage_with_x(
    INCINERATOR,
    "When you cycle ~, you may have it deal X damage to target creature, where X is the number of Goblins on the battlefield."
)]
#[case::counters_and_the_sacrifice(
    QUEST,
    "Remove three quest counters from ~ and sacrifice it: Draw two cards."
)]
#[case::untapped_before_the_supertype(
    SHAKU,
    "Tap an untapped legendary permanent you control: Untap ~."
)]
#[case::regenerate_each(REGENERATE, "Regenerate each creature you control.")]
#[case::all_gain(ACCURSED, "{1}{B}, {T}: All Zombies gain menace until end of turn.")]
#[case::counter_on_that_creature(
    UNICORN,
    "Whenever another creature you control enters, put a +1/+1 counter on that creature."
)]
#[case::you_and_a_target_each_draw(LORAN, "{T}: You and target opponent each draw a card.")]
#[case::value_not_greater_than_the_spell(
    TYRANT,
    "Whenever you cast a spell, return up to one target nonland permanent an opponent controls with mana value less than or equal to that spell's mana value to its owner's hand."
)]
#[case::value_equal_to_the_spell(
    KRILE,
    "Whenever you cast a noncreature spell, you may return target creature card with mana value equal to that spell's mana value from your graveyard to your hand. Do this only once each turn."
)]
#[case::group_attacks_a_player(
    LANDROVAL,
    "Whenever two or more creatures you control attack a player, target attacking creature without flying gains flying until end of turn."
)]
#[case::two_targets_destroyed(BREACH, "Destroy target artifact and target enchantment.")]
#[case::another_permanent_sacrificed(KORVOLD, "Whenever ~ attacks, sacrifice another permanent.")]
fn the_sample_wordings_are_written_as_the_oracle_words_them(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::fewer_tokens(
    JOSU,
    "When ~ enters, if he was kicked, create seven 2/2 black Zombie Knight creature tokens with menace."
)]
#[case::kicked_twice(
    JOSU,
    "When ~ enters, if he was kicked twice, create eight 2/2 black Zombie Knight creature tokens with menace."
)]
#[case::other_destroyed_kind(
    SANCTIFIERS,
    "When ~ enters, if it was kicked, destroy target artifact or creature."
)]
#[case::a_third_of_the_kicks(
    LIGHTKEEPER,
    "When ~ enters, you gain 3 life for each time it was kicked."
)]
#[case::for_each_time_it_was_cast(
    LIGHTKEEPER,
    "When ~ enters, you gain 2 life for each time it was cast."
)]
#[case::your_creature_for_the_opponents(
    FLOCK,
    "At the beginning of your end step, if a creature entered the battlefield under an opponent's control this turn, draw a card."
)]
#[case::a_land_instead(
    LICTOR,
    "When ~ enters, if a land entered the battlefield under an opponent's control this turn, draw a card."
)]
#[case::an_opponent_for_you(
    LICTOR,
    "When ~ enters, if a creature entered the battlefield under your control this turn, draw a card."
)]
#[case::other_group_counted(
    POLLUTER,
    "When you cycle ~, you may have target player lose life equal to the number of Goblins on the battlefield."
)]
#[case::not_a_player_but_you(
    POLLUTER,
    "When you cycle ~, you may lose life equal to the number of Zombies on the battlefield."
)]
#[case::a_fixed_number(
    INCINERATOR,
    "When you cycle ~, you may have it deal 2 damage to target creature, where X is the number of Goblins on the battlefield."
)]
#[case::another_group(
    INCINERATOR,
    "When you cycle ~, you may have it deal X damage to target creature, where X is the number of Zombies on the battlefield."
)]
#[case::fewer_counters(
    QUEST,
    "Remove two quest counters from ~ and sacrifice it: Draw two cards."
)]
#[case::another_counter_kind(
    QUEST,
    "Remove three charge counters from ~ and sacrifice it: Draw two cards."
)]
#[case::exile_it_instead(
    QUEST,
    "Remove three quest counters from ~ and exile it: Draw two cards."
)]
#[case::two_permanents_tapped(SHAKU, "Tap two untapped legendary permanents you control: Untap ~.")]
#[case::no_supertype(SHAKU, "Tap an untapped permanent you control: Untap ~.")]
#[case::regenerate_all_of_them(REGENERATE, "Regenerate each creature.")]
#[case::regenerate_theirs(REGENERATE, "Regenerate each creature you don't control.")]
#[case::another_kind_gains(ACCURSED, "{1}{B}, {T}: All Vampires gain menace until end of turn.")]
#[case::another_keyword_gained(ACCURSED, "{1}{B}, {T}: All Zombies gain flying until end of turn.")]
#[case::until_your_next_turn(
    ACCURSED,
    "{1}{B}, {T}: All Zombies gain menace until your next turn."
)]
#[case::a_counter_on_the_card(
    UNICORN,
    "Whenever another creature you control enters, put a +1/+1 counter on ~."
)]
#[case::two_counters_on_that_creature(
    UNICORN,
    "Whenever another creature you control enters, put two +1/+1 counters on that creature."
)]
#[case::each_draw_two(LORAN, "{T}: You and target opponent each draw two cards.")]
#[case::each_draw_a_target_player(LORAN, "{T}: You and target player each draw a card.")]
#[case::strictly_less_than_the_spell(
    TYRANT,
    "Whenever you cast a spell, return up to one target nonland permanent an opponent controls with mana value less than that spell's mana value to its owner's hand."
)]
#[case::value_of_another_spell(
    TYRANT,
    "Whenever you cast a spell, return up to one target nonland permanent an opponent controls with mana value less than or equal to the mana value of ~ to its owner's hand."
)]
#[case::do_this_twice(
    KRILE,
    "Whenever you cast a noncreature spell, you may return target creature card with mana value equal to that spell's mana value from your graveyard to your hand. Do this only twice each turn."
)]
#[case::value_at_most_the_spells(
    KRILE,
    "Whenever you cast a noncreature spell, you may return target creature card with mana value less than or equal to that spell's mana value from your graveyard to your hand. Do this only once each turn."
)]
#[case::three_attackers(
    LANDROVAL,
    "Whenever three or more creatures you control attack a player, target attacking creature without flying gains flying until end of turn."
)]
#[case::flying_for_the_turn_first(
    LANDROVAL,
    "Whenever two or more creatures you control attack a player, target attacking creature without flying gains flying until your next turn."
)]
#[case::two_targets_of_one_kind(BREACH, "Destroy target artifact and target creature.")]
#[case::the_wrong_order(BREACH, "Destroy target enchantment and target artifact.")]
#[case::sacrifice_a_creature_instead(KORVOLD, "Whenever ~ attacks, sacrifice another creature.")]
#[case::sacrifice_any_permanent(KORVOLD, "Whenever ~ attacks, sacrifice a permanent.")]
fn a_changed_count_player_kind_group_zone_or_duration_never_reads_back(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}

#[test]
fn the_token_of_the_kicked_trigger_is_a_plain_effect() {
    // The kicked wording rests on the wordings of the effect alone: a token effect with no condition reads on its own.
    assert!(written(
        &format!(r#"{{"Spell":{{"effects":[{ZOMBIES}]}}}}"#),
        "Create eight 2/2 black Zombie Knight creature tokens with menace."
    ));
}
