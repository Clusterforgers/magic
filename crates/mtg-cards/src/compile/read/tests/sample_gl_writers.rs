//! Goal: the writers the sample of the names G to L needed give the oracle wording of a sweep with "each" and a
//! singular noun, a creature "that isn't all colors", the card as "it" after its own tap or block, "each other player"
//! with "of their choice", "enters or is put into a graveyard from the battlefield", an untapped card in a step's "if",
//! a change of control, a token made with the Equipment of the sentence before attached, an exile undone at once or at the
//! end step, a search whose finds go two ways, cards of the graveyard returned to the hand, a mill with the optional
//! return, cards shuffled into a library and a mill by a count of "that player's graveyard"; a changed verb,
//! group, count, place, player or duration never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const GAZE: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}]}}}]}}"#;
const IRIDIAN: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Colors","op":"Lt","to":5}}]}}}]}}"#;
const MEDICS: &str = r#"{"Triggered":{"when":{"Tapped":{"what":["This"]}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#;
const GRAVE_PACT: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Sacrifice":{"who":"Opponents","filter":[{"Type":"Creature"}]}}]}}"#;
const ICHOR: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}]},"effects":[{"Draw":{"count":1}}]}}"#;
const HOWLING: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw"}},"if":{"Matches":{"subject":"This","selector":["Untapped"]}},"effects":[{"Draw":{"who":"Triggering","count":1}}]}}"#;
const INSURRECTION: &str = r#"{"Spell":{"effects":[{"Untap":{"what":{"Each":[{"Type":"Creature"}]}}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"SetController":"You"},{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}"#;
const CADETS: &str = r#"{"Triggered":{"when":{"AnyOf":[{"Blocks":{"what":["This"]}},{"BecomesBlocked":{"what":["This"]}}]},"targets":[{"what":"Opponent"}],"effects":[{"Continuous":{"affected":"This","mods":[{"SetController":{"Target":0}}],"duration":"Forever"}}]}}"#;
const KODAMA: &str = r#"{"Spell":{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"count":{"min":0,"max":2},"reveal":true,"as":"found"}},{"Choose":{"from":{"Bound":"found"},"as":"battlefield","rest_as":"hand"}},{"Move":{"what":{"Bound":"battlefield"},"to":{"Battlefield":{"tapped":true}}}},{"Move":{"what":{"Bound":"hand"},"to":"Hand"}},{"Shuffle":{}}]}}"#;
const LYCHGUARD: &str = r#"{"Activated":{"cost":[{"Mana":"{3}{B}"},"SacrificeSelf"],"effects":[{"Move":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Supertype":"Legendary"},{"Type":"Creature"}]},"to":"Hand"}}]}}"#;
const GRAPPLE: &str = r#"{"Spell":{"effects":[{"Mill":{"count":3}},{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"},{"AnyOf":[{"Type":"Creature"},{"Type":"Land"}]}]},"as":"returned"}},{"Move":{"what":{"Bound":"returned"},"to":"Hand"}}]}}]}}"#;
const LOAMING: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"},{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]},"bounds":{"min":0,"max":null}}],"effects":[{"Move":{"what":{"Target":1},"to":{"Library":{"place":"Shuffled"}}}}]}}"#;
const KEENING: &str = r#"{"Activated":{"cost":[{"Mana":"{5}"},"Tap"],"targets":[{"what":"Player"}],"effects":[{"Mill":{"who":{"Target":0},"count":{"Count":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}}}]}}"#;
const THREATEN: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}}]}}"#;
const GRIP: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Subtype":"Equipment"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"Forever"}},{"CreateToken":{"token":{"Custom":{"name":"Phyrexian Germ","colors":["Black"],"types":["Creature"],"subtypes":["Phyrexian","Germ"],"power":0,"toughness":0}},"as":"tokens"}},{"Attach":{"what":{"Target":0},"to":{"Bound":"tokens"}}}]}}"#;
const GILRAEN: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]}}],"effects":[{"Exile":{"what":{"Target":0},"as":"exiled"}},{"May":{"effects":[{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{}}}}]}},{"If":{"condition":{"Not":"Performed"},"then":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{"counters":[[{"Other":"vigilance"},1],[{"Other":"lifelink"},1]]}}}}]}}]}}]}}"#;
const PERMANENT_CONTROL: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"Forever"}}]}}"#;

#[rstest]
#[case::each_nonland_permanent(GAZE, "Destroy each nonland permanent with mana value X or less.")]
#[case::all_is_still_written(GAZE, "Destroy all nonland permanents with mana value X or less.")]
#[case::not_all_colors(IRIDIAN, "Destroy each creature that isn't all colors.")]
#[case::own_tap_as_it(MEDICS, "Whenever ~ becomes tapped, it deals 1 damage to any target.")]
#[case::own_tap_named(MEDICS, "Whenever ~ becomes tapped, ~ deals 1 damage to any target.")]
#[case::other_players_choice(
    GRAVE_PACT,
    "Whenever a creature you control dies, each other player sacrifices a creature of their choice."
)]
#[case::opponents(
    GRAVE_PACT,
    "Whenever a creature you control dies, each opponent sacrifices a creature."
)]
#[case::put_into_a_graveyard(
    ICHOR,
    "When ~ enters or is put into a graveyard from the battlefield, draw a card."
)]
#[case::dies(ICHOR, "When ~ enters or dies, draw a card.")]
#[case::untapped_card_in_a_step(
    HOWLING,
    "At the beginning of each player's draw step, if ~ is untapped, that player draws an additional card."
)]
#[case::group_control(
    INSURRECTION,
    "Untap all creatures and gain control of them until end of turn. They gain haste until end of turn."
)]
#[case::control_of_the_card(
    CADETS,
    "Whenever ~ blocks or becomes blocked, target opponent gains control of it."
)]
#[case::control_for_the_turn(THREATEN, "Gain control of target creature until end of turn.")]
#[case::control_for_good(PERMANENT_CONTROL, "Gain control of target artifact.")]
#[case::token_with_the_equipment(
    GRIP,
    "Gain control of target Equipment, then create a 0/0 black Phyrexian Germ creature token and attach that Equipment to it."
)]
#[case::undone_at_once_or_at_the_end_step(
    GILRAEN,
    "{2}, {T}: Exile another target creature you control. You may return that card to the battlefield under its owner's control. If you don't, at the beginning of the next end step, return that card to the battlefield under its owner's control with a vigilance counter and a lifelink counter on it."
)]
#[case::search_two_ways(
    KODAMA,
    "Search your library for up to two basic land cards, reveal those cards, put one onto the battlefield tapped and the other into your hand, then shuffle."
)]
#[case::all_cards_of_a_graveyard(
    LYCHGUARD,
    "{3}{B}, Sacrifice ~: Return all legendary creature cards from your graveyard to your hand."
)]
#[case::mill_then_may_return(
    GRAPPLE,
    "Mill three cards, then you may return a creature or land card from your graveyard to your hand."
)]
#[case::their_graveyard_shuffled(
    LOAMING,
    "When ~ enters, target player shuffles any number of target cards from their graveyard into their library."
)]
#[case::that_players_graveyard(
    KEENING,
    "{5}, {T}: Target player mills X cards, where X is the number of cards in that player's graveyard."
)]
fn the_sample_wordings_are_written_as_the_oracle_words_them(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::a_permanent_of_any_kind(GAZE, "Destroy each permanent with mana value X or less.")]
#[case::more_value(GAZE, "Destroy each nonland permanent with mana value X or more.")]
#[case::exile_for_destroy(GAZE, "Exile each nonland permanent with mana value X or less.")]
#[case::no_colors_limit(IRIDIAN, "Destroy each creature.")]
#[case::another_type(IRIDIAN, "Destroy each artifact that isn't all colors.")]
#[case::untap_for_tap(
    MEDICS,
    "Whenever ~ becomes untapped, it deals 1 damage to any target."
)]
#[case::other_damage(MEDICS, "Whenever ~ becomes tapped, it deals 2 damage to any target.")]
#[case::two_creatures(
    GRAVE_PACT,
    "Whenever a creature you control dies, each other player sacrifices two creatures of their choice."
)]
#[case::you_sacrifice(
    GRAVE_PACT,
    "Whenever a creature you control dies, you sacrifice a creature of your choice."
)]
#[case::anywhere(
    ICHOR,
    "When ~ enters or is put into a graveyard from anywhere, draw a card."
)]
#[case::two_cards(
    ICHOR,
    "When ~ enters or is put into a graveyard from the battlefield, draw two cards."
)]
#[case::tapped_card(
    HOWLING,
    "At the beginning of each player's draw step, if ~ is tapped, that player draws an additional card."
)]
#[case::your_draw_step(
    HOWLING,
    "At the beginning of your draw step, if ~ is untapped, you draw an additional card."
)]
#[case::until_your_next_turn(
    INSURRECTION,
    "Untap all creatures and gain control of them until your next turn. They gain haste until end of turn."
)]
#[case::other_keyword(
    INSURRECTION,
    "Untap all creatures and gain control of them until end of turn. They gain flying until end of turn."
)]
#[case::other_group(
    INSURRECTION,
    "Untap all lands and gain control of them until end of turn. They gain haste until end of turn."
)]
#[case::any_player(
    CADETS,
    "Whenever ~ blocks or becomes blocked, target player gains control of it."
)]
#[case::only_blocking(CADETS, "Whenever ~ blocks, target opponent gains control of it.")]
#[case::for_the_turn_only(THREATEN, "Gain control of target creature.")]
#[case::another_kind(THREATEN, "Gain control of target land until end of turn.")]
#[case::permanent_for_the_turn(
    PERMANENT_CONTROL,
    "Gain control of target artifact until end of turn."
)]
#[case::token_with_another_attached(
    GRIP,
    "Gain control of target Equipment, then create a 0/0 black Phyrexian Germ creature token and attach that Aura to it."
)]
#[case::token_of_another_size(
    GRIP,
    "Gain control of target Equipment, then create a 1/1 black Phyrexian Germ creature token and attach that Equipment to it."
)]
#[case::control_for_the_turn_only(
    GRIP,
    "Gain control of target Equipment until end of turn, then create a 0/0 black Phyrexian Germ creature token and attach that Equipment to it."
)]
#[case::another_counter(
    GILRAEN,
    "{2}, {T}: Exile another target creature you control. You may return that card to the battlefield under its owner's control. If you don't, at the beginning of the next end step, return that card to the battlefield under its owner's control with a flying counter and a lifelink counter on it."
)]
#[case::no_second_chance(
    GILRAEN,
    "{2}, {T}: Exile another target creature you control. You may return that card to the battlefield under its owner's control. At the beginning of the next end step, return that card to the battlefield under its owner's control with a vigilance counter and a lifelink counter on it."
)]
#[case::one_counter(
    GILRAEN,
    "{2}, {T}: Exile another target creature you control. You may return that card to the battlefield under its owner's control. If you don't, at the beginning of the next end step, return that card to the battlefield under its owner's control with a vigilance counter on it."
)]
#[case::not_tapped(
    KODAMA,
    "Search your library for up to two basic land cards, reveal those cards, put one onto the battlefield and the other into your hand, then shuffle."
)]
#[case::swapped(
    KODAMA,
    "Search your library for up to two basic land cards, reveal those cards, put one into your hand and the other onto the battlefield tapped, then shuffle."
)]
#[case::three_lands(
    KODAMA,
    "Search your library for up to three basic land cards, reveal those cards, put one onto the battlefield tapped and the other into your hand, then shuffle."
)]
#[case::to_the_battlefield(
    LYCHGUARD,
    "{3}{B}, Sacrifice ~: Return all legendary creature cards from your graveyard to the battlefield."
)]
#[case::one_card(
    LYCHGUARD,
    "{3}{B}, Sacrifice ~: Return a legendary creature card from your graveyard to your hand."
)]
#[case::other_cards(
    LYCHGUARD,
    "{3}{B}, Sacrifice ~: Return all legendary artifact cards from your graveyard to your hand."
)]
#[case::not_optional(
    GRAPPLE,
    "Mill three cards, then return a creature or land card from your graveyard to your hand."
)]
#[case::mill_two(
    GRAPPLE,
    "Mill two cards, then you may return a creature or land card from your graveyard to your hand."
)]
#[case::one_type(
    GRAPPLE,
    "Mill three cards, then you may return a creature card from your graveyard to your hand."
)]
#[case::to_the_battlefield_from_the_graveyard(
    GRAPPLE,
    "Mill three cards, then you may return a creature or land card from your graveyard to the battlefield."
)]
#[case::an_opponent(
    LOAMING,
    "When ~ enters, target opponent shuffles any number of target cards from their graveyard into their library."
)]
#[case::creature_cards(
    LOAMING,
    "When ~ enters, target player shuffles any number of target creature cards from their graveyard into their library."
)]
#[case::up_to_two(
    LOAMING,
    "When ~ enters, target player shuffles up to two target cards from their graveyard into their library."
)]
#[case::your_graveyard(
    KEENING,
    "{5}, {T}: Target player mills X cards, where X is the number of cards in your graveyard."
)]
#[case::all_graveyards(
    KEENING,
    "{5}, {T}: Target player mills X cards, where X is the number of cards in all graveyards."
)]
fn a_changed_verb_group_count_place_player_or_duration_does_not_read_back(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}
