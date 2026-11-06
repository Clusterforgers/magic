//! Goal: the gap pack of round 6 builder e stays right: a card taken from exile is any player's unless the text says whose, a slug two stored names share goes to the plainer name, the one Forge scripts, and a destroy of the targeted reads every target the lines before only chose.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

/// The translation of a card with the given header and lines as compact JSON, and its gaps.
fn card(types: &str, lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:{types}\n{lines}\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

#[rstest]
// Sengir Nosferatu's Bat.
#[case::an_exiled_card(
    "Return an exiled card named Sengir Nosferatu to the battlefield under its owner's control",
    false
)]
#[case::a_card_in_exile(
    "Put a creature card in exile onto the battlefield under its owner's control",
    false
)]
#[case::you_own("Return a creature card you own in exile to the battlefield", true)]
#[case::that_player_owns(
    "Put a creature card that player owns from exile onto the battlefield",
    true
)]
fn a_card_from_exile_is_anyones_unless_the_text_says_whose(
    #[case] words: &str,
    #[case] owned: bool,
) {
    let (json, found) = card(
        "Creature Bat",
        &format!(
            "PT:1/2\nA:AB$ ChangeZone | Cost$ 1 B Sac<1/CARDNAME> | ChangeType$ Creature | ChangeNum$ 1 | Origin$ Exile | Destination$ Battlefield | Hidden$ True | SpellDescription$ {words}.\nOracle:{{1}}{{B}}, Sacrifice this creature: {words}."
        ),
    );
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(json.contains(r#"{"OwnedBy":"You"}"#), owned, "{json}");
}

#[rstest]
#[case::the_plain_name_over_a_spell_commander("Lava Axe", "Lava, Axe", true)]
#[case::a_spell_commander_over_the_plain_name("Lava, Axe", "Lava Axe", false)]
#[case::equals_keep_their_place("Lava Axe", "Lava Axe", false)]
#[case::an_apostrophe_counts("Ajanis Pridemate", "Ajani's Pridemate", true)]
fn a_shared_slug_goes_to_the_plainer_name(
    #[case] name: &str,
    #[case] kept: &str,
    #[case] plainer: bool,
) {
    assert_eq!(crate::translate::stored::plainer(name, kept), plainer);
}

#[rstest]
// Stomp and Howl: two chosen targets, then the destroy of both.
#[case::two_chosen(
    "A:SP$ Pump | ValidTgts$ Artifact | AILogic$ Destroy | IsCurse$ True | SubAbility$ DBEnch | SpellDescription$ Destroy target artifact and target enchantment.\nSVar:DBEnch:DB$ Pump | ValidTgts$ Enchantment | AILogic$ Destroy | IsCurse$ True | SubAbility$ DBDestroy\nSVar:DBDestroy:DB$ Destroy | Defined$ Targeted",
    Some(
        r#"{"Together":{"effects":[{"Destroy":{"what":{"Target":0}}},{"Destroy":{"what":{"Target":1}}}]}}"#
    )
)]
// The destroy with a target of its own reads only that one; the chosen one stays unread.
#[case::own_target(
    "A:SP$ Pump | ValidTgts$ Artifact | AILogic$ Destroy | IsCurse$ True | SubAbility$ DBDestroy | SpellDescription$ Destroy target artifact and target enchantment.\nSVar:DBDestroy:DB$ Destroy | ValidTgts$ Enchantment",
    None
)]
// A pump that does something is no choice alone.
#[case::a_real_pump(
    "A:SP$ Pump | ValidTgts$ Creature | NumAtt$ +2 | SubAbility$ DBEnch | SpellDescription$ Target creature gets +2/+0 until end of turn. Destroy target enchantment.\nSVar:DBEnch:DB$ Pump | ValidTgts$ Enchantment | AILogic$ Destroy | IsCurse$ True | SubAbility$ DBDestroy\nSVar:DBDestroy:DB$ Destroy | Defined$ Targeted",
    None
)]
fn a_destroy_of_the_targeted_reads_every_target_the_lines_before_only_chose(
    #[case] lines: &str,
    #[case] together: Option<&str>,
) {
    let (json, _) = card("Sorcery", lines);
    assert_eq!(json.contains("Together"), together.is_some(), "{json}");
    if let Some(together) = together {
        assert!(json.contains(together), "{json}");
    }
}
