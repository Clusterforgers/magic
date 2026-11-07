//! Goal: `UntapUpTo$` lets the player untap fewer only when Oracle says "up to" (Rising Waters untaps exactly one), and
//! the Crowns' "enchanted creature and other creatures that share a creature type with it" reads as the host and the
//! creatures sharing a subtype with it; a near-miss of the Crown phrase stays a gap.
use rstest::rstest;
use serde_json::Value;

use super::{abilities, gaps};

fn read(text: &str) -> String {
    assert!(gaps(text).is_empty(), "{:?}", gaps(text));
    Value::Array(abilities(text)).to_string()
}

const UNTAP: &str =
    "SVar:TrigUntap:DB$ Untap | UntapUpTo$ True | UntapType$ Land.YouCtrl | Amount$ 1";

#[rstest]
#[case::a_land("you untap a land you control.", false)]
#[case::up_to("Untap up to one land you control.", true)]
#[case::player_up_to("that player untaps up to one land.", true)]
fn fix16_untap_up_to_follows_oracle(#[case] oracle: &str, #[case] fewer: bool) {
    let text = format!(
        "Name:T\nManaCost:U\nTypes:Enchantment\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigUntap | TriggerDescription$ x\n{UNTAP}\nOracle:{oracle}\n"
    );
    assert_eq!(read(&text).contains(r#""min":0"#), fewer, "{}", read(&text));
}

const CROWN: &str =
    "AttachedBy Sacrificed & Valid Creature.sharesCreatureTypeWith AttachedBy Sacrificed";

#[rstest]
#[case::crown(CROWN, true)]
#[case::other_word(
    "AttachedBy Sacrificed & Valid Creature.sharesColorWith AttachedBy Sacrificed",
    false
)]
fn fix16_a_crown_names_its_host_and_its_kin(#[case] defined: &str, #[case] read_whole: bool) {
    let text = format!(
        "Name:T\nManaCost:W\nTypes:Enchantment Aura\nK:Enchant:Creature\nA:AB$ Pump | Cost$ Sac<1/CARDNAME> | Defined$ {defined} | KW$ Flying | SpellDescription$ x\nOracle:Enchant creature\nx\n"
    );
    assert_eq!(gaps(&text).is_empty(), read_whole, "{:?}", gaps(&text));
    if read_whole {
        let expected = r#"{"Each":[{"Type":"Creature"},{"AnyOf":[{"Is":"Attached"},{"SharesSubtype":"Attached"}]}]}"#;
        assert!(read(&text).contains(expected), "{}", read(&text));
    }
}
