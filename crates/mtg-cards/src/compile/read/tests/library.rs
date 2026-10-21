//! Goal: searching a library (with the move of what is found and the shuffle), looking at its top cards, shuffling it and putting a card on it read in the shapes of the hand-written files for a card or several, with every filter the cards print, and write back as the same text; other zones, names, counts and orders stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::rampant_growth(
    "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#
)]
#[case::that_card(
    "Search your library for a basic land card, put that card onto the battlefield tapped, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#
)]
#[case::demonic_tutor(
    "Search your library for a card, put that card into your hand, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[],"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Shuffle":{}}]}}"#
)]
#[case::revealed(
    "Search your library for a creature card, reveal it, put it into your hand, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Type":"Creature"}],"reveal":true,"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Shuffle":{}}]}}"#
)]
#[case::two_land_types(
    "Search your library for a Plains or Island card, put it onto the battlefield, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"AnyOf":[{"Subtype":"Plains"},{"Subtype":"Island"}]}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{}}}},{"Shuffle":{}}]}}"#
)]
#[case::two_card_types(
    "Search your library for an instant or sorcery card, reveal it, put it into your hand, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"reveal":true,"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Shuffle":{}}]}}"#
)]
#[case::two_subtypes(
    "Search your library for an Aura or Equipment card, reveal it, put it into your hand, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"}]}],"reveal":true,"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Shuffle":{}}]}}"#
)]
#[case::with_a_mana_value(
    "Search your library for a creature card with mana value 3 or less, put it onto the battlefield, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Le","to":3}}],"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{}}}},{"Shuffle":{}}]}}"#
)]
#[case::a_subtype_of_permanents(
    "Search your library for a Rebel permanent card with mana value 2 or less, put it onto the battlefield, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Subtype":"Rebel"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"},{"Type":"Battle"}]},{"Compare":{"stat":"ManaValue","op":"Le","to":2}}],"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{}}}},{"Shuffle":{}}]}}"#
)]
#[case::up_to_two(
    "Search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"count":{"min":0,"max":2},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#
)]
#[case::any_number(
    "Search your library for any number of Dragon creature cards, put them onto the battlefield, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Subtype":"Dragon"},{"Type":"Creature"}],"count":{"min":0,"max":null},"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{}}}},{"Shuffle":{}}]}}"#
)]
#[case::plural_revealed(
    "Search your library for up to two land cards, reveal them, put them into your hand, then shuffle.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Type":"Land"}],"count":{"min":0,"max":2},"reveal":true,"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":"Hand"}},{"Shuffle":{}}]}}"#
)]
#[case::on_top(
    "Search your library for a card, then shuffle and put that card on top.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[],"as":"card"}},{"Shuffle":{}},{"Move":{"what":{"Bound":"card"},"to":{"Library":{"place":"Top"}}}}]}}"#
)]
#[case::revealed_on_top(
    "Search your library for a basic land card, reveal it, then shuffle and put that card on top.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"reveal":true,"as":"land"}},{"Shuffle":{}},{"Move":{"what":{"Bound":"land"},"to":{"Library":{"place":"Top"}}}}]}}"#
)]
#[case::on_top_and_a_cost(
    "Search your library for a card, then shuffle and put that card on top. You lose 2 life.",
    r#"{"Spell":{"effects":[{"Search":{"filter":[],"as":"card"}},{"Shuffle":{}},{"Move":{"what":{"Bound":"card"},"to":{"Library":{"place":"Top"}}}},{"LoseLife":{"amount":2}}]}}"#
)]
#[case::after_another_effect(
    "Destroy target land. Search your library for a basic land card, put that card onto the battlefield tapped, then shuffle.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#
)]
fn a_search_reads_in_the_shape_of_the_hand_written_files(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::flooded_strand(
    "{T}, Pay 1 life, Sacrifice ~: Search your library for a Plains or Island card, put it onto the battlefield, then shuffle.",
    r#"{"Activated":{"cost":["Tap",{"PayLife":1},"SacrificeSelf"],"effects":[{"Search":{"filter":[{"AnyOf":[{"Subtype":"Plains"},{"Subtype":"Island"}]}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{}}}},{"Shuffle":{}}]}}"#
)]
#[case::evolving_wilds(
    "{T}, Sacrifice ~: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
    r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#
)]
#[case::an_artifact_that_fetches(
    "{2}, {T}, Sacrifice ~: Search your library for a basic land card, reveal it, put it into your hand, then shuffle.",
    r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap","SacrificeSelf"],"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"reveal":true,"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":"Hand"}},{"Shuffle":{}}]}}"#
)]
#[case::a_sorcery_speed_tutor(
    "{3}, {T}: Search your library for a Mercenary permanent card with mana value 3 or less, put it onto the battlefield, then shuffle.",
    r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"effects":[{"Search":{"filter":[{"Subtype":"Mercenary"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"},{"Type":"Battle"}]},{"Compare":{"stat":"ManaValue","op":"Le","to":3}}],"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{}}}},{"Shuffle":{}}]}}"#
)]
fn a_search_as_the_effect_of_an_activated_ability_reads(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::may_reveal(
    "When ~ enters, you may search your library for a basic land card, reveal it, put it into your hand, then shuffle.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"reveal":true,"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":"Hand"}},{"Shuffle":{}}]}}]}}"#
)]
#[case::on_dying(
    "When ~ dies, search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#
)]
fn a_search_in_a_trigger_reads(#[case] text: &str, #[case] json: &str) {
    assert_eq!(trigger(text), Some(ability(json)), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

/// Every filter the cards print with every count, destination and revelation, alone and after
/// "you may".
#[test]
fn every_filter_count_and_destination_of_a_search_writes_back() {
    let filters = [
        ("card", "cards"),
        ("basic land card", "basic land cards"),
        ("land card", "land cards"),
        ("Forest card", "Forest cards"),
        ("Plains or Island card", "Plains or Island cards"),
        ("Aura or Equipment card", "Aura or Equipment cards"),
        ("creature card", "creature cards"),
        ("artifact or creature card", "artifact or creature cards"),
        ("legendary creature card", "legendary creature cards"),
        ("nonland card", "nonland cards"),
        ("Dragon creature card", "Dragon creature cards"),
        (
            "Rebel permanent card with mana value 2 or less",
            "Rebel permanent cards with mana value 2 or less",
        ),
        (
            "artifact card with mana value 1 or less",
            "artifact cards with mana value 1 or less",
        ),
    ];
    let counts = ["up to two", "up to three", "any number of"];
    let places = [
        "onto the battlefield",
        "onto the battlefield tapped",
        "into your hand",
    ];
    let mut read = 0;
    for (one, many) in filters {
        let article = if one.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'I']) {
            "an"
        } else {
            "a"
        };
        let mut wordings = Vec::new();
        for reveal in [false, true] {
            let (it, them) = if reveal {
                (", reveal it", ", reveal them")
            } else {
                ("", "")
            };
            for place in places {
                wordings.push(format!(
                    "search your library for {article} {one}{it}, put it {place}, then shuffle"
                ));
                wordings.push(format!(
                    "search your library for {article} {one}{it}, put that card {place}, then shuffle"
                ));
                for count in counts {
                    wordings.push(format!(
                        "search your library for {count} {many}{them}, put them {place}, then shuffle"
                    ));
                }
            }
            wordings.push(format!(
                "search your library for {article} {one}{it}, then shuffle and put that card on top"
            ));
        }
        for wording in wordings {
            let sentence = format!("{}.", capitalize(&wording));
            golden(&sentence, ClauseKind::Spell, false);
            let optional = format!("When ~ enters, you may {wording}.");
            golden(&optional, ClauseKind::Triggered, false);
            let activated = format!("{{T}}, Sacrifice ~: {}", capitalize(&sentence));
            golden(&activated, ClauseKind::Activated, false);
            read += 3;
        }
    }
    assert!(read > 1000, "only {read} wordings were read");
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[rstest]
#[case::no_card_found_yet("Search your library for a card.")]
#[case::no_shuffle(
    "Search your library for a basic land card, put it onto the battlefield tapped."
)]
#[case::shuffle_first(
    "Search your library for a basic land card, shuffle, then put it onto the battlefield."
)]
#[case::exiled("Search your library for a card, exile it, then shuffle.")]
#[case::into_the_graveyard(
    "Search your library for a card, put it into your graveyard, then shuffle."
)]
#[case::another_library(
    "Search target player's library for a card, put it onto the battlefield, then shuffle."
)]
#[case::the_graveyard_too(
    "Search your library and graveyard for a card, put it into your hand, then shuffle."
)]
#[case::a_name("Search your library for a card named Island, put it into your hand, then shuffle.")]
#[case::two_kinds(
    "Search your library for a basic land card and a creature card, put them into your hand, then shuffle."
)]
#[case::different_kinds_in_two_places(
    "Search your library for a basic land card, put it onto the battlefield, put a creature card into your hand, then shuffle."
)]
#[case::two_without_up_to(
    "Search your library for two basic land cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::up_to_one(
    "Search your library for up to one basic land card, put it onto the battlefield, then shuffle."
)]
#[case::up_to_x(
    "Search your library for up to X basic land cards, put them onto the battlefield, then shuffle."
)]
#[case::digits(
    "Search your library for up to 2 basic land cards, put them onto the battlefield, then shuffle."
)]
#[case::plural_for_one(
    "Search your library for a basic land cards, put it onto the battlefield, then shuffle."
)]
#[case::singular_for_many(
    "Search your library for up to two basic land card, put them onto the battlefield, then shuffle."
)]
#[case::them_for_one(
    "Search your library for a basic land card, put them onto the battlefield, then shuffle."
)]
#[case::it_for_many(
    "Search your library for up to two basic land cards, put it onto the battlefield, then shuffle."
)]
#[case::many_on_top(
    "Search your library for up to two cards, then shuffle and put those cards on top."
)]
#[case::revealed_late(
    "Search your library for a card, put it into your hand, reveal it, then shuffle."
)]
#[case::a_mana_value_of_x(
    "Search your library for a creature card with mana value X, put it into your hand, then shuffle."
)]
#[case::the_same_mana_value(
    "Search your library for a card with the same mana value as ~, put it into your hand, then shuffle."
)]
#[case::a_card_or_a_card(
    "Search your library for an instant card or a card with flash, put it into your hand, then shuffle."
)]
#[case::under_your_control(
    "Search your library for a creature card, put it onto the battlefield under your control, then shuffle."
)]
#[case::attached(
    "Search your library for an Aura card, put it onto the battlefield attached to ~, then shuffle."
)]
#[case::an_opponent_searches(
    "Target opponent searches their library for a card, puts it into their hand, then shuffles."
)]
#[case::a_sentence_none_reads(
    "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle. Goad it."
)]
fn a_search_the_templates_do_not_model_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::the_top_card(
    "Look at the top card of your library.",
    r#"{"Spell":{"effects":[{"LookAtTop":{"as":"top"}}]}}"#
)]
#[case::the_top_three(
    "Look at the top three cards of your library.",
    r#"{"Spell":{"effects":[{"LookAtTop":{"count":3,"as":"top"}}]}}"#
)]
#[case::keep_one_in_any_order(
    "Look at the top five cards of your library. Put one of them into your hand and the rest on the bottom of your library in any order.",
    r#"{"Spell":{"effects":[{"LookAtTop":{"count":5,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}]}}"#
)]
#[case::keep_one_in_a_random_order(
    "Look at the top four cards of your library. Put one of them into your hand and the rest on the bottom of your library in a random order.",
    r#"{"Spell":{"effects":[{"LookAtTop":{"count":4,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#
)]
#[case::keep_one_and_mill_the_rest(
    "Look at the top four cards of your library. Put one of them into your hand and the rest into your graveyard.",
    r#"{"Spell":{"effects":[{"LookAtTop":{"count":4,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":"Graveyard"}}]}}"#
)]
#[case::then_draw(
    "Look at the top two cards of your library. Draw a card.",
    r#"{"Spell":{"effects":[{"LookAtTop":{"count":2,"as":"top"}},{"Draw":{}}]}}"#
)]
fn a_look_at_the_top_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case("{T}: Look at the top card of your library.")]
#[case("{2}, {T}: Look at the top three cards of your library.")]
#[case("{1}{U}, {T}: Look at the top two cards of your library. Draw a card.")]
fn a_look_as_an_activated_ability_reads(#[case] text: &str) {
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::opponents_library("Look at the top card of target player's library.")]
#[case::x_cards("Look at the top X cards of your library.")]
#[case::digits("Look at the top 3 cards of your library.")]
#[case::singular_for_many("Look at the top three card of your library.")]
#[case::one_card_in_words("Look at the top one cards of your library.")]
#[case::keep_none("Look at the top three cards of your library. Put one of them into your hand.")]
#[case::keep_two(
    "Look at the top five cards of your library. Put two of them into your hand and the rest on the bottom of your library in any order."
)]
#[case::reveal_a_kind(
    "Look at the top five cards of your library. You may reveal a creature card from among them and put it into your hand."
)]
#[case::the_rest_on_top(
    "Look at the top three cards of your library. Put one of them into your hand and the rest on top of your library in any order."
)]
#[case::the_rest_on_the_bottom_unordered(
    "Look at the top three cards of your library. Put one of them into your hand and the rest on the bottom of your library."
)]
#[case::the_rest_exiled(
    "Look at the top three cards of your library. Put one of them into your hand and the rest into exile."
)]
#[case::all_in_the_hand("Look at the top three cards of your library. Put them into your hand.")]
#[case::the_top_without_a_library("Look at the top card.")]
fn a_look_the_templates_do_not_model_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::yours("Shuffle your library.", r#"{"Spell":{"effects":[{"Shuffle":{}}]}}"#)]
#[case::a_target(
    "Target player shuffles their library.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Shuffle":{"who":{"Target":0}}}]}}"#
)]
#[case::each_player(
    "Each player shuffles their library.",
    r#"{"Spell":{"effects":[{"Shuffle":{"who":"EachPlayer"}}]}}"#
)]
#[case::after_a_draw(
    "Draw a card, then shuffle your library.",
    r#"{"Spell":{"effects":[{"Draw":{}},{"Shuffle":{}}]}}"#
)]
#[case::you_may(
    "You may shuffle your library.",
    r#"{"Spell":{"effects":[{"May":{"effects":[{"Shuffle":{}}]}}]}}"#
)]
fn a_shuffle_reads_in_the_shape_of_the_hand_written_files(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::the_graveyard("Shuffle your graveyard into your library.")]
#[case::the_card("Shuffle ~ into its owner's library.")]
#[case::their_for_you("Shuffle their library.")]
#[case::your_for_a_target("Target player shuffles your library.")]
#[case::no_library("Shuffle.")]
fn a_shuffle_the_templates_do_not_model_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::bottom_from_the_graveyard(
    "Put target card from your graveyard on the bottom of your library.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Bottom"}}}}]}}"#
)]
#[case::top_from_the_graveyard(
    "Put target creature card from your graveyard on top of your library.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Top"}}}}]}}"#
)]
#[case::top_of_its_owners(
    "Put target creature on top of its owner's library.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Top"}}}}]}}"#
)]
#[case::bottom_of_its_owners(
    "Put target nonland permanent on the bottom of its owner's library.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Bottom"}}}}]}}"#
)]
fn a_card_put_on_a_library_reads_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case("{3}: Put target card from your graveyard on the bottom of your library.")]
#[case("{T}: Put target creature card from your graveyard on top of your library.")]
fn a_card_put_on_a_library_as_an_activated_ability_reads(#[case] text: &str) {
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::a_wrong_place("Put target card from your graveyard on the top of your library.")]
#[case::the_wrong_library("Put target card from your graveyard on top of its owner's library.")]
#[case::an_owner_for_a_card("Put target creature on top of your library.")]
#[case::a_spell("Put target spell on top of its owner's library.")]
#[case::any_graveyard("Put target card from a graveyard on top of its owner's library.")]
#[case::shuffled_in("Put target card from your graveyard into your library, then shuffle.")]
#[case::in_the_middle("Put target card from your graveyard into your library third from the top.")]
#[case::the_card_itself("Put ~ on top of your library.")]
fn a_card_put_on_a_library_the_templates_do_not_model_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
