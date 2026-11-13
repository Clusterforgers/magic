//! Goal: the r47-01 wordings read and write back: a card's flash permission for beholding or for tapping creatures of a
//! kind, the cast from the graveyard "but not from anywhere else" as its two abilities, and the boast rule; a near miss
//! (paid in mana, another kind of cost, another zone, other players) reads as something else or not at all.
use super::*;
use crate::compile::read::{Own, static_pair, static_pair_texts};
use crate::cost::{Cost, CostItem};
use crate::modify::GameRule;

#[rstest]
#[case::behold_a_dragon(
    "You may cast this spell as though it had flash if you behold a Dragon as an additional cost to cast it.",
    r#"[{"Behold":{"filter":[{"Subtype":"Dragon"}]}}]"#
)]
#[case::behold_an_elf(
    "You may cast this spell as though it had flash if you behold an Elf as an additional cost to cast it.",
    r#"[{"Behold":{"filter":[{"Subtype":"Elf"}]}}]"#
)]
#[case::tap_three_flyers(
    "You may cast this spell as though it had flash by tapping three untapped creatures you control with flying in addition to paying its other costs.",
    r#"[{"TapPermanents":{"filter":[{"Type":"Creature"},{"HasKeyword":"Flying"}],"count":3}}]"#
)]
#[case::tap_two_creatures(
    "You may cast this spell as though it had flash by tapping two untapped creatures you control in addition to paying its other costs.",
    r#"[{"TapPermanents":{"filter":[{"Type":"Creature"}],"count":2}}]"#
)]
fn r4701_flash_for_a_cost_reads_and_writes_back(#[case] text: &str, #[case] cost: &str) {
    let read = golden(text, ClauseKind::Static, true);
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition: None,
        ..
    } = read
    else {
        panic!("{text}: {read:?}");
    };
    assert!(permission.flash, "{text}");
    let cost: Cost = serde_json::from_str(cost).unwrap();
    assert_eq!(permission.additional, Some(cost), "{text}");
}

#[rstest]
#[case::behold_and_more(
    "You may cast this spell as though it had flash if you behold two Dragons as an additional cost to cast it."
)]
#[case::reveal_instead(
    "You may cast this spell as though it had flash if you reveal a Dragon card from your hand as an additional cost to cast it."
)]
#[case::tap_without_untapped(
    "You may cast this spell as though it had flash by tapping three creatures an opponent controls in addition to paying its other costs."
)]
fn r4701_flash_near_misses_are_no_behold_or_tap(#[case] text: &str) {
    let read = clause(text, ClauseKind::Static, true);
    let held = |item: &CostItem| {
        matches!(
            item,
            CostItem::Behold { .. } | CostItem::TapPermanents { .. }
        )
    };
    if let Some(Ability::Rule {
        rule: GameRule::MayCast(permission),
        ..
    }) = &read
    {
        let items = permission
            .additional
            .as_ref()
            .map(|cost| cost.0.as_slice())
            .unwrap_or_default();
        assert!(
            !items.iter().any(held)
                || !same_text(&render_variants(read.as_ref().unwrap())[0], text),
            "{text}"
        );
    }
}

#[test]
fn r4701_graveyard_only_reads_as_the_permission_and_the_restriction() {
    let text = "You may cast this card from your graveyard, but not from anywhere else.";
    let read = static_pair(text, false, &Own::default()).unwrap();
    assert_eq!(
        read,
        vec![
            ability(
                r#"{"Rule":{"rule":{"MayCast":{"what":["This"],"from":"Graveyard"}},"from":"Graveyard"}}"#
            ),
            ability(
                r#"{"CastOnly":{"condition":{"Matches":{"subject":"This","selector":[{"In":"Graveyard"}]}}}}"#
            ),
        ]
    );
    assert!(
        static_pair_texts(&read[0], &read[1])
            .iter()
            .any(|written| same_text(written, text))
    );
}

#[rstest]
#[case::from_your_hand("You may cast this card from your hand, but not from anywhere else.")]
#[case::without_the_restriction("You may cast this card from your graveyard.")]
fn r4701_graveyard_only_near_misses(#[case] text: &str) {
    assert!(
        static_pair(text, false, &Own::default())
            .is_none_or(|read| !read.iter().any(|a| matches!(a, Ability::CastOnly { .. }))),
        "{text}"
    );
}

#[rstest]
#[case::the_controller(
    "Creatures you control can boast twice during each of your turns rather than once.",
    true
)]
#[case::three_times(
    "Creatures you control can boast three times during each of your turns rather than once.",
    false
)]
fn r4701_boast_twice(#[case] text: &str, #[case] reads: bool) {
    let read = clause(text, ClauseKind::Static, true);
    let twice = matches!(
        read,
        Some(Ability::Rule {
            rule: GameRule::BoastTwice(crate::refs::Ref::You),
            ..
        })
    );
    assert_eq!(twice, reads, "{text}: {read:?}");
    if reads {
        golden(text, ClauseKind::Static, true);
    }
}
