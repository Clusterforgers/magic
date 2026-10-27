//! Goal: the writers the fifth sample of the names A to F needed give the oracle wording of each twin with the card's
//! words put back, a look that keeps some cards, a return "instead", a draw "equal to the difference", a win at a number
//! of counters, an Aura's creature returned and the swapped phrases; a changed number, kind, player, zone, condition or
//! group never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn c5_af_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const C5_AF_ALESHA: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"},{"Compare":{"stat":"Power","op":"Le","to":2}}]}}],"effects":[{"Pay":{"cost":"{W/B}{W/B}","paid":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"tapped":true}}}},{"JoinAttack":{"what":{"Target":0}}}]}}]}}"#;
const C5_AF_BANQUET: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":{"Times":{"by":2,"of":"X"}}}}}}"#;
const C5_AF_BARRET: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"targets":[{"what":{"Object":[{"Subtype":"Equipment"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":1}},{"what":{"Object":[{"Subtype":"Rebel"},{"ControlledBy":"You"}]}}],"effects":[{"Attach":{"what":{"Target":0},"to":{"Target":1}}}]}}"#;
const C5_AF_PRETENDER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["Other",{"Type":"Creature"},{"ControlledBy":"You"},"ChosenSubtype"],"to":"Battlefield"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#;
const C5_AF_STRAY_SOULS: &str = r#"{"Activated":{"cost":"{5}{B}{B}","from":"Graveyard","effects":[{"Move":{"what":"This","to":{"Library":{"place":"Top"}}}}]}}"#;
const C5_AF_MINE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This","Untapped"],"to":"Battlefield"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Dwarf","colors":["Red"],"types":["Creature"],"subtypes":["Dwarf"],"power":1,"toughness":1}}}}]}}"#;
const C5_AF_DROWNER: &str = r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Subtype":"Eldrazi"},{"Subtype":"Scion"}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#;
const C5_AF_TAILS: &str = r#"{"Activated":{"cost":"{1}","targets":[{"what":{"Either":[{"Object":["Spell"]},{"Object":[]}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetColors":["White"]}],"duration":"UntilEndOfTurn"}}]}}"#;
const C5_AF_ETCHINGS: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},{"Sacrifice":{"filter":[{"Type":"Creature"},"ChosenSubtype"]}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}}"#;
const C5_AF_ROSS: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"ControlledBy":"Opponents"},{"AttackingPlayer":"You"}],"at_least":2}},"effects":[{"Draw":{}}]}}"#;
const C5_AF_CHALICE: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"charge","count":"Kicks"}}}}"#;
const C5_AF_CELESTINE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"},{"Compare":{"stat":"ManaValue","op":"Le","to":{"Tallied":{"who":"You","what":"LifeGainedTotal"}}}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}"#;
const C5_AF_SURVEYOR: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"Defending"}]},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}},"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Subtype":"Plains"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#;
const C5_AF_DORA: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Compare":{"left":{"Largest":{"who":"Opponents","of":[{"Type":"Land"}]}},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}},"effects":[{"CreateToken":{"token":{"Named":"vibranium"},"tapped":true}}]}}"#;
const C5_AF_SELL_SWORD: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"Compare":{"left":{"Stat":{"of":"This","stat":"Power"}},"op":"Ge","right":6}},"effects":[{"Draw":{}}]}}"#;
const C5_AF_CHOPPER: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Is":"Attached"}],"to":"EachPlayer","combat":true}},"effects":[{"May":{"effects":[{"Sacrifice":{"filter":[{"Is":"Dealer"}]}}]}},{"If":{"condition":"Performed","then":[{"Draw":{"count":{"Stat":{"of":"Dealer","stat":"Power"}}}}]}}]}}"#;
const C5_AF_FERTILID: &str = r#"{"Activated":{"cost":[{"Mana":"{1}{G}"},{"RemoveCounters":{"counter":"+1/+1"}}],"targets":[{"what":"Player"}],"effects":[{"Search":{"who":{"Target":0},"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{"who":{"Target":0}}}]}}"#;
const C5_AF_BROODLORD: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"bounds":{"min":0,"max":"X"},"divide":"X"}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":"Divided"}}]}}"#;
const C5_AF_OTTER: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":"X"},"divide":"X"}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":"Divided"}}]}}"#;
const C5_AF_HUSSAR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"LookAtTop":{"count":3,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"card","rest_as":"rest"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}]}}"#;
const C5_AF_DIG: &str = r#"{"Spell":{"effects":[{"LookAtTop":{"count":7,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"count":{"min":2,"max":2},"as":"cards","rest_as":"rest"}},{"Move":{"what":{"Bound":"cards"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}]}}"#;
const C5_AF_AERITH: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"if":{"Compare":{"left":{"Tallied":{"who":"You","what":"LifeGained"}},"op":"Gt","right":0}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"If":{"condition":{"Compare":{"left":{"Tallied":{"who":"You","what":"LifeGainedTotal"}},"op":"Ge","right":7}},"then":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}],"otherwise":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}]}}"#;
const C5_AF_DAMIA: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Lt","right":7}},"effects":[{"Draw":{"count":{"Sum":[7,{"Times":{"by":-1,"of":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}]}}}]}}"#;
const C5_AF_REACTOR: &str = r#"{"Triggered":{"when":{"State":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":"charge"}},"op":"Ge","right":20}}}},"effects":[{"WinGame":{}}]}}"#;
const C5_AF_LOYALTY: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{"under":"You"}}}}]}}"#;
const C5_AF_BRALLIN: &str = r#"{"Triggered":{"when":{"Discarded":{"by":"You"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}},{"DealDamage":{"to":"Opponents","amount":1}}]}}"#;
const C5_AF_CAZUR: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}},"effects":[{"PutCounters":{"on":"Dealer","counter":"+1/+1"}}]}}"#;
const C5_AF_LIEGE: &str = r#"{"Triggered":{"when":{"SpellCast":{"by":"You","what":[{"Color":"Black"}]}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":["Tapped"]}},"then":[{"May":{"effects":[{"Destroy":{"what":{"Target":0}}}]}}]}}]}}"#;
const C5_AF_INVESTIGATOR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"CreateToken":{"token":{"Named":"clue"},"count":{"Players":"Opponents"}}}]}}"#;
const C5_AF_COVENANT: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":"X"},"divide":"X"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#;
const C5_AF_AJANI: &str = r#"{"Loyalty":{"cost":{"Minus":9},"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1","count":5}},{"PutCounters":{"on":{"Each":[{"Type":"Planeswalker"},"Other",{"ControlledBy":"You"}]},"counter":"loyalty","count":5}}]}}"#;
const C5_AF_ALHARU: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},"Other"]},"bounds":{"min":0,"max":2}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#;
const C5_AF_PIZZA: &str = r#"{"Activated":{"cost":[{"Mana":"{2}{W}{U}{B}{R}{G}"},"Tap","SacrificeSelf"],"targets":[{"what":"Player"},{"what":"AnyTarget"},{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}}],"effects":[{"GainLife":{"who":{"Target":0},"amount":3}},{"Draw":{"who":{"Target":0}}},{"Discard":{"who":"Opponents"}},{"DealDamage":{"to":{"Target":1},"amount":3}},{"PutCounters":{"on":{"Target":2},"counter":"+1/+1","count":3}}]}}"#;
const C5_AF_EXPERIMENT: &str = r#"{"Activated":{"cost":[{"RemoveCounters":{"counter":"+1/+1","count":2}}],"effects":[{"Regenerate":{"what":"This"}}]}}"#;
const C5_AF_BADGER: &str = r#"{"Static":{"affected":{"Each":[{"AnyOf":[{"Subtype":"Fungus"},{"Subtype":"Saproling"}]},{"ControlledBy":"You"}]},"mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}}]}}"#;
const C5_AF_DINA: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap",{"Sacrifice":{"filter":[{"Type":"Creature"},"Other"]}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"GainLife":{"amount":{"Stat":{"of":"Paid","stat":"Power"}}}},{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"Stat":{"of":"Paid","stat":"Power"}}}}]}}"#;
const C5_AF_DROGSKOL: &str = r#"{"Replacement":{"when":{"Damage":{"to":{"Each":[{"Subtype":"Spirit"},{"ControlledBy":"You"}]},"combat":false}},"by":"Prevent"}}"#;
const C5_AF_CHAUFFEUR: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":{"Sum":[1,{"Count":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]}]}}}}}"#;
const C5_AF_DRAGONSTORM: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Subtype":"Dragon"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#;

#[rstest]
#[case::tapped_and_attacking(
    C5_AF_ALESHA,
    "Whenever ~ attacks, you may pay {W/B}{W/B}. If you do, return target creature card with power 2 or less from your graveyard to the battlefield tapped and attacking."
)]
#[case::twice_x(
    C5_AF_BANQUET,
    "This creature enters with twice X +1/+1 counters on it."
)]
#[case::up_to_one_attached(
    C5_AF_BARRET,
    "At the beginning of combat on your turn, attach up to one target Equipment you control to target Rebel you control."
)]
#[case::of_the_chosen_type(
    C5_AF_PRETENDER,
    "Whenever another creature you control of the chosen type enters, put a +1/+1 counter on this creature."
)]
#[case::on_top_from_graveyard(
    C5_AF_STRAY_SOULS,
    "{5}{B}{B}: Put this card from your graveyard on top of your library."
)]
#[case::enters_untapped(
    C5_AF_MINE,
    "When this land enters untapped, create a 1/1 red Dwarf creature token."
)]
#[case::two_subtypes_sacrificed(C5_AF_DROWNER, "Sacrifice an Eldrazi Scion: Tap target creature.")]
#[case::spell_or_permanent(
    C5_AF_TAILS,
    "{1}: Target spell or permanent becomes white until end of turn."
)]
#[case::sacrifice_of_chosen_type(
    C5_AF_ETCHINGS,
    "{1}, Sacrifice a creature of the chosen type: Target creature you control gains indestructible until end of turn."
)]
#[case::two_or_more_attackers(
    C5_AF_ROSS,
    "Whenever an opponent attacks you with two or more creatures, draw a card."
)]
#[case::each_time_kicked(
    C5_AF_CHALICE,
    "This artifact enters with a charge counter on it for each time it was kicked."
)]
#[case::where_x_is_life_gained(
    C5_AF_CELESTINE,
    "At the beginning of your end step, return target creature card with mana value X or less from your graveyard to the battlefield, where X is the amount of life you gained this turn."
)]
#[case::defending_lands(
    C5_AF_SURVEYOR,
    "Whenever this Vehicle attacks, if defending player controls more lands than you, search your library for a basic Plains card, put it onto the battlefield tapped, then shuffle."
)]
#[case::opponent_lands(
    C5_AF_DORA,
    "When this creature enters, if an opponent controls more lands than you, create a tapped Vibranium token."
)]
#[case::own_power_at_least(
    C5_AF_SELL_SWORD,
    "Whenever this creature attacks, if its power is 6 or greater, draw a card."
)]
#[case::sacrifice_equipped(
    C5_AF_CHOPPER,
    "Whenever equipped creature deals combat damage to a player, you may sacrifice it. If you do, draw cards equal to its power."
)]
#[case::target_player_searches(
    C5_AF_FERTILID,
    "{1}{G}, Remove a +1/+1 counter from this creature: Target player searches their library for a basic land card, puts it onto the battlefield tapped, then shuffles."
)]
#[case::distribute_among_others(
    C5_AF_BROODLORD,
    "When this creature enters, distribute X +1/+1 counters among any number of other target creatures you control."
)]
#[case::distribute_among_any(
    C5_AF_OTTER,
    "Distribute X +1/+1 counters among any number of target creatures you control."
)]
#[case::keep_one_of_three(
    C5_AF_HUSSAR,
    "When this creature enters, look at the top three cards of your library, then put one of them into your hand and the rest on the bottom of your library in any order."
)]
#[case::keep_two_of_seven(
    C5_AF_DIG,
    "Look at the top seven cards of your library. Put two of them into your hand and the rest on the bottom of your library in any order."
)]
#[case::battlefield_instead(
    C5_AF_AERITH,
    "At the beginning of your end step, if you gained life this turn, return target creature card from your graveyard to your hand. If you gained 7 or more life this turn, return that card to the battlefield instead."
)]
#[case::draw_the_difference(
    C5_AF_DAMIA,
    "At the beginning of your upkeep, if you have fewer than seven cards in hand, draw cards equal to the difference."
)]
#[case::win_at_counters(
    C5_AF_REACTOR,
    "When this artifact has twenty or more charge counters on it, you win the game."
)]
#[case::enchanted_returns(
    C5_AF_LOYALTY,
    "When enchanted creature dies, return it to the battlefield under your control."
)]
#[case::and_it_deals(
    C5_AF_BRALLIN,
    "Whenever you discard a card, put a +1/+1 counter on ~ and it deals 1 damage to each opponent."
)]
#[case::on_that_creature(
    C5_AF_CAZUR,
    "Whenever a creature you control deals combat damage to a player, put a +1/+1 counter on that creature."
)]
#[case::if_it_is_tapped(
    C5_AF_LIEGE,
    "Whenever you cast a black spell, you may destroy target creature if it's tapped."
)]
#[case::investigate_x_times(
    C5_AF_INVESTIGATOR,
    "When this creature enters, investigate X times, where X is the number of opponents you have."
)]
#[case::any_number_divided(
    C5_AF_COVENANT,
    "~ deals X damage divided as you choose among any number of target creatures."
)]
#[case::and_counters_on_others(
    C5_AF_AJANI,
    "−9: Put five +1/+1 counters on each creature you control and five loyalty counters on each other planeswalker you control."
)]
#[case::other_target(
    C5_AF_ALHARU,
    "When ~ enters, put a +1/+1 counter on each of up to two other target creatures."
)]
#[case::each_of_your_opponents(
    C5_AF_PIZZA,
    "{2}{W}{U}{B}{R}{G}, {T}, Sacrifice this artifact: Target player gains 3 life and draws a card. Each of your opponents discards a card. This artifact deals 3 damage to any target. Put three +1/+1 counters on up to one target creature."
)]
#[case::regenerate_it(
    C5_AF_EXPERIMENT,
    "Remove two +1/+1 counters from this creature: Regenerate it."
)]
#[case::each_and(
    C5_AF_BADGER,
    "Each Fungus and Saproling you control has \"{T}: Add {G}.\""
)]
#[case::sacrificed_power(
    C5_AF_DINA,
    "{2}, {T}, Sacrifice another creature: You gain X life and put X +1/+1 counters on target creature you control, where X is the sacrificed creature's power."
)]
#[case::prevent_all_noncombat(
    C5_AF_DROGSKOL,
    "Prevent all noncombat damage that would be dealt to Spirits you control."
)]
#[case::a_number_equal_to(
    C5_AF_CHAUFFEUR,
    "This creature enters with a number of +1/+1 counters on it equal to one plus the number of other creatures you control."
)]
#[case::when_it_returns(
    C5_AF_DRAGONSTORM,
    "When a Dragon you control enters, return this enchantment to its owner's hand."
)]
fn c5_af_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        c5_af_written(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::not_attacking(C5_AF_ALESHA.replace(r#",{"JoinAttack":{"what":{"Target":0}}}"#, ""), "Whenever ~ attacks, you may pay {W/B}{W/B}. If you do, return target creature card with power 2 or less from your graveyard to the battlefield tapped and attacking.")]
#[case::thrice_x(C5_AF_BANQUET.replace(r#""by":2"#, r#""by":3"#), "This creature enters with twice X +1/+1 counters on it.")]
#[case::exactly_one_attached(C5_AF_BARRET.replace(r#","bounds":{"min":0,"max":1}"#, ""), "At the beginning of combat on your turn, attach up to one target Equipment you control to target Rebel you control.")]
#[case::any_type(C5_AF_PRETENDER.replace(r#","ChosenSubtype""#, ""), "Whenever another creature you control of the chosen type enters, put a +1/+1 counter on this creature.")]
#[case::to_the_bottom(C5_AF_STRAY_SOULS.replace(r#""place":"Top""#, r#""place":"Bottom""#), "{5}{B}{B}: Put this card from your graveyard on top of your library.")]
#[case::enters_tapped(C5_AF_MINE.replace(r#""Untapped""#, r#""Tapped""#), "When this land enters untapped, create a 1/1 red Dwarf creature token.")]
#[case::other_subtype(C5_AF_DROWNER.replace(r#""Eldrazi""#, r#""Elf""#), "Sacrifice an Eldrazi Scion: Tap target creature.")]
#[case::becomes_black(C5_AF_TAILS.replace(r#""White""#, r#""Black""#), "{1}: Target spell or permanent becomes white until end of turn.")]
#[case::three_attackers(C5_AF_ROSS.replace(r#""at_least":2"#, r#""at_least":3"#), "Whenever an opponent attacks you with two or more creatures, draw a card.")]
#[case::loyalty_per_kick(C5_AF_CHALICE.replace(r#""charge""#, r#""loyalty""#), "This artifact enters with a charge counter on it for each time it was kicked.")]
#[case::life_lost(C5_AF_CELESTINE.replace("LifeGainedTotal", "LifeLost"), "At the beginning of your end step, return target creature card with mana value X or less from your graveyard to the battlefield, where X is the amount of life you gained this turn.")]
#[case::fewer_lands(C5_AF_SURVEYOR.replace(r#""op":"Gt""#, r#""op":"Lt""#), "Whenever this Vehicle attacks, if defending player controls more lands than you, search your library for a basic Plains card, put it onto the battlefield tapped, then shuffle.")]
#[case::opponent_creatures(C5_AF_DORA.replace(r#""of":[{"Type":"Land"}]"#, r#""of":[{"Type":"Creature"}]"#), "When this creature enters, if an opponent controls more lands than you, create a tapped Vibranium token.")]
#[case::power_five(C5_AF_SELL_SWORD.replace(r#""right":6"#, r#""right":5"#), "Whenever this creature attacks, if its power is 6 or greater, draw a card.")]
#[case::draw_by_toughness(C5_AF_CHOPPER.replace(r#""stat":"Power""#, r#""stat":"Toughness""#), "Whenever equipped creature deals combat damage to a player, you may sacrifice it. If you do, draw cards equal to its power.")]
#[case::untapped_land(C5_AF_FERTILID.replace(r#""tapped":true"#, r#""tapped":false"#), "{1}{G}, Remove a +1/+1 counter from this creature: Target player searches their library for a basic land card, puts it onto the battlefield tapped, then shuffles.")]
#[case::not_divided(C5_AF_OTTER.replace(r#","divide":"X""#, ""), "Distribute X +1/+1 counters among any number of target creatures you control.")]
#[case::random_order(C5_AF_DIG.replace(r#""order":"Chosen""#, r#""order":"Random""#), "Look at the top seven cards of your library. Put two of them into your hand and the rest on the bottom of your library in any order.")]
#[case::keep_three(C5_AF_DIG.replace(r#""min":2,"max":2"#, r#""min":3,"max":3"#), "Look at the top seven cards of your library. Put two of them into your hand and the rest on the bottom of your library in any order.")]
#[case::more_life(C5_AF_AERITH.replace(r#""right":7"#, r#""right":8"#), "At the beginning of your end step, if you gained life this turn, return target creature card from your graveyard to your hand. If you gained 7 or more life this turn, return that card to the battlefield instead.")]
#[case::hand_of_six(C5_AF_DAMIA.replace(r#""right":7"#, r#""right":6"#), "At the beginning of your upkeep, if you have fewer than seven cards in hand, draw cards equal to the difference.")]
#[case::loyalty_counters(C5_AF_REACTOR.replace(r#""charge""#, r#""loyalty""#), "When this artifact has twenty or more charge counters on it, you win the game.")]
#[case::to_the_hand(C5_AF_LOYALTY.replace(r#"{"Battlefield":{"under":"You"}}"#, r#""Hand""#), "When enchanted creature dies, return it to the battlefield under your control.")]
#[case::damage_to_you(C5_AF_BRALLIN.replace(r#""to":"Opponents""#, r#""to":"You""#), "Whenever you discard a card, put a +1/+1 counter on ~ and it deals 1 damage to each opponent.")]
#[case::counter_on_self(C5_AF_CAZUR.replace(r#""on":"Dealer""#, r#""on":"This""#), "Whenever a creature you control deals combat damage to a player, put a +1/+1 counter on that creature.")]
#[case::if_untapped(C5_AF_LIEGE.replace(r#"["Tapped"]"#, r#"["Untapped"]"#), "Whenever you cast a black spell, you may destroy target creature if it's tapped.")]
#[case::clue_per_player(C5_AF_INVESTIGATOR.replace(r#""Players":"Opponents""#, r#""Players":"EachPlayer""#), "When this creature enters, investigate X times, where X is the number of opponents you have.")]
#[case::other_owner(C5_AF_ALHARU.replace(r#","Other""#, ""), "When ~ enters, put a +1/+1 counter on each of up to two other target creatures.")]
#[case::each_player_discards(C5_AF_PIZZA.replace(r#""Discard":{"who":"Opponents"}"#, r#""Discard":{"who":"EachPlayer"}"#), "{2}{W}{U}{B}{R}{G}, {T}, Sacrifice this artifact: Target player gains 3 life and draws a card. Each of your opponents discards a card. This artifact deals 3 damage to any target. Put three +1/+1 counters on up to one target creature.")]
#[case::regenerate_after_mana(C5_AF_EXPERIMENT.replace(r#"{"RemoveCounters":{"counter":"+1/+1","count":2}}"#, r#"{"Mana":"{2}"}"#), "{2}: Regenerate it.")]
#[case::revealed_power(C5_AF_DINA.replace(r#"{"Sacrifice":{"filter":[{"Type":"Creature"},"Other"]}}"#, r#"{"Mana":"{1}"}"#), "{2}, {T}, {1}: You gain X life and put X +1/+1 counters on target creature you control, where X is the sacrificed creature's power.")]
#[case::combat_damage(C5_AF_DROGSKOL.replace(r#""combat":false"#, r#""combat":true"#), "Prevent all noncombat damage that would be dealt to Spirits you control.")]
#[case::self_returns_each_time(C5_AF_DRAGONSTORM.replace(r#"{"Move":{"what":"This","to":"Hand"}}"#, r#"{"Draw":{}}"#), "When a Dragon you control enters, draw a card.")]
fn c5_af_near_misses_never_read_back(#[case] json: String, #[case] clause: &str) {
    assert!(!c5_af_written(&json, clause), "{clause}");
}
