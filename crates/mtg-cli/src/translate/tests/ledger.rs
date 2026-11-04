//! Goal: the ledger names, for every script line, the abilities it gave (as indices into its face's proposal) or the construct that stopped it, so each ability is held by a row and a gap row never holds an ability.
use mtg_cards::proposal::partial::LedgerLine;
use rstest::rstest;

use super::translate;

const MIXED: &str = "Name:T\nTypes:Creature Elf\nK:Flying\nA:AB$ RollDice | Cost$ T | Sides$ 6 | SpellDescription$ Roll a die.\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ Draw | TriggerDescription$ Whenever CARDNAME attacks, draw a card.\nSVar:Draw:DB$ Draw\nK:Trample\n";
const JOINED_TRIGGERS: &str = "Name:T\nTypes:Creature Dog\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Go | TriggerDescription$ When CARDNAME enters or attacks, draw a card.\nT:Mode$ Attacks | ValidCard$ Card.Self | Execute$ Go | Secondary$ True | TriggerDescription$ When CARDNAME enters or attacks, draw a card.\nSVar:Go:DB$ Draw\n";
const JOINED_STATICS: &str = "Name:T\nTypes:Enchantment Aura\nK:Enchant creature\nS:Mode$ Continuous | Affected$ Creature.EnchantedBy | AddPower$ 2 | AddToughness$ 2 | Description$ Enchanted creature gets +2/+2 and can't block.\nS:Mode$ CantBlock | ValidCard$ Creature.EnchantedBy | Secondary$ True | Description$ Enchanted creature can't block.\n";
const TWO_FACES: &str = "Name:Front\nTypes:Creature Elf\nK:Flying\nALTERNATE\nName:Back\nTypes:Creature Goblin\nK:Haste\nA:AB$ RollDice | Cost$ T | Sides$ 6 | SpellDescription$ Roll a die.\n";

#[rstest]
#[case::lines_in_order(MIXED, &[
    ("K", "Flying", &[0][..], false),
    ("K", "Trample", &[1], false),
    ("A", "Roll a die.", &[], true),
    ("T", "Whenever CARDNAME attacks, draw a card.", &[2], false),
])]
fn each_line_has_a_row(#[case] script: &str, #[case] expected: &[(&str, &str, &[usize], bool)]) {
    let got: Vec<(String, String, Vec<usize>, bool)> = translate(script)
        .ledger()
        .into_iter()
        .map(|row| (row.tag, row.text, row.abilities, row.gap.is_some()))
        .collect();
    let want: Vec<(String, String, Vec<usize>, bool)> = expected
        .iter()
        .map(|(tag, text, abilities, gap)| {
            (
                (*tag).to_owned(),
                (*text).to_owned(),
                abilities.to_vec(),
                *gap,
            )
        })
        .collect();
    assert_eq!(got, want);
}

#[rstest]
#[case::mixed(MIXED)]
#[case::joined_triggers(JOINED_TRIGGERS)]
#[case::joined_statics(JOINED_STATICS)]
#[case::two_faces(TWO_FACES)]
fn every_ability_belongs_to_a_row_and_no_gap_row_holds_one(#[case] script: &str) {
    let card = translate(script);
    let rows = card.ledger();
    for (face, out) in card.faces.iter().enumerate() {
        for at in 0..out.abilities.len() {
            let holders = rows
                .iter()
                .filter(|row| row.face == face && row.abilities.contains(&at))
                .count();
            assert!(holders >= 1, "ability {at} of face {face} has no row");
        }
        let gap_rows = rows
            .iter()
            .filter(|row| row.face == face && row.gap.is_some())
            .count();
        assert!(gap_rows >= out.gaps.len());
    }
    assert!(
        rows.iter()
            .all(|row| row.gap.is_none() || row.abilities.is_empty())
    );
}

#[test]
fn lines_joined_into_one_ability_share_its_row_data() {
    for script in [JOINED_TRIGGERS, JOINED_STATICS] {
        let all = translate(script).ledger();
        let joined: Vec<&LedgerLine> = all.iter().filter(|row| row.tag != "K").collect();
        assert_eq!(joined.len(), 2);
        assert_eq!(joined[0].abilities, joined[1].abilities);
        assert_eq!(joined[0].abilities.len(), 1);
    }
}

#[test]
fn the_face_of_each_row_is_the_face_of_its_line() {
    let faces: Vec<(usize, String)> = translate(TWO_FACES)
        .ledger()
        .into_iter()
        .map(|row| (row.face, row.text))
        .collect();
    assert_eq!(
        faces,
        vec![
            (0, "Flying".to_owned()),
            (1, "Haste".to_owned()),
            (1, "Roll a die.".to_owned())
        ]
    );
}
