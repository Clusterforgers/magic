//! Goal: round 11 pack g11c writes the block tax, the attack tax of one creature, the two as one sentence and the draws
//! of a draw step as the card prints them; another payer, price, creature, attacker or draw does not read back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn paired(first: &str, second: &str, clause: &str) -> bool {
    super::super::static_pairs::texts(&ability(first), &ability(second))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn block_tax(fields: &str) -> String {
    format!(r#"{{"Rule":{{"rule":{{"BlockTax":{{{fields}}}}}}}}}"#)
}

fn self_attack(creature: &str, price: &str) -> String {
    format!(
        r#"{{"Rule":{{"rule":{{"AttackTax":{{"attacked":"Both","attackers":[{creature}],"who":"EachPlayer",{price}}}}}}}}}"#
    )
}

const ATTACKING: &str =
    r#"{"Compare":{"left":{"Count":["This","Attacking"]},"op":"Ge","right":1}}"#;
const TITHES: &str = "As long as ~ is attacking, creatures can't block unless their controller pays {1} for each of those creatures.";
const HIPPARION: &str = "~ can't block creatures with power 3 or greater unless you pay {1}.";
const PRESENCE: &str = "Enchanted creature can't be blocked unless defending player pays {3} for each creature they control that's blocking it.";
const HEAT_WAVE: &str = "Nonblue creatures can't block creatures you control unless their controller pays 1 life for each blocking creature they control.";
const POWER_3: &str = r#"[{"Type":"Creature"},{"Compare":{"op":"Ge","stat":"Power","to":3}}]"#;
const NONBLUE: &str = r#"[{"Type":"Creature"},{"Not":{"Color":"Blue"}}]"#;
const YOURS: &str = r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#;

fn tithes(fields: &str, condition: &str) -> String {
    format!(r#"{{"Rule":{{"condition":{condition},"rule":{{"BlockTax":{{{fields}}}}}}}}}"#)
}

#[rstest]
#[case::tithes(tithes(r#""mana":"{1}""#, ATTACKING), TITHES, true)]
#[case::tithes_other_price(tithes(r#""mana":"{2}""#, ATTACKING), TITHES, false)]
#[case::tithes_only_its_own(
    tithes(r#""mana":"{1}","blockers":["This"]"#, ATTACKING),
    TITHES,
    false
)]
#[case::hipparion(block_tax(&format!(r#""attackers":{POWER_3},"blockers":["This"],"mana":"{{1}}""#)), HIPPARION, true)]
#[case::hipparion_any_attacker(block_tax(r#""blockers":["This"],"mana":"{1}""#), HIPPARION, false)]
#[case::hipparion_other_creatures(block_tax(&format!(r#""attackers":{POWER_3},"mana":"{{1}}""#)), HIPPARION, false)]
#[case::presence(
    block_tax(r#""attackers":[{"Is":"Attached"}],"mana":"{3}""#),
    PRESENCE,
    true
)]
#[case::presence_its_blockers_not_its_attackers(
    block_tax(r#""blockers":[{"Is":"Attached"}],"mana":"{3}""#),
    PRESENCE,
    false
)]
#[case::heat_wave(block_tax(&format!(r#""attackers":{YOURS},"blockers":{NONBLUE},"life":1"#)), HEAT_WAVE, true)]
#[case::heat_wave_mana_is_not_life(block_tax(&format!(r#""attackers":{YOURS},"blockers":{NONBLUE},"mana":"{{1}}""#)), HEAT_WAVE, false)]
#[case::heat_wave_any_attacker(block_tax(&format!(r#""blockers":{NONBLUE},"life":1"#)), HEAT_WAVE, false)]
fn g11c_block_taxes_write_as_printed(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        written(&json, text),
        reads,
        "{:?}",
        written_variants(&ability(&json))
    );
}

const BEHEMOTH: &str = "~ can't attack or block unless you pay {2}.";
const RAYS: &str = "Enchanted creature can't attack or block unless its controller pays {3}.";
const MYR: &str = "~ can't attack or block unless you pay {1} for each +1/+1 counter on it.";
const COWED: &str = "Enchanted creature can't attack or block unless its controller pays {1} for each card in your hand.";
const THIS: &str = r#"{"Is":"This"}"#;
const ATTACHED: &str = r#"{"Is":"Attached"}"#;
const COUNTERS: &str = r#""mana":"{1}","per":{"Counters":{"counter":"+1/+1","on":"This"}}"#;
const HAND: &str = r#""mana":"{1}","per":{"Count":[{"OwnedBy":"You"},{"In":"Hand"}]}"#;

#[rstest]
#[case::behemoth(
    self_attack(THIS, r#""mana":"{2}""#),
    block_tax(r#""blockers":["This"],"mana":"{2}""#),
    BEHEMOTH,
    true
)]
#[case::behemoth_other_block_price(
    self_attack(THIS, r#""mana":"{2}""#),
    block_tax(r#""blockers":["This"],"mana":"{1}""#),
    BEHEMOTH,
    false
)]
#[case::behemoth_every_blocker(
    self_attack(THIS, r#""mana":"{2}""#),
    block_tax(r#""mana":"{2}""#),
    BEHEMOTH,
    false
)]
#[case::rays(
    self_attack(ATTACHED, r#""mana":"{3}""#),
    block_tax(r#""blockers":[{"Is":"Attached"}],"mana":"{3}""#),
    RAYS,
    true
)]
#[case::rays_is_not_the_aura(
    self_attack(THIS, r#""mana":"{3}""#),
    block_tax(r#""blockers":["This"],"mana":"{3}""#),
    RAYS,
    false
)]
#[case::myr(self_attack(THIS, COUNTERS), block_tax(&format!(r#""blockers":["This"],{COUNTERS}"#)), MYR, true)]
#[case::myr_once(
    self_attack(THIS, r#""mana":"{1}""#),
    block_tax(r#""blockers":["This"],"mana":"{1}""#),
    MYR,
    false
)]
#[case::cowed(self_attack(ATTACHED, HAND), block_tax(&format!(r#""blockers":[{{"Is":"Attached"}}],{HAND}"#)), COWED, true)]
fn g11c_attack_and_block_taxes_of_one_creature_write_as_one_sentence(
    #[case] attack: String,
    #[case] block: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        paired(&attack, &block, text),
        reads,
        "{:?}",
        super::super::static_pairs::texts(&ability(&attack), &ability(&block))
    );
}

fn draw_trigger(when: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"PutCounters":{{"counter":"+1/+1","on":"This"}}}}],"when":{{"Drew":{when}}}}}}}"#
    )
}

const LEELA: &str = "Whenever an opponent draws a card except the first one they draw in each of their draw steps, put a +1/+1 counter on ~.";
const FIRST: &str =
    "Whenever you draw your first card during each of your draw steps, put a +1/+1 counter on ~.";

#[rstest]
#[case::except_the_first(r#"{"first_in_draw_step":false,"who":"Opponents"}"#, LEELA, true)]
#[case::every_card_is_not_except(r#"{"who":"Opponents"}"#, LEELA, false)]
#[case::the_first_is_not_except(r#"{"first_in_draw_step":true,"who":"Opponents"}"#, LEELA, false)]
#[case::yours_are_not_theirs(r#"{"first_in_draw_step":false,"who":"You"}"#, LEELA, false)]
#[case::the_first(r#"{"first_in_draw_step":true,"who":"You"}"#, FIRST, true)]
#[case::except_is_not_the_first(r#"{"first_in_draw_step":false,"who":"You"}"#, FIRST, false)]
fn g11c_draws_of_a_draw_step_write_and_read_as_printed(
    #[case] when: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let json = draw_trigger(when);
    assert_eq!(
        written(&json, text),
        reads,
        "{:?}",
        written_variants(&ability(&json))
    );
    let read = clause(text, ClauseKind::Triggered, false);
    assert_eq!(read.as_ref() == Some(&ability(&json)), reads, "{read:?}");
}
