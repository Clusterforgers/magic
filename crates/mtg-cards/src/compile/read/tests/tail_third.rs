//! Goal: the wordings that alone held back two precon cards each in the third round (the controller of the creature that entered, a free cast with a mana value limit, a creature that gains shroud after an attach, a return from the graveyard at the end step, another creature that dies and returns, damage from one creature to another, a creature dealt damage by this creature) read into the DSL, write back as the same text, and leave their near-misses to the table.
use super::*;
use crate::effect::{Destination, Effect};
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::{Amount, Cmp, StatKind};
use mtg_core::card::CardType;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

fn effects_of(read: Ability) -> Vec<Effect> {
    match read {
        Ability::Triggered { effects, .. } | Ability::Spell { effects, .. } => effects,
        other => panic!("not a trigger or a spell: {other:?}"),
    }
}

#[rstest]
#[case::one(
    "Whenever a creature an opponent controls enters, you may have that player lose 1 life.",
    1
)]
#[case::three(
    "Whenever a creature an opponent controls enters, you may have that player lose 3 life.",
    3
)]
#[case::any_creature("Whenever a creature enters, you may have that player lose 2 life.", 2)]
fn the_controller_of_the_creature_that_entered_may_lose_life(
    #[case] text: &str,
    #[case] lost: i32,
) {
    let effects = effects_of(golden(text, ClauseKind::Triggered, false));
    assert_eq!(
        effects,
        vec![Effect::May {
            who: Ref::You,
            effects: vec![Effect::LoseLife {
                who: Ref::ControllerOf(Box::new(Ref::Triggering)),
                amount: Amount::Fixed(lost),
            }],
        }],
        "{text}"
    );
}

#[rstest]
#[case::the_card_itself("When ~ enters, you may have that player lose 1 life.")]
#[case::not_a_creature("Whenever you draw a card, you may have that player lose 1 life.")]
#[case::a_target_player("Whenever a creature enters, you may have target player lose 1 life.")]
#[case::gain("Whenever a creature enters, you may have that player gain 1 life.")]
#[case::without_the_choice("Whenever a creature enters, have that player lose 1 life.")]
#[case::a_spell("You may have that player lose 1 life.")]
fn that_player_loses_life_only_for_the_controller_of_what_entered(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
    assert_eq!(spell(text), None, "{text}");
}

fn nonland_in_hand(limit: Option<Amount>) -> Ref {
    let mut preds = vec![
        Pred::In(ZoneKind::Hand),
        Pred::OwnedBy(Ref::You),
        Pred::Not(Box::new(Pred::Type(CardType::Land))),
    ];
    preds.extend(limit.map(|to| Pred::Compare {
        stat: StatKind::ManaValue,
        op: Cmp::Le,
        to,
    }));
    Ref::Each(Selector(preds))
}

#[rstest]
#[case::four(
    "You may cast a spell with mana value 4 or less from your hand without paying its mana cost.",
    Some(Amount::Fixed(4))
)]
#[case::five(
    "You may cast a spell with mana value 5 or less from your hand without paying its mana cost.",
    Some(Amount::Fixed(5))
)]
#[case::zero(
    "You may cast a spell with mana value 0 or less from your hand without paying its mana cost.",
    Some(Amount::Fixed(0))
)]
#[case::x(
    "You may cast a spell with mana value X or less from your hand without paying its mana cost.",
    Some(Amount::X)
)]
fn a_spell_up_to_a_mana_value_is_cast_free_from_the_hand(
    #[case] text: &str,
    #[case] limit: Option<Amount>,
) {
    let read = golden(text, ClauseKind::Spell, true);
    let effects = effects_of(read);
    let [
        Effect::Choose {
            from, count, bind, ..
        },
        Effect::CastFromZone { what, free, .. },
    ] = effects.as_slice()
    else {
        panic!("not a choice and a cast: {effects:?}");
    };
    assert_eq!(*from, nonland_in_hand(limit), "{text}");
    assert_eq!(*count, Bounds::up_to(Some(Amount::Fixed(1))));
    assert_eq!(*what, Ref::Bound(bind.clone()));
    assert!(*free, "{text}");
}

#[rstest]
#[case::the_graveyard(
    "You may cast a spell with mana value 4 or less from your graveyard without paying its mana cost."
)]
#[case::more(
    "You may cast a spell with mana value 4 or more from your hand without paying its mana cost."
)]
#[case::paying("You may cast a spell with mana value 4 or less from your hand.")]
#[case::forced(
    "Cast a spell with mana value 4 or less from your hand without paying its mana cost."
)]
#[case::an_opponent_casts(
    "Target opponent may cast a spell with mana value 4 or less from their hand without paying its mana cost."
)]
#[case::words(
    "You may cast a spell with mana value four or fewer from your hand without paying its mana cost."
)]
fn free_casts_that_are_not_a_spell_up_to_a_value_from_the_hand_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::shroud(
    "When ~ enters, attach it to target creature you control. That creature gains shroud until end of turn."
)]
#[case::haste(
    "When ~ enters, attach it to target creature you control. That creature gains haste until end of turn."
)]
#[case::it(
    "When ~ enters, attach it to target creature you control. It gains shroud until end of turn."
)]
#[case::two_keywords(
    "When ~ enters, attach it to target creature you control. That creature gains hexproof and haste until end of turn."
)]
fn a_creature_gains_a_keyword_after_the_card_attaches_to_it(#[case] text: &str) {
    let Ability::Triggered { effects, .. } = golden(text, ClauseKind::Triggered, false) else {
        panic!("not a trigger: {text}");
    };
    assert!(
        matches!(
            effects.as_slice(),
            [
                Effect::Attach {
                    to: Ref::Target(0),
                    ..
                },
                Effect::Continuous {
                    affected: Ref::Target(0),
                    ..
                }
            ]
        ),
        "{effects:?}"
    );
}

#[rstest]
#[case::alone("That creature gains shroud until end of turn.")]
#[case::another_creature(
    "When ~ enters, attach it to target creature you control. Another creature gains shroud until end of turn."
)]
#[case::those(
    "When ~ enters, attach it to target creature you control. Those creatures gain shroud until end of turn."
)]
#[case::first(
    "When ~ enters, that creature gains shroud until end of turn. Attach it to target creature you control."
)]
fn that_creature_needs_the_attach_before_it(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::tapped(
    "At the beginning of your end step, if you gained 3 or more life this turn, return this card from your graveyard to the battlefield tapped.",
    true
)]
#[case::untapped(
    "At the beginning of your end step, if you gained 2 or more life this turn, return this card from your graveyard to the battlefield.",
    false
)]
#[case::every_end_step(
    "At the beginning of each end step, if you gained 4 or more life this turn, return this card from your graveyard to the battlefield tapped.",
    true
)]
fn a_card_returns_itself_at_the_end_step_for_the_life_gained(
    #[case] text: &str,
    #[case] tapped: bool,
) {
    let read = golden(text, ClauseKind::Triggered, false);
    let Ability::Triggered {
        from,
        intervening_if: Some(_),
        effects,
        ..
    } = read
    else {
        panic!("no condition: {text}");
    };
    assert_eq!(from, ZoneKind::Graveyard, "{text}");
    assert!(
        matches!(
            effects.as_slice(),
            [Effect::Move { what: Ref::This, to: Destination::Battlefield { tapped: held, .. } }] if *held == tapped
        ),
        "{effects:?}"
    );
}

#[rstest]
#[case::from_the_battlefield(
    "At the beginning of your end step, if you gained 3 or more life this turn, return this card from the battlefield to the hand."
)]
#[case::from_exile(
    "At the beginning of your end step, if you gained 3 or more life this turn, return this card from exile to the battlefield."
)]
fn the_return_from_the_graveyard_is_only_of_the_card_itself(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

fn the_dead_card() -> Ref {
    Ref::Each(Selector(vec![
        Pred::In(ZoneKind::Graveyard),
        Pred::SameCard(Ref::Triggering),
    ]))
}

#[rstest]
#[case::at_the_end_step(
    "Whenever another multicolored creature you control dies, return it to its owner's hand at the beginning of the next end step.",
    true
)]
#[case::a_nontoken_creature(
    "Whenever another nontoken creature you control dies, return it to its owner's hand at the beginning of the next end step.",
    true
)]
#[case::now(
    "Whenever another creature you control dies, return it to its owner's hand.",
    false
)]
fn another_creature_that_dies_returns_as_the_card_in_the_graveyard(
    #[case] text: &str,
    #[case] later: bool,
) {
    let effects = effects_of(golden(text, ClauseKind::Triggered, false));
    if later {
        let [
            Effect::Choose { from, bind, .. },
            Effect::DelayedTrigger {
                effects: moved,
                once: true,
                ..
            },
        ] = effects.as_slice()
        else {
            panic!("not a choice and a delayed trigger: {effects:?}");
        };
        assert_eq!(*from, the_dead_card(), "{text}");
        assert_eq!(
            *moved,
            vec![Effect::Move {
                what: Ref::Bound(bind.clone()),
                to: Destination::Hand
            }]
        );
    } else {
        assert_eq!(
            effects,
            vec![Effect::Move {
                what: the_dead_card(),
                to: Destination::Hand
            }],
            "{text}"
        );
    }
}

#[rstest]
#[case::enters(
    "Whenever another creature you control enters, return it to its owner's hand at the beginning of the next end step."
)]
#[case::attacks(
    "Whenever another creature you control attacks, return it to its owner's hand at the beginning of the next end step."
)]
#[case::exile(
    "Whenever another creature you control dies, exile it at the beginning of the next end step."
)]
#[case::the_battlefield(
    "Whenever another creature you control dies, return it to the battlefield at the beginning of the next end step."
)]
#[case::your_end_step(
    "Whenever another creature you control dies, return it to its owner's hand at the beginning of your next end step."
)]
fn the_dead_card_returns_to_the_hand_at_the_next_end_step_only(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::opponent(
    "Target creature you control deals damage equal to its power to target creature an opponent controls."
)]
#[case::you_dont_control(
    "Target creature you control deals damage equal to its power to target creature you don't control."
)]
#[case::any_creature(
    "Target creature you control deals damage equal to its power to target creature."
)]
fn one_creature_deals_damage_equal_to_its_power_to_another(#[case] text: &str) {
    let Ability::Spell { targets, effects } = golden(text, ClauseKind::Spell, false) else {
        panic!("not a spell: {text}");
    };
    assert_eq!(targets.len(), 2, "{text}");
    assert_eq!(
        effects,
        vec![Effect::DealDamage {
            source: Some(Ref::Target(0)),
            to: Ref::Target(1),
            amount: Amount::Stat {
                of: Ref::Target(0),
                stat: StatKind::Power
            },
        }],
        "{text}"
    );
}

#[rstest]
#[case::toughness(
    "Target creature you control deals damage equal to its toughness to target creature an opponent controls."
)]
#[case::fixed(
    "Target creature you control deals 3 damage to target creature an opponent controls."
)]
#[case::to_itself("Target creature you control deals damage equal to its power to itself.")]
#[case::to_a_player(
    "Target creature you control deals damage equal to its power to target player."
)]
#[case::one_target("Target creature deals damage equal to its power to it.")]
fn damage_between_creatures_that_is_not_equal_to_power_stays_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::counter(
    "Whenever a creature dealt damage by this creature this turn dies, put a +1/+1 counter on this creature."
)]
#[case::counter_on_it(
    "Whenever a creature dealt damage by this creature this turn dies, put a +1/+1 counter on ~."
)]
#[case::token(
    "Whenever a creature dealt damage by this creature this turn dies, create a Blood token."
)]
fn a_creature_dealt_damage_by_this_creature_dies(#[case] text: &str) {
    let Ability::Triggered { when, .. } = golden(text, ClauseKind::Triggered, false) else {
        panic!("not a trigger: {text}");
    };
    let TriggerPattern::ZoneChange { what, from, to } = when else {
        panic!("not a death: {text}");
    };
    assert_eq!(
        what,
        Selector(vec![
            Pred::Type(CardType::Creature),
            Pred::DamagedBy(Ref::This)
        ]),
        "{text}"
    );
    assert_eq!(
        (from, to),
        (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard))
    );
}

#[rstest]
#[case::another_creature(
    "Whenever a creature dealt damage by another creature this turn dies, put a +1/+1 counter on this creature."
)]
#[case::no_turn(
    "Whenever a creature dealt damage by this creature dies, put a +1/+1 counter on this creature."
)]
#[case::any_damage(
    "Whenever a creature dealt damage this turn dies, put a +1/+1 counter on this creature."
)]
#[case::this_turn_first(
    "Whenever a creature dealt damage this turn by this creature dies, put a +1/+1 counter on this creature."
)]
fn damage_by_something_else_stays_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

#[rstest]
#[case::elemental(
    "As ~ enters, you may reveal an Elemental card from your hand. If you don't, ~ enters tapped.",
    "Elemental"
)]
#[case::faerie(
    "As ~ enters, you may reveal a Faerie card from your hand. If you don't, ~ enters tapped.",
    "Faerie"
)]
#[case::elf(
    "As ~ enters, you may reveal an Elf card from your hand. If you don't, ~ enters tapped.",
    "Elf"
)]
#[case::treefolk(
    "As ~ enters, you may reveal a Treefolk card from your hand. If you don't, ~ enters tapped.",
    "Treefolk"
)]
#[case::kithkin(
    "As ~ enters, you may reveal a Kithkin card from your hand. If you don't, ~ enters tapped.",
    "Kithkin"
)]
#[case::it(
    "As ~ enters, you may reveal a Giant card from your hand. If you don't, it enters tapped.",
    "Giant"
)]
fn a_land_enters_tapped_unless_a_card_of_its_kind_is_revealed(
    #[case] text: &str,
    #[case] kind: &str,
) {
    let Ability::Replacement {
        by: ReplaceBy::RevealOrTapped(of),
        when: ReplaceWhen::Enters { what },
        ..
    } = golden(text, ClauseKind::Static, false)
    else {
        panic!("not a reveal: {text}");
    };
    assert_eq!(what, Selector::this());
    assert_eq!(of.0.len(), 3, "{of:?}");
    for pred in [
        Pred::Subtype(mtg_core::card::Subtype(kind.to_owned())),
        Pred::In(ZoneKind::Hand),
        Pred::OwnedBy(Ref::You),
    ] {
        assert!(of.0.contains(&pred), "{pred:?} in {of:?}");
    }
}

#[rstest]
#[case::the_graveyard(
    "As ~ enters, you may reveal an Elemental card from your graveyard. If you don't, ~ enters tapped."
)]
#[case::another_penalty(
    "As ~ enters, you may reveal an Elemental card from your hand. If you don't, you lose 1 life."
)]
#[case::if_you_do(
    "As ~ enters, you may reveal an Elemental card from your hand. If you do, ~ enters tapped."
)]
#[case::untapped(
    "As ~ enters, you may reveal an Elemental card from your hand. If you don't, ~ enters untapped."
)]
#[case::two_cards(
    "As ~ enters, you may reveal two Elemental cards from your hand. If you don't, ~ enters tapped."
)]
#[case::pay_life_too(
    "As ~ enters, you may reveal an Elemental card from your hand or pay 2 life. If you don't, ~ enters tapped."
)]
fn a_reveal_that_is_not_for_a_card_of_a_kind_from_the_hand_stays_unread(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
