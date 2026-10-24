//! Goal: a vote on two named choices reads with its outcome, "gets more votes ... or the vote is tied" as a comparison of the votes of the names or "for each A vote" as a repeat over a name, in the open or secret, and writes back as the same text; a vote with another outcome, other names or a side that does not read stays with the table.
use super::*;

const MAGISTER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Vote":{"options":{"Named":["time","knowledge"]}}},{"If":{"condition":{"Compare":{"left":{"Players":{"Bound":"time"}},"op":"Gt","right":{"Players":{"Bound":"knowledge"}}}},"then":[{"Draw":{"count":2}}],"otherwise":[{"GainLife":{"amount":3}}]}}]}}"#;

#[rstest]
#[case::more_votes_or_tied(
    "When this creature enters, starting with you, each player votes for time or knowledge. If time gets more votes, draw two cards. If knowledge gets more votes or the vote is tied, you gain 3 life.",
    ClauseKind::Triggered,
    MAGISTER
)]
#[case::for_each_vote(
    "Whenever this creature attacks, starting with you, each player votes for evidence or bribery. For each evidence vote, draw a card. For each bribery vote, you gain 2 life.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Vote":{"options":{"Named":["evidence","bribery"]}}},{"RepeatEach":{"over":{"Bound":"evidence"},"as":"voter","effects":[{"Draw":{}}]}},{"RepeatEach":{"over":{"Bound":"bribery"},"as":"voter","effects":[{"GainLife":{"amount":2}}]}}]}}"#
)]
#[case::the_vote_alone(
    "When this creature enters, starting with you, each player votes for grace or condemnation.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Vote":{"options":{"Named":["grace","condemnation"]}}}]}}"#
)]
#[case::secret(
    "When this creature enters, each player secretly votes for truth or consequences, then those votes are revealed.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Vote":{"options":{"Named":["truth","consequences"]},"secret":true}}]}}"#
)]
fn a_vote_on_named_choices_reads_with_what_it_does_with_the_votes(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::the_same_name_twice(
    "When this creature enters, starting with you, each player votes for time or time."
)]
#[case::an_open_vote_without_the_controller_first(
    "When this creature enters, each player votes for time or knowledge."
)]
#[case::a_secret_vote_with_the_controller_first(
    "When this creature enters, starting with you, each player secretly votes for time or knowledge, then those votes are revealed."
)]
#[case::a_secret_vote_never_revealed(
    "When this creature enters, each player secretly votes for time or knowledge."
)]
#[case::three_choices(
    "When this creature enters, starting with you, each player votes for time, knowledge or luck."
)]
#[case::the_sides_swapped(
    "When this creature enters, starting with you, each player votes for time or knowledge. If knowledge gets more votes, draw two cards. If time gets more votes or the vote is tied, you gain 3 life."
)]
#[case::the_tie_to_the_first_side(
    "When this creature enters, starting with you, each player votes for time or knowledge. If time gets more votes or the vote is tied, draw two cards. If knowledge gets more votes, you gain 3 life."
)]
#[case::a_side_that_does_not_read(
    "When this creature enters, starting with you, each player votes for time or knowledge. If time gets more votes, frobnicate. If knowledge gets more votes or the vote is tied, you gain 3 life."
)]
#[case::only_one_side_per_vote(
    "When this creature enters, starting with you, each player votes for time or knowledge. For each time vote, draw a card."
)]
fn a_vote_the_table_words_differently_stays_unread(#[case] text: &str) {
    assert!(
        clause(text, ClauseKind::Triggered, false).is_none(),
        "{text}"
    );
}
