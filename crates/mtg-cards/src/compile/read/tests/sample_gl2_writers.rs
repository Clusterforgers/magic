//! Goal: the writers the second sample of the names G to L needed give the oracle wording of tokens counted by the
//! creatures that died, a restriction with "unless", counters on the creature of an event and the card, a target chosen
//! before the counters, a pumped group that is untapped, a sacrifice avoided by returning a permanent, the top cards put
//! back, two groups destroyed, targets exiled and returned, each player's poison counter, "X plus N" damage, a madness
//! condition and a payment of mana and a sacrifice; a changed kind, count, group, place, player or cost never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const NO_ATTACK: &str = r#"{"Static":{"affected":"This","mods":["CantAttack"],"condition":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"op":"Lt","right":4}}}}"#;
const TREASURES: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"CreateToken":{"token":{"Named":"treasure"},"count":{"Tallied":{"who":"EachPlayer","what":"NontokenCreaturesDied"}}}}]}}"#;
const COUNTER_PAIR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"Triggering","counter":"+1/+1"}},{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#;
const STUN: &str = r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"Opponents","at":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"AttackingPlayer":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":{"Other":"stun"}}}]}}"#;
const PUMP_UNTAP: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":{"Target":0}}]},"mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}},{"Untap":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":{"Target":0}}]}}}]}}"#;
const BASIN: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Pay":{"cost":[{"ReturnToHand":{"filter":[{"Subtype":"Forest"},"Untapped"]}}],"unpaid":[{"Sacrifice":{"filter":["This"]}}]}}]}}"#;
const PUT_BACK: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"LookAtTop":{"count":3,"as":"top"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Top","order":"Chosen"}}}}]}}"#;
const WAKE: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}},{"Destroy":{"what":{"Each":[{"Type":"Planeswalker"},{"Not":{"ControlledBy":"You"}}]}}}]}}"#;
const FLICKER: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Land"}]},{"ControlledBy":"You"}]},"bounds":{"min":2,"max":2}}],"effects":[{"Exile":{"what":{"Target":0},"as":"exiled"}},{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{"under":"You"}}}}]}}"#;
const RATS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"EachPlayer","counter":"poison"}}]}}"#;
const BURN: &str = r#"{"Spell":{"effects":[{"DealDamage":{"source":"This","to":{"Each":[{"Type":"Creature"},{"Not":"EnteredThisTurn"}]},"amount":{"Sum":["X",2]}}}]}}"#;
const MADNESS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"CastWith":"Madness"},"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"effects":[{"May":{"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}]}}"#;
const SERVICE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Pay":{"cost":[{"Mana":"{2}"},{"Sacrifice":{"filter":["Token"]}}],"paid":[{"CreateToken":{"token":{"Custom":{"name":"Rhino Warrior","colors":["Green"],"types":["Creature"],"subtypes":["Rhino","Warrior"],"power":4,"toughness":4}}}}]}}]}}"#;

#[rstest]
#[case::unless_artifacts(NO_ATTACK, "~ can't attack unless you control four or more artifacts.")]
#[case::for_each_died(
    TREASURES,
    "At the beginning of your end step, create a Treasure token for each nontoken creature that died this turn."
)]
#[case::died_as_a_count(
    TREASURES,
    "At the beginning of your end step, create X Treasure tokens, where X is the number of nontoken creatures that died this turn."
)]
#[case::counters_on_two(
    COUNTER_PAIR,
    "Whenever another creature you control enters, put a +1/+1 counter on that creature and a +1/+1 counter on ~."
)]
#[case::counters_in_two_sentences(
    COUNTER_PAIR,
    "Whenever another creature you control enters, put a +1/+1 counter on it. Put a +1/+1 counter on ~."
)]
#[case::choose_then_counter(
    STUN,
    "Whenever an opponent attacks you, choose target creature attacking you. Put a stun counter on that creature."
)]
#[case::group_pumped_and_untapped(
    PUMP_UNTAP,
    "When ~ enters, creatures target player controls get +2/+2 until end of turn. Untap them."
)]
#[case::sacrifice_it_unless(
    BASIN,
    "When ~ enters, sacrifice it unless you return an untapped Forest you control to its owner's hand."
)]
#[case::put_back(
    PUT_BACK,
    "When ~ enters, look at the top three cards of your library, then put them back in any order."
)]
#[case::two_groups_destroyed(
    WAKE,
    "Destroy all creatures you don't control and all planeswalkers you don't control."
)]
#[case::two_groups_in_two_sentences(
    WAKE,
    "Destroy all creatures you don't control. Destroy each planeswalker you don't control."
)]
#[case::targets_flickered(
    FLICKER,
    "Exile two target artifacts, creatures, and/or lands you control, then return those cards to the battlefield under your control."
)]
#[case::each_player_poisoned(RATS, "When ~ enters, each player gets a poison counter.")]
#[case::x_plus_two(
    BURN,
    "~ deals X plus 2 damage to each creature that didn't enter this turn."
)]
#[case::madness_paid(
    MADNESS,
    "When ~ enters, if its madness cost was paid, you may return target creature card from a graveyard to its owner's hand."
)]
#[case::pay_and_sacrifice(
    SERVICE,
    "At the beginning of your end step, you may pay {2} and sacrifice a token. If you do, create a 4/4 green Rhino Warrior creature token."
)]
fn the_second_sample_wordings_are_written_as_the_oracle_words_them(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::more_artifacts(NO_ATTACK, "~ can't attack unless you control five or more artifacts.")]
#[case::block_for_attack(NO_ATTACK, "~ can't block unless you control four or more artifacts.")]
#[case::creatures_for_artifacts(
    NO_ATTACK,
    "~ can't attack unless you control four or more creatures."
)]
#[case::every_creature(
    TREASURES,
    "At the beginning of your end step, create a Treasure token for each creature that died this turn."
)]
#[case::another_token(
    TREASURES,
    "At the beginning of your end step, create a Clue token for each nontoken creature that died this turn."
)]
#[case::each_end_step(
    TREASURES,
    "At the beginning of each end step, create a Treasure token for each nontoken creature that died this turn."
)]
#[case::two_counters_on_the_card(
    COUNTER_PAIR,
    "Whenever another creature you control enters, put a +1/+1 counter on that creature and two +1/+1 counters on ~."
)]
#[case::the_card_first(
    COUNTER_PAIR,
    "Whenever another creature you control enters, put a +1/+1 counter on ~ and a +1/+1 counter on that creature."
)]
#[case::any_creature(
    COUNTER_PAIR,
    "Whenever a creature you control enters, put a +1/+1 counter on that creature and a +1/+1 counter on ~."
)]
#[case::other_counter(
    STUN,
    "Whenever an opponent attacks you, choose target creature attacking you. Put a +1/+1 counter on that creature."
)]
#[case::attacking_an_opponent(
    STUN,
    "Whenever an opponent attacks you, choose target creature attacking an opponent. Put a stun counter on that creature."
)]
#[case::more_power(
    PUMP_UNTAP,
    "When ~ enters, creatures target player controls get +3/+3 until end of turn. Untap them."
)]
#[case::your_creatures(
    PUMP_UNTAP,
    "When ~ enters, creatures you control get +2/+2 until end of turn. Untap them."
)]
#[case::tap_for_untap(
    PUMP_UNTAP,
    "When ~ enters, creatures target player controls get +2/+2 until end of turn. Tap them."
)]
#[case::tapped_forest(
    BASIN,
    "When ~ enters, sacrifice it unless you return a tapped Forest you control to its owner's hand."
)]
#[case::island(
    BASIN,
    "When ~ enters, sacrifice it unless you return an untapped Island you control to its owner's hand."
)]
#[case::four_cards(
    PUT_BACK,
    "When ~ enters, look at the top four cards of your library, then put them back in any order."
)]
#[case::on_the_bottom(
    PUT_BACK,
    "When ~ enters, look at the top three cards of your library, then put them on the bottom of your library in any order."
)]
#[case::own_creatures_destroyed(
    WAKE,
    "Destroy all creatures you control and all planeswalkers you don't control."
)]
#[case::artifacts_destroyed(
    WAKE,
    "Destroy all creatures you don't control and all artifacts you don't control."
)]
#[case::three_targets(
    FLICKER,
    "Exile three target artifacts, creatures, and/or lands you control, then return those cards to the battlefield under your control."
)]
#[case::one_card(
    FLICKER,
    "Exile two target artifacts, creatures, and/or lands you control, then return that card to the battlefield under your control."
)]
#[case::owners_control(
    FLICKER,
    "Exile two target artifacts, creatures, and/or lands you control, then return those cards to the battlefield under their owners' control."
)]
#[case::each_opponent_poisoned(RATS, "When ~ enters, each opponent gets a poison counter.")]
#[case::two_poison(RATS, "When ~ enters, each player gets two poison counters.")]
#[case::plus_three(
    BURN,
    "~ deals X plus 3 damage to each creature that didn't enter this turn."
)]
#[case::entered_this_turn(
    BURN,
    "~ deals X plus 2 damage to each creature that entered this turn."
)]
#[case::another_cost_was_paid(
    MADNESS,
    "When ~ enters, if its prowl cost was paid, you may return target creature card from a graveyard to its owner's hand."
)]
#[case::to_your_hand(
    MADNESS,
    "When ~ enters, if its madness cost was paid, you may return target creature card from a graveyard to your hand."
)]
#[case::less_mana(
    SERVICE,
    "At the beginning of your end step, you may pay {1} and sacrifice a token. If you do, create a 4/4 green Rhino Warrior creature token."
)]
#[case::sacrifice_a_creature(
    SERVICE,
    "At the beginning of your end step, you may pay {2} and sacrifice a creature. If you do, create a 4/4 green Rhino Warrior creature token."
)]
#[case::a_smaller_token(
    SERVICE,
    "At the beginning of your end step, you may pay {2} and sacrifice a token. If you do, create a 3/3 green Rhino Warrior creature token."
)]
fn a_changed_kind_count_group_place_player_or_cost_does_not_read_back(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}
