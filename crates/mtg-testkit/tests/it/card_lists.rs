//! Goal: the per-cluster card lists name each card once, every corpus list names cards of the corpus, and every list that counts copies asks for at least one.
use std::collections::HashSet;

use mtg_testkit::action_cards::ACTION_CARDS;
use mtg_testkit::alternative_cards::ALTERNATIVE_CARDS;
use mtg_testkit::attach_cards::{ATTACH_CORPUS_CARDS, ATTACH_SYNTHETIC_CARDS};
use mtg_testkit::cascade_cards::CASCADE_CARDS;
use mtg_testkit::combat_keyword_cards::COMBAT_KEYWORD_CARDS;
use mtg_testkit::corpus;
use mtg_testkit::cost_cards::MODIFIER_CARDS;
use mtg_testkit::counter_cards::{COUNTER_CORPUS_CARDS, COUNTER_SYNTHETIC_CARDS};
use mtg_testkit::defining_cards::DEFINING_CARDS;
use mtg_testkit::evasion_cards::EVASION_CARDS;
use mtg_testkit::event_cards::EVENT_CARDS;
use mtg_testkit::extra_cost_cards::EXTRA_COST_CARDS;
use mtg_testkit::face_cards::FACE_CARDS;
use mtg_testkit::family_cards::FAMILY_CARDS;
use mtg_testkit::foretell_cards::FORETELL_CARDS;
use mtg_testkit::impulse_cards::IMPULSE_CARDS;
use mtg_testkit::kicker_cards::KICKED_CARDS;
use mtg_testkit::layer_cards::LAYER_CARDS;
use mtg_testkit::loyalty_cards::{LOYALTY_CORPUS_CARDS, LOYALTY_SYNTHETIC_CARDS};
use mtg_testkit::madness_cards::MADNESS_CARDS;
use mtg_testkit::nth_cards::NTH_CARDS;
use mtg_testkit::prevent_cards::{self, PREVENT_CARDS};
use mtg_testkit::protect_cards::{self, PROTECT_CARDS};
use mtg_testkit::replace_cards;
use mtg_testkit::rule_cards::RULE_CARDS;
use mtg_testkit::static_cards::STATIC_CARDS;
use mtg_testkit::vehicle_cards::VEHICLE_CARDS;
use mtg_testkit::x_after_cards::X_AFTER_CARDS;
use mtg_testkit::x_cards::X_CARDS;
use mtg_testkit::zone_cards::{ZONE_CORPUS_CARDS, ZONE_SYNTHETIC_CARDS};
use rstest::rstest;

/// The names of the lists that also count copies, one after the other.
fn names(lists: &[&[(&'static str, usize)]]) -> Vec<&'static str> {
    lists
        .iter()
        .flat_map(|list| list.iter().map(|&(name, _)| name))
        .collect()
}

fn assert_once(list: &[&str]) {
    let mut seen = HashSet::new();
    for name in list {
        assert!(seen.insert(*name), "{name} is listed twice");
    }
}

fn assert_in_the_corpus(list: &[&str]) {
    for name in list {
        assert!(
            corpus::card(name).is_ok(),
            "{name} is not a card of the corpus"
        );
    }
}

#[rstest]
#[case::action(ACTION_CARDS)]
#[case::alternative(ALTERNATIVE_CARDS)]
#[case::attach(ATTACH_CORPUS_CARDS)]
#[case::cascade(CASCADE_CARDS)]
#[case::counter(COUNTER_CORPUS_CARDS)]
#[case::defining(DEFINING_CARDS)]
#[case::extra_cost(EXTRA_COST_CARDS)]
#[case::face(FACE_CARDS)]
#[case::foretell(FORETELL_CARDS)]
#[case::impulse(IMPULSE_CARDS)]
#[case::kicker(KICKED_CARDS)]
#[case::loyalty(LOYALTY_CORPUS_CARDS)]
#[case::madness(MADNESS_CARDS)]
#[case::vehicle(VEHICLE_CARDS)]
#[case::x_after(X_AFTER_CARDS)]
#[case::zone(ZONE_CORPUS_CARDS)]
fn a_corpus_list_names_cards_of_the_corpus_once(#[case] list: &[&str]) {
    assert_once(list);
    assert_in_the_corpus(list);
}

#[rstest]
#[case::combat_keyword(names(&[COMBAT_KEYWORD_CARDS]))]
#[case::evasion(names(&[EVASION_CARDS]))]
#[case::event(names(&[EVENT_CARDS]))]
#[case::family(names(&[FAMILY_CARDS]))]
#[case::prevent(names(&[PREVENT_CARDS, prevent_cards::CORPUS_CARDS]))]
#[case::protect(names(&[PROTECT_CARDS, protect_cards::CORPUS_CARDS]))]
#[case::replace(names(&[replace_cards::CORPUS_CARDS]))]
fn a_corpus_list_with_copies_names_cards_of_the_corpus_once(#[case] list: Vec<&str>) {
    assert_once(&list);
    assert_in_the_corpus(&list);
}

#[rstest]
#[case::combat_keyword(COMBAT_KEYWORD_CARDS)]
#[case::evasion(EVASION_CARDS)]
#[case::event(EVENT_CARDS)]
#[case::family(FAMILY_CARDS)]
#[case::prevent(PREVENT_CARDS)]
#[case::prevent_corpus(prevent_cards::CORPUS_CARDS)]
#[case::protect(PROTECT_CARDS)]
#[case::protect_corpus(protect_cards::CORPUS_CARDS)]
#[case::replace(replace_cards::CORPUS_CARDS)]
fn a_list_with_copies_asks_for_at_least_one_of_each(#[case] list: &[(&str, usize)]) {
    for (name, copies) in list {
        assert!(*copies >= 1, "{name} is listed with no copies");
    }
}

#[rstest]
#[case::attach(ATTACH_SYNTHETIC_CARDS)]
#[case::counter(COUNTER_SYNTHETIC_CARDS)]
#[case::layer(LAYER_CARDS)]
#[case::loyalty(LOYALTY_SYNTHETIC_CARDS)]
#[case::modifier(MODIFIER_CARDS)]
#[case::nth(NTH_CARDS)]
#[case::rule(RULE_CARDS)]
#[case::statics(STATIC_CARDS)]
#[case::x(X_CARDS)]
#[case::zone(ZONE_SYNTHETIC_CARDS)]
fn a_synthetic_list_names_each_card_once(#[case] list: &[&str]) {
    assert_once(list);
}
