//! Goal: the creatures that crewed or saddled the source this turn ("creature that crewed it this turn") read into `Pred::Crewed` and `Pred::Saddled` of the source as a target, as a plural group and as the choice of "any number of" them, and write back as the card's own text; wordings about another permanent stay unread.
use super::*;

#[rstest]
#[case::explores(
    "Whenever ~ attacks, target creature that crewed it this turn explores.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Crewed":"This"}]}}],"effects":[{"Explore":{"what":{"Target":0}}}]}}"#
)]
#[case::counter(
    "Whenever ~ attacks, put a +1/+1 counter on target creature that saddled it this turn.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Saddled":"This"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#
)]
#[case::up_to_one_returned(
    "Whenever ~ attacks or blocks, return up to one target creature that crewed it this turn to its owner's hand.",
    r#"{"Triggered":{"when":{"AnyOf":[{"Attacks":{"what":["This"]}},{"Blocks":{"what":["This"]}}]},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Crewed":"This"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#
)]
fn the_creatures_that_paid_for_the_source_read_and_write_back(
    #[case] text: &str,
    #[case] written: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, written);
}

#[rstest]
#[case::then_you_may_return(
    "Whenever ~ attacks, draw a card. Then you may return any number of creatures that saddled it this turn to their owner's hand."
)]
#[case::you_may_return(
    "Whenever ~ attacks, you may return any number of creatures that crewed it this turn to their owner's hand."
)]
fn the_choice_of_any_number_of_the_creatures_is_a_choice_and_a_move(#[case] text: &str) {
    let read = golden(text, ClauseKind::Triggered, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(
        json.contains(r#"{"Choose":{"from":{"Each":[{"Type":"Creature"},"#),
        "{json}"
    );
    assert!(json.contains(r#""count":{"min":0,"max":null}"#), "{json}");
    assert!(
        json.contains(r#"{"Move":{"what":{"Bound":"returned"},"to":"Hand"}}"#),
        "{json}"
    );
}

#[rstest]
#[case::another_permanent(
    "Whenever ~ attacks, target creature that crewed target Vehicle this turn explores."
)]
#[case::another_turn("Whenever ~ attacks, target creature that crewed it last turn explores.")]
#[case::without_a_may(
    "Whenever ~ attacks, return any number of creatures that saddled it this turn to their owner's hand."
)]
#[case::not_a_creature_group(
    "Whenever ~ attacks, you may return any number of cards that saddled it this turn to their owner's hand."
)]
#[case::an_opponent_group(
    "Whenever ~ attacks, you may return any number of creatures your opponents control to their owner's hand."
)]
fn other_wordings_about_who_crewed_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

#[rstest]
#[case::another_vehicle(
    "Whenever ~ becomes crewed, up to one other target Vehicle you control becomes an artifact creature until end of turn.",
    r#"{"Triggered":{"when":{"BecomesCrewed":{"what":["This"]}},"targets":[{"what":{"Object":[{"Subtype":"Vehicle"},"Other",{"ControlledBy":"You"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Artifact"},{"AddType":"Creature"}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::mill_then_grow(
    "Whenever ~ becomes saddled for the first time each turn, mill two cards, then ~ gets +X/+X until end of turn, where X is the number of creature cards in your graveyard.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"BecomesSaddled":{"what":["This"]}},"nth":1}},"effects":[{"Mill":{"count":2}},{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"toughness":{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn the_wordings_after_a_crew_or_a_saddle_read_and_write_back(
    #[case] text: &str,
    #[case] written: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, written);
}

#[rstest]
#[case::grow_first(
    "Whenever ~ becomes saddled, ~ gets +X/+X until end of turn, then mill two cards, where X is the number of creature cards in your graveyard."
)]
#[case::another_step(
    "Whenever ~ becomes saddled, draw a card, then ~ gets +1/+1 until end of turn."
)]
#[case::a_target_after_a_mill(
    "Whenever ~ becomes saddled, mill two cards, then target creature gets +1/+1 until end of turn."
)]
fn a_change_reads_after_a_mill_only_as_a_step_of_the_card_itself(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None, "{text}");
}

#[rstest]
#[case::up_to_one("Return up to one target creature to its owner's hand.")]
#[case::one("Return target creature to its owner's hand.")]
#[case::up_to_one_you_control("Return up to one target artifact you control to its owner's hand.")]
fn a_target_that_may_stay_unchosen_is_returned_like_one_that_may_not(#[case] text: &str) {
    let read = golden(text, ClauseKind::Spell, false);
    let json = serde_json::to_string(&read).unwrap();
    assert!(
        json.contains(r#"{"Move":{"what":{"Target":0},"to":"Hand"}}"#),
        "{json}"
    );
}
