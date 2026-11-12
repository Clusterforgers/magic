//! Goal: the r373 readers and writers read and write the card's words and nothing near them: loyalty activations twice each turn (Oath of Teferi), at instant speed (Teferi's emblem), at an additional [+N] (Carth the Lion) and once more for each planeswalker (The Chain Veil); Drain Life's gain of the damage dealt up to what its recipient had; Doubling Cube's doubled pool. Another player group, a zero cost, another duration and the plain damage dealt this way do not read back.
use super::*;

fn rule(rule: &str) -> Ability {
    ability(&format!(r#"{{"Rule":{{"rule":{rule}}}}}"#))
}

const OATH: &str =
    "You may activate the loyalty abilities of planeswalkers you control twice each turn rather than only once.";
const EMBLEM: &str = "You may activate loyalty abilities of planeswalkers you control on any player's turn any time you could cast an instant.";
const CARTH: &str = "Planeswalkers' loyalty abilities you activate cost an additional [+1] to activate.";
const CARTH_TWO: &str =
    "Planeswalkers' loyalty abilities you activate cost an additional [+2] to activate.";

#[rstest]
#[case::oath(r#"{"LoyaltyTwice":"You"}"#, OATH, true)]
#[case::oath_of_opponents(r#"{"LoyaltyTwice":"Opponents"}"#, OATH, false)]
#[case::oath_as_once_more(r#"{"LoyaltyOnceMore":"You"}"#, OATH, false)]
#[case::emblem(r#"{"LoyaltyAsInstant":"You"}"#, EMBLEM, true)]
#[case::emblem_of_each_player(r#"{"LoyaltyAsInstant":"EachPlayer"}"#, EMBLEM, false)]
#[case::carth(r#"{"LoyaltyCostMore":{"who":"You","plus":1}}"#, CARTH, true)]
#[case::carth_of_two(r#"{"LoyaltyCostMore":{"who":"You","plus":2}}"#, CARTH_TWO, true)]
#[case::carth_of_two_as_one(r#"{"LoyaltyCostMore":{"who":"You","plus":2}}"#, CARTH, false)]
#[case::carth_of_nothing(r#"{"LoyaltyCostMore":{"who":"You","plus":0}}"#, CARTH, false)]
fn r373_loyalty_rules_write_as_printed(
    #[case] json: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&rule(json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

#[rstest]
#[case::oath(OATH, r#"{"LoyaltyTwice":"You"}"#)]
#[case::emblem(EMBLEM, r#"{"LoyaltyAsInstant":"You"}"#)]
#[case::carth(CARTH, r#"{"LoyaltyCostMore":{"who":"You","plus":1}}"#)]
fn r373_loyalty_rules_read_from_their_words(#[case] text: &str, #[case] json: &str) {
    assert_eq!(golden(text, ClauseKind::Static, false), rule(json));
}

#[rstest]
#[case::once_more_than_printed("You may activate the loyalty abilities of planeswalkers you control three times each turn rather than only once.")]
#[case::opponents("Your opponents may activate loyalty abilities of planeswalkers they control on any player's turn any time they could cast an instant.")]
#[case::without_a_bracket("Planeswalkers' loyalty abilities you activate cost an additional +1 to activate.")]
fn r373_near_loyalty_wordings_stay_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Static, false), None);
}

const VEIL: &str = "{4}, {T}: For each planeswalker you control, you may activate one of its loyalty abilities once this turn as though none of its loyalty abilities have been activated this turn.";

fn veil(duration: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{4}}"}},"Tap"],"effects":[{{"Rule":{{"rule":{{"LoyaltyOnceMore":"You"}},"duration":"{duration}"}}}}]}}}}"#
    ))
}

#[rstest]
#[case::veil("UntilEndOfTurn", true)]
#[case::veil_until_your_next_turn("UntilYourNextTurn", false)]
fn r373_the_chain_veil_writes_as_printed(#[case] duration: &str, #[case] reads: bool) {
    let found = written_variants(&veil(duration));
    assert_eq!(found.iter().any(|w| same_text(w, VEIL)), reads, "{found:?}");
}

#[test]
fn r373_the_chain_veil_reads_from_its_words() {
    assert_eq!(golden(VEIL, ClauseKind::Activated, false), veil("UntilEndOfTurn"));
}

const DRAIN: &str = "~ deals X damage to any target. You gain life equal to the damage dealt, but not more life than the player's life total before the damage was dealt, the planeswalker's loyalty before the damage was dealt, or the creature's toughness.";

fn drain(gain: &str) -> Ability {
    ability(&format!(
        r#"{{"Spell":{{"targets":[{{"what":"AnyTarget"}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":"X"}}}},{{"GainLife":{{"amount":"{gain}"}}}}]}}}}"#
    ))
}

#[rstest]
#[case::up_to_health("DealtUpToHealth", true)]
#[case::all_the_damage("DamageDealtThisWay", false)]
fn r373_drain_life_writes_as_printed(#[case] gain: &str, #[case] reads: bool) {
    let found = written_variants(&drain(gain));
    assert_eq!(found.iter().any(|w| same_text(w, DRAIN)), reads, "{found:?}");
}

#[test]
fn r373_drain_life_reads_from_its_words() {
    assert_eq!(golden(DRAIN, ClauseKind::Spell, true), drain("DealtUpToHealth"));
}

const CUBE: &str = "{3}, {T}: Double the amount of each type of unspent mana you have.";

fn cube(mana: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{3}}"}},"Tap"],"effects":[{{"AddMana":{{"mana":{mana}}}}}]}}}}"#
    ))
}

#[rstest]
#[case::cube(r#""Unspent""#, true)]
#[case::devotion(r#""Devotion""#, false)]
fn r373_doubling_cube_writes_as_printed(#[case] mana: &str, #[case] reads: bool) {
    let found = written_variants(&cube(mana));
    assert_eq!(found.iter().any(|w| same_text(w, CUBE)), reads, "{found:?}");
}

const FLAGBEARER: &str = "While an opponent is choosing targets as part of casting a spell they control or activating an ability they control, that player must choose at least one Flagbearer on the battlefield if able.";

#[rstest]
#[case::flagbearer(r#"{"MustTarget":{"who":"Opponents","what":[{"Subtype":"Flagbearer"}]}}"#, true)]
#[case::flagbearer_for_you(r#"{"MustTarget":{"who":"You","what":[{"Subtype":"Flagbearer"}]}}"#, false)]
#[case::soldier(r#"{"MustTarget":{"who":"Opponents","what":[{"Subtype":"Soldier"}]}}"#, false)]
fn r373_the_flagbearer_rule_writes_as_printed(#[case] json: &str, #[case] reads: bool) {
    let found = written_variants(&rule(json));
    assert_eq!(found.iter().any(|w| same_text(w, FLAGBEARER)), reads, "{found:?}");
}

#[test]
fn r373_the_flagbearer_rule_reads_from_its_words() {
    assert_eq!(
        golden(FLAGBEARER, ClauseKind::Static, false),
        rule(r#"{"MustTarget":{"who":"Opponents","what":[{"Subtype":"Flagbearer"}]}}"#)
    );
}

const MASK: &str = "As long as a card exiled with ~ has flying, ~ has flying. The same is true for fear, first strike, double strike, haste, landwalk, protection, and trample.";
const EATER: &str = "As long as a card exiled with ~ has flying, equipped creature has flying. The same is true for first strike, double strike, deathtouch, haste, hexproof, indestructible, lifelink, menace, protection, reach, trample, and vigilance.";

fn keywords_of(affected: &str, from: &str, names: &str) -> Ability {
    ability(&format!(
        r#"{{"Static":{{"affected":"{affected}","mods":[{{"KeywordsOf":{{"from":{from},"names":{names}}}}}]}}}}"#
    ))
}

const EXILED_WITH: &str = r#"{"Each":[{"In":"Exile"},{"ExiledWith":"This"}]}"#;
const MASK_NAMES: &str = r#"["Flying","Fear","First strike","Double strike","Haste","Landwalk","Protection","Trample"]"#;
const EATER_NAMES: &str = r#"["Flying","First strike","Double strike","Deathtouch","Haste","Hexproof","Indestructible","Lifelink","Menace","Protection","Reach","Trample","Vigilance"]"#;

#[rstest]
#[case::mask(keywords_of("This", EXILED_WITH, MASK_NAMES), MASK, true)]
#[case::mask_without_fear(keywords_of("This", EXILED_WITH, r#"["Flying","First strike","Double strike","Haste","Landwalk","Protection","Trample"]"#), MASK, false)]
#[case::mask_of_graveyard_cards(keywords_of("This", r#"{"Each":[{"In":"Graveyard"}]}"#, MASK_NAMES), MASK, false)]
#[case::eater(keywords_of("Attached", EXILED_WITH, EATER_NAMES), EATER, true)]
#[case::eater_on_itself(keywords_of("This", EXILED_WITH, EATER_NAMES), EATER, false)]
fn r373_keywords_of_exiled_cards_write_as_printed(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

#[rstest]
#[case::flagbearer(r#"{"Static":{"affected":"Attached","mods":[{"AddSubtype":"Flagbearer"}]}}"#, "Enchanted creature is a Flagbearer.", true)]
#[case::flagbearer_of_this(r#"{"Static":{"affected":"This","mods":[{"AddSubtype":"Flagbearer"}]}}"#, "Enchanted creature is a Flagbearer.", false)]
#[case::fortified(r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Indestructible"}]}}"#, "Fortified land has indestructible.", true)]
#[case::fortified_of_this(r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Indestructible"}]}}"#, "Fortified land has indestructible.", false)]
fn r373_attachments_write_as_printed(#[case] json: &str, #[case] text: &str, #[case] reads: bool) {
    let found = written_variants(&ability(json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}
