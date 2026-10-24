//! Goal: the writers of the proposal checker give the oracle wording of the third sample of the names M to R (pronouns, groups, permissions to cast, shuffles, payments and graveyards), and a wrong object, count, player, kind or step never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// "Whenever this creature deals combat damage to a player, you may have it deal that much damage to target creature that player controls."
const MORDANT_DRAGON: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Triggering"}]}}],"effects":[{"May":{"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":"Triggering"}}]}}]}}"#;

/// "Discard a card: This creature gains hexproof until end of turn. Tap it."
const PROGNOSTIC_SPHINX: &str = r#"{"Activated":{"cost":[{"Discard":{}}],"effects":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Hexproof"}],"duration":"UntilEndOfTurn"}},{"Tap":{"what":"This"}}]}}"#;

/// "~ deals 1 damage to target creature you control. That creature gains trample until end of turn. Draw a card."
const RILE: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":1}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}},{"Draw":{}}]}}"#;

/// "{X}{U}{R}, {T}: ~ deals X damage to target creature. That creature's controller draws X cards."
const NIN: &str = r#"{"Activated":{"cost":[{"Mana":"{X}{U}{R}"},"Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":"X"}},{"Draw":{"who":{"ControllerOf":{"Target":0}},"count":"X"}}]}}"#;

/// "Whenever a creature an opponent controls dies, that player loses 2 life."
const MASSACRE_WURM: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"LoseLife":{"who":{"ControllerOf":"Triggering"},"amount":2}}]}}"#;

/// "When ~ enters, return up to one other target artifact or creature you control to its owner's hand. If it was an artifact, put a +1/+1 counter on ~."
const RESCUE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]},"Other",{"ControlledBy":"You"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}},{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Type":"Artifact"}]}},"then":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}]}}"#;

/// "Whenever you gain life, you may put that many +1/+1 counters on each creature you control. Do this only once each turn."
const NYKTHOS_PARAGON: &str = r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"limit":"OncePerTurn","effects":[{"May":{"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1","count":"Triggering"}}]}}]}}"#;

/// "When ~ enters, sacrifice it unless you return another creature you control to its owner's hand."
const QUICKLING: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Pay":{"cost":[{"ReturnToHand":{"filter":["Other",{"Type":"Creature"}]}}],"unpaid":[{"Sacrifice":{"filter":["This"]}}]}}]}}"#;

#[rstest]
#[case::have_it_deal(
    MORDANT_DRAGON,
    "Whenever ~ deals combat damage to a player, you may have it deal that much damage to target creature that player controls."
)]
#[case::tap_it(
    PROGNOSTIC_SPHINX,
    "Discard a card: ~ gains hexproof until end of turn. Tap it."
)]
#[case::that_creature_gains(
    RILE,
    "~ deals 1 damage to target creature you control. That creature gains trample until end of turn. Draw a card."
)]
#[case::that_creatures_controller(
    NIN,
    "{X}{U}{R}, {T}: ~ deals X damage to target creature. That creature's controller draws X cards."
)]
#[case::that_player(
    MASSACRE_WURM,
    "Whenever a creature an opponent controls dies, that player loses 2 life."
)]
#[case::it_was_an_artifact(
    RESCUE,
    "When ~ enters, return up to one other target artifact or creature you control to its owner's hand. If it was an artifact, put a +1/+1 counter on ~."
)]
#[case::do_this_once(
    NYKTHOS_PARAGON,
    "Whenever you gain life, you may put that many +1/+1 counters on each creature you control. Do this only once each turn."
)]
#[case::sacrifice_it_unless(
    QUICKLING,
    "When ~ enters, sacrifice it unless you return another creature you control to its owner's hand."
)]
fn a_card_says_it_with_the_pronoun_of_the_text(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_pronoun(
    PROGNOSTIC_SPHINX,
    "Discard a card: ~ gains hexproof until end of turn. Untap it."
)]
#[case::another_object(
    PROGNOSTIC_SPHINX,
    "Discard a card: ~ gains hexproof until end of turn. Tap that creature."
)]
#[case::another_keyword(
    RILE,
    "~ deals 1 damage to target creature you control. That creature gains flying until end of turn. Draw a card."
)]
#[case::another_player(
    NIN,
    "{X}{U}{R}, {T}: ~ deals X damage to target creature. That creature's owner draws X cards."
)]
#[case::another_count(
    MASSACRE_WURM,
    "Whenever a creature an opponent controls dies, that player loses 3 life."
)]
#[case::another_kind(
    RESCUE,
    "When ~ enters, return up to one other target artifact or creature you control to its owner's hand. If it was a creature, put a +1/+1 counter on ~."
)]
#[case::another_limit(
    NYKTHOS_PARAGON,
    "Whenever you gain life, you may put that many +1/+1 counters on each creature you control. Do this only once each game."
)]
#[case::another_return(
    QUICKLING,
    "When ~ enters, sacrifice it unless you return a land you control to its owner's hand."
)]
fn another_pronoun_object_player_or_count_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "Whenever this creature or another Zombie enters, all non-Zombie creatures get -1/-1 until end of turn."
const NOXIOUS_GHOUL: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":["Other",{"Subtype":"Zombie"}],"to":"Battlefield"}}]},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"Subtype":"Zombie"}}]},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]}}"#;

/// "Whenever a land you control enters, you may have target player lose 3 life. If you do, put three +1/+1 counters on ~."
const OB_NIXILIS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"May":{"effects":[{"LoseLife":{"who":{"Target":0},"amount":3}}]}},{"If":{"condition":"Performed","then":[{"PutCounters":{"on":"This","counter":"+1/+1","count":3}}]}}]}}"#;

/// "{T}, Pay six {E}: Draw three cards."
const ROIL_CARTOGRAPHER: &str =
    r#"{"Activated":{"cost":["Tap",{"PayEnergy":6}],"effects":[{"Draw":{"count":3}}]}}"#;

#[rstest]
#[case::all_non_creatures(
    NOXIOUS_GHOUL,
    "Whenever ~ or another Zombie enters, all non-Zombie creatures get -1/-1 until end of turn."
)]
#[case::have_the_player_lose(
    OB_NIXILIS,
    "Whenever a land you control enters, you may have target player lose 3 life. If you do, put three +1/+1 counters on ~."
)]
#[case::energy_in_words(ROIL_CARTOGRAPHER, "{T}, Pay six {E}: Draw three cards.")]
fn all_have_and_pay_wordings_read(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_subtype(
    NOXIOUS_GHOUL,
    "Whenever ~ or another Zombie enters, all non-Vampire creatures get -1/-1 until end of turn."
)]
#[case::another_bonus(
    NOXIOUS_GHOUL,
    "Whenever ~ or another Zombie enters, all non-Zombie creatures get -2/-2 until end of turn."
)]
#[case::the_group_itself(
    NOXIOUS_GHOUL,
    "Whenever ~ or another Zombie enters, all Zombie creatures get -1/-1 until end of turn."
)]
#[case::another_opponent(
    OB_NIXILIS,
    "Whenever a land you control enters, you may have target opponent lose 3 life. If you do, put three +1/+1 counters on ~."
)]
#[case::another_life(
    OB_NIXILIS,
    "Whenever a land you control enters, you may have target player lose 2 life. If you do, put three +1/+1 counters on ~."
)]
#[case::fewer_counters(ROIL_CARTOGRAPHER, "{T}, Pay five {E}: Draw three cards.")]
#[case::another_draw(ROIL_CARTOGRAPHER, "{T}, Pay six {E}: Draw two cards.")]
fn another_group_player_or_payment_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "~ deals X damage to each creature without flying and each planeswalker."
const MAGMAQUAKE: &str = r#"{"Spell":{"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}}]},"amount":"X"}},{"DealDamage":{"to":{"Each":[{"Type":"Planeswalker"}]},"amount":"X"}}]}}"#;

/// "{R}: ~ deals 1 damage to each creature and each player."
const PYROHEMIA: &str = r#"{"Activated":{"cost":"{R}","effects":[{"DealDamage":{"source":"This","to":{"Each":[{"Type":"Creature"}]},"amount":1}},{"DealDamage":{"source":"This","to":"EachPlayer","amount":1}}]}}"#;

/// "Return target creature you control and target creature you don't control to their owners' hands."
const PEEL_FROM_REALITY: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}},{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}},{"Move":{"what":{"Target":1},"to":"Hand"}}]}}"#;

#[rstest]
#[case::two_groups_hit_at_once(
    MAGMAQUAKE,
    "~ deals X damage to each creature without flying and each planeswalker."
)]
#[case::creatures_and_players(PYROHEMIA, "{R}: ~ deals 1 damage to each creature and each player.")]
#[case::two_targets_returned_together(
    PEEL_FROM_REALITY,
    "Return target creature you control and target creature you don't control to their owners' hands."
)]
fn damage_to_two_groups_and_the_return_of_two_targets_are_one_sentence(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_group(
    MAGMAQUAKE,
    "~ deals X damage to each creature without flying and each player."
)]
#[case::another_amount(
    MAGMAQUAKE,
    "~ deals 2 damage to each creature without flying and each planeswalker."
)]
#[case::groups_swapped(PYROHEMIA, "{R}: ~ deals 1 damage to each player and each creature.")]
#[case::another_damage(PYROHEMIA, "{R}: ~ deals 2 damage to each creature and each player.")]
#[case::targets_swapped(
    PEEL_FROM_REALITY,
    "Return target creature you don't control and target creature you control to their owners' hands."
)]
#[case::the_same_kind_twice(
    PEEL_FROM_REALITY,
    "Return target creature you control and target creature you control to their owners' hands."
)]
fn another_group_amount_or_target_does_not_join(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "You may cast historic spells as though they had flash."
const RAFF: &str = r#"{"Rule":{"rule":{"MayCast":{"what":[{"AnyOf":[{"Type":"Artifact"},{"Supertype":"Legendary"},{"Subtype":"Saga"}]}],"flash":true}}}}"#;

/// "Any player may cast Sliver spells as though they had flash."
const QUICK_SLIVER: &str = r#"{"Rule":{"rule":{"MayCast":{"who":"EachPlayer","what":[{"Subtype":"Sliver"}],"flash":true}}}}"#;

/// "As long as ~ is tapped, you may cast spells as though they had flash."
const QUICKSILVER: &str = r#"{"Rule":{"rule":{"MayCast":{"flash":true}},"condition":{"Matches":{"subject":"This","selector":["Tapped"]}}}}"#;

/// "Creature spells you control can't be countered."
const RHYTHM_OF_THE_WILD: &str = r#"{"Rule":{"rule":{"CantBeCountered":{"Each":[{"Type":"Creature"},"Spell",{"ControlledBy":"You"}]}}}}"#;

#[rstest]
#[case::historic_spells(RAFF, "You may cast historic spells as though they had flash.")]
#[case::any_player_a_subtype(
    QUICK_SLIVER,
    "Any player may cast Sliver spells as though they had flash."
)]
#[case::while_tapped(
    QUICKSILVER,
    "As long as ~ is tapped, you may cast spells as though they had flash."
)]
#[case::creature_spells_are_safe(
    RHYTHM_OF_THE_WILD,
    "Creature spells you control can't be countered."
)]
fn flash_permissions_and_safe_spells_read(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_player(RAFF, "Any player may cast historic spells as though they had flash.")]
#[case::another_kind(RAFF, "You may cast legendary spells as though they had flash.")]
#[case::you_not_any_player(QUICK_SLIVER, "You may cast Sliver spells as though they had flash.")]
#[case::another_subtype(
    QUICK_SLIVER,
    "Any player may cast Spirit spells as though they had flash."
)]
#[case::untapped(
    QUICKSILVER,
    "As long as ~ is untapped, you may cast spells as though they had flash."
)]
#[case::no_condition(QUICKSILVER, "You may cast spells as though they had flash.")]
#[case::another_card_type(RHYTHM_OF_THE_WILD, "Artifact spells you control can't be countered.")]
#[case::another_controller(
    RHYTHM_OF_THE_WILD,
    "Creature spells your opponents control can't be countered."
)]
fn another_permission_player_kind_or_condition_does_not_read(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}

/// "At the beginning of your upkeep, if you have four or more creature cards in your graveyard, you may return target creature card from your graveyard to your hand."
const OVERSOLD_CEMETERY: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"}]},"op":"Ge","right":4}},"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"}]}}],"effects":[{"May":{"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}]}}"#;

/// "~ deals 4 damage to any target. Look at the top four cards of your library. Put one of those cards into your hand and the rest on the bottom of your library in any order."
const PROPHETIC_BOLT: &str = r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":4}},{"LookAtTop":{"count":4,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}]}}"#;

/// "Whenever this creature attacks while you don't control another Dinosaur, put a stun counter on it."
const PUGNACIOUS_HAMMERSKULL: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"Compare":{"left":{"Count":[{"Subtype":"Dinosaur"},"Other",{"ControlledBy":"You"}]},"op":"Eq","right":0}},"effects":[{"PutCounters":{"on":"This","counter":{"Other":"stun"}}}]}}"#;

/// "{4}, {T}: Exile target attacking creature an opponent controls. At the beginning of the next end step, return it to the battlefield tapped under its owner's control."
const MYSTIFYING_MAZE: &str = r#"{"Activated":{"cost":[{"Mana":"{4}"},"Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking",{"ControlledBy":"Opponents"}]}}],"effects":[{"Exile":{"what":{"Target":0},"as":"exiled"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{"tapped":true}}}}]}}]}}"#;

#[rstest]
#[case::four_or_more_in_words(
    OVERSOLD_CEMETERY,
    "At the beginning of your upkeep, if you have four or more creature cards in your graveyard, you may return target creature card from your graveyard to your hand."
)]
#[case::one_of_those_cards(
    PROPHETIC_BOLT,
    "~ deals 4 damage to any target. Look at the top four cards of your library. Put one of those cards into your hand and the rest on the bottom of your library in any order."
)]
#[case::while_you_dont_control_another(
    PUGNACIOUS_HAMMERSKULL,
    "Whenever this creature attacks while you don't control another Dinosaur, put a stun counter on it."
)]
#[case::the_time_first(
    MYSTIFYING_MAZE,
    "{4}, {T}: Exile target attacking creature an opponent controls. At the beginning of the next end step, return it to the battlefield tapped under its owner's control."
)]
fn counts_picks_conditions_and_delays_read(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_number(
    OVERSOLD_CEMETERY,
    "At the beginning of your upkeep, if you have five or more creature cards in your graveyard, you may return target creature card from your graveyard to your hand."
)]
#[case::another_zone(
    OVERSOLD_CEMETERY,
    "At the beginning of your upkeep, if you have four or more creature cards in your hand, you may return target creature card from your graveyard to your hand."
)]
#[case::another_pick(
    PROPHETIC_BOLT,
    "~ deals 4 damage to any target. Look at the top four cards of your library. Put two of those cards into your hand and the rest on the bottom of your library in any order."
)]
#[case::another_subtype(
    PUGNACIOUS_HAMMERSKULL,
    "Whenever this creature attacks while you don't control another Dragon, put a stun counter on it."
)]
#[case::another_counter(
    PUGNACIOUS_HAMMERSKULL,
    "Whenever this creature attacks while you don't control another Dinosaur, put a charge counter on it."
)]
#[case::another_step(
    MYSTIFYING_MAZE,
    "{4}, {T}: Exile target attacking creature an opponent controls. At the beginning of the next combat, return it to the battlefield tapped under its owner's control."
)]
#[case::untapped_return(
    MYSTIFYING_MAZE,
    "{4}, {T}: Exile target attacking creature an opponent controls. At the beginning of the next end step, return it to the battlefield under its owner's control."
)]
fn another_count_pick_condition_or_delay_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "Whenever you discard a card, you may exile that card from your graveyard. If you do, until the end of your next turn, you may play that card."
const MOONSTONE: &str = r#"{"Triggered":{"when":{"Discarded":{"by":"You"}},"effects":[{"May":{"effects":[{"Exile":{"what":"Triggering","as":"exiled"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"exiled"},"play":true}},"duration":"UntilEndOfYourNextTurn"}}]}}]}}"#;

/// "Whenever an opponent gains life, you may pay {R}. If you do, return this card from your graveyard to your hand."
const PUNISHING_FIRE: &str = r#"{"Triggered":{"when":{"LifeGained":{"who":"Opponents"}},"from":"Graveyard","effects":[{"Pay":{"cost":"{R}","paid":[{"Move":{"what":"This","to":"Hand"}}]}}]}}"#;

#[rstest]
#[case::play_until_the_end_of_your_next_turn(
    MOONSTONE,
    "Whenever you discard a card, you may exile that card from your graveyard. If you do, until the end of your next turn, you may play that card."
)]
#[case::pay_to_return_from_the_graveyard(
    PUNISHING_FIRE,
    "Whenever an opponent gains life, you may pay {R}. If you do, return this card from your graveyard to your hand."
)]
fn exiled_and_returned_cards_read(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::this_turn(
    MOONSTONE,
    "Whenever you discard a card, you may exile that card from your graveyard. If you do, you may play that card this turn."
)]
#[case::cast_not_play(
    MOONSTONE,
    "Whenever you discard a card, you may exile that card from your graveyard. If you do, until the end of your next turn, you may cast that card."
)]
#[case::another_cost(
    PUNISHING_FIRE,
    "Whenever an opponent gains life, you may pay {G}. If you do, return this card from your graveyard to your hand."
)]
#[case::another_zone(
    PUNISHING_FIRE,
    "Whenever an opponent gains life, you may pay {R}. If you do, return this card from your graveyard to the battlefield."
)]
fn another_duration_cost_or_zone_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "The owner of target nonland permanent shuffles it into their library, then draws two cards."
const OBLATION: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Shuffled"}}}},{"Draw":{"who":{"OwnerOf":{"Target":0}},"count":2}}]}}"#;

/// "{5}, {T}, Exile this land: Target creature's owner shuffles it into their library."
const OMINOUS_CEMETERY: &str = r#"{"Activated":{"cost":[{"Mana":"{5}"},"Tap","ExileSelf"],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Shuffled"}}}}]}}"#;

/// "{2}, Exile this artifact: Shuffle any number of target cards from your graveyard into your library."
const PERPETUAL_TIMEPIECE: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"ExileSelf"],"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":null}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Shuffled"}}}}]}}"#;

#[rstest]
#[case::the_owner_shuffles_and_draws(
    OBLATION,
    "The owner of target nonland permanent shuffles it into their library, then draws two cards."
)]
#[case::the_owner_of_a_creature(
    OMINOUS_CEMETERY,
    "{5}, {T}, Exile this land: Target creature's owner shuffles it into their library."
)]
#[case::any_number_of_cards(
    PERPETUAL_TIMEPIECE,
    "{2}, Exile this artifact: Shuffle any number of target cards from your graveyard into your library."
)]
fn a_shuffle_into_the_library_reads(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_draw(
    OBLATION,
    "The owner of target nonland permanent shuffles it into their library, then draws three cards."
)]
#[case::another_place(
    OBLATION,
    "The owner of target nonland permanent shuffles it into their graveyard, then draws two cards."
)]
#[case::the_controller(
    OBLATION,
    "The controller of target nonland permanent shuffles it into their library, then draws two cards."
)]
#[case::another_target(
    OMINOUS_CEMETERY,
    "{5}, {T}, Exile this land: Target artifact's owner shuffles it into their library."
)]
#[case::their_graveyard(
    PERPETUAL_TIMEPIECE,
    "{2}, Exile this artifact: Shuffle any number of target cards from their graveyard into your library."
)]
#[case::up_to_two(
    PERPETUAL_TIMEPIECE,
    "{2}, Exile this artifact: Shuffle up to two target cards from your graveyard into your library."
)]
fn another_owner_place_or_cards_do_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "{U}, Discard this card: Draw a card."
const MNEMONIC_SPHERE: &str = r#"{"Activated":{"cost":[{"Mana":"{U}"},"DiscardSelf"],"from":"Hand","effects":[{"Draw":{}}]}}"#;

/// "Whenever ~ or another Elf you control dies, you may pay {1} and 1 life. If you do, draw a card."
const MIARA: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},{"ZoneChange":{"what":["Other",{"Subtype":"Elf"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}}]},"effects":[{"Pay":{"cost":[{"Mana":"{1}"},{"PayLife":1}],"paid":[{"Draw":{}}]}}]}}"#;

/// "This creature gets +1/+1 for each poison counter your opponents have."
const MYCOSYNTH_FIEND: &str = r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":{"Counters":{"on":"Opponents","counter":"poison"}},"toughness":{"Counters":{"on":"Opponents","counter":"poison"}}}}]}}"#;

#[rstest]
#[case::discard_this_card(MNEMONIC_SPHERE, "{U}, Discard this card: Draw a card.")]
#[case::mana_and_life(
    MIARA,
    "Whenever ~ or another Elf you control dies, you may pay {1} and 1 life. If you do, draw a card."
)]
#[case::poison_of_the_opponents(
    MYCOSYNTH_FIEND,
    "This creature gets +1/+1 for each poison counter your opponents have."
)]
fn a_discard_cost_two_part_payment_and_poison_count_read(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::another_cost(MNEMONIC_SPHERE, "{U}, Sacrifice this card: Draw a card.")]
#[case::another_draw(MNEMONIC_SPHERE, "{U}, Discard this card: Draw two cards.")]
#[case::another_mana(
    MIARA,
    "Whenever ~ or another Elf you control dies, you may pay {2} and 1 life. If you do, draw a card."
)]
#[case::another_life(
    MIARA,
    "Whenever ~ or another Elf you control dies, you may pay {1} and 2 life. If you do, draw a card."
)]
#[case::another_bonus(
    MYCOSYNTH_FIEND,
    "This creature gets +2/+2 for each poison counter your opponents have."
)]
#[case::your_own_poison(
    MYCOSYNTH_FIEND,
    "This creature gets +1/+1 for each poison counter you have."
)]
fn another_cost_payment_or_count_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "Return all nonland permanents target player controls to their owner's hand."
const RIVERS_REBUKE: &str = r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Move":{"what":{"Each":[{"Not":{"Type":"Land"}},{"ControlledBy":{"Target":0}}]},"to":"Hand"}}]}}"#;

/// "Return all artifact and enchantment cards from all graveyards to the battlefield under their owners' control."
const OPEN_THE_VAULTS: &str = r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"In":"Graveyard"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]},"to":{"Battlefield":{}}}}]}}"#;

/// "Return target creature or land card from a graveyard to its owner's hand. You gain 6 life."
const PULSE_OF_MURASA: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Graveyard"},{"AnyOf":[{"Type":"Creature"},{"Type":"Land"}]}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}},{"GainLife":{"amount":6}}]}}"#;

#[rstest]
#[case::a_group_of_a_player(
    RIVERS_REBUKE,
    "Return all nonland permanents target player controls to their owner's hand."
)]
#[case::to_their_owners(
    OPEN_THE_VAULTS,
    "Return all artifact and enchantment cards from all graveyards to the battlefield under their owners' control."
)]
#[case::from_any_graveyard(
    PULSE_OF_MURASA,
    "Return target creature or land card from a graveyard to its owner's hand. You gain 6 life."
)]
fn returns_of_a_group_and_of_graveyard_cards_read(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::every_permanent(
    RIVERS_REBUKE,
    "Return all permanents target player controls to their owner's hand."
)]
#[case::another_place(
    RIVERS_REBUKE,
    "Return all nonland permanents target player controls to their owner's graveyard."
)]
#[case::another_kind_of_card(
    OPEN_THE_VAULTS,
    "Return all artifact and creature cards from all graveyards to the battlefield under their owners' control."
)]
#[case::under_your_control(
    OPEN_THE_VAULTS,
    "Return all artifact and enchantment cards from all graveyards to the battlefield under your control."
)]
#[case::your_graveyard(
    PULSE_OF_MURASA,
    "Return target creature or land card from your graveyard to its owner's hand. You gain 6 life."
)]
#[case::another_life(
    PULSE_OF_MURASA,
    "Return target creature or land card from a graveyard to its owner's hand. You gain 5 life."
)]
fn another_group_place_kind_or_life_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// "Put X +1/+1 counters on target creature you control. It deals damage equal to its power to each other creature."
const NOVA_FLAME: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":"X"}},{"DealDamage":{"source":{"Target":0},"to":{"Each":[{"Type":"Creature"},{"Not":{"Is":{"Target":0}}}]},"amount":{"Stat":{"of":{"Target":0},"stat":"Power"}}}}]}}"#;

/// "~ deals 3 damage to any target. If a creature is dealt damage this way, it gets +5/+0 until end of turn."
const PROVOKE_THE_TROLLS: &str = r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":3}},{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Type":"Creature"}]}},"then":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":5,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}]}}"#;

#[rstest]
#[case::damage_by_the_target(
    NOVA_FLAME,
    "Put X +1/+1 counters on target creature you control. It deals damage equal to its power to each other creature."
)]
#[case::a_creature_dealt_damage(
    PROVOKE_THE_TROLLS,
    "~ deals 3 damage to any target. If a creature is dealt damage this way, it gets +5/+0 until end of turn."
)]
fn a_last_sentence_about_the_target_reads(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::damage_to_one_group(
    NOVA_FLAME,
    "Put X +1/+1 counters on target creature you control. It deals damage equal to its power to each creature."
)]
#[case::another_kind(
    NOVA_FLAME,
    "Put X +1/+1 counters on target creature you control. It deals damage equal to its power to each other artifact."
)]
#[case::another_stat(
    NOVA_FLAME,
    "Put X +1/+1 counters on target creature you control. It deals damage equal to its toughness to each other creature."
)]
#[case::another_bonus(
    PROVOKE_THE_TROLLS,
    "~ deals 3 damage to any target. If a creature is dealt damage this way, it gets +4/+0 until end of turn."
)]
#[case::another_kind_dealt_damage(
    PROVOKE_THE_TROLLS,
    "~ deals 3 damage to any target. If a planeswalker is dealt damage this way, it gets +5/+0 until end of turn."
)]
#[case::another_duration(
    PROVOKE_THE_TROLLS,
    "~ deals 3 damage to any target. If a creature is dealt damage this way, it gets +5/+0 until your next turn."
)]
fn another_group_stat_bonus_or_kind_does_not_read(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}
