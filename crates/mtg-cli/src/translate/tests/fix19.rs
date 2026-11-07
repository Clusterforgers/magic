//! Goal: a "you may cast" whose cast asks on its own (CR 608.2g) is asked once: no `May` wraps a `CastFromZone` and its
//! exile shield, while a "may" around other steps keeps its wrapper.
use rstest::rstest;
use serde_json::Value;

use super::{abilities, gaps};

fn read(text: &str) -> String {
    assert!(gaps(text).is_empty(), "{:?}", gaps(text));
    Value::Array(abilities(text)).to_string()
}

const EFREET: &str = "Name:T\nManaCost:3 R\nTypes:Creature Efreet\nPT:1/4\nT:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Player | CombatDamage$ True | Execute$ TrigPlay | TriggerZones$ Battlefield | OptionalDecider$ You | TriggerDescription$ Whenever CARDNAME deals combat damage to a player, you may cast target instant or sorcery card from your graveyard without paying its mana cost. If that spell would be put into your graveyard, exile it instead.\nSVar:TrigPlay:DB$ Play | TgtZone$ Graveyard | ValidTgts$ Instant.YouOwn,Sorcery.YouOwn | TgtPrompt$ x | ValidSA$ Spell | WithoutManaCost$ True | Optional$ True | ReplaceGraveyard$ Exile\nOracle:Whenever this creature deals combat damage to a player, you may cast target instant or sorcery card from your graveyard without paying its mana cost. If that spell would be put into your graveyard, exile it instead.\n";

const DRAWER: &str = "Name:T\nManaCost:1\nTypes:Creature Elf\nPT:1/1\nT:Mode$ DamageDone | ValidSource$ Card.Self | ValidTarget$ Player | CombatDamage$ True | Execute$ TrigDraw | TriggerZones$ Battlefield | OptionalDecider$ You | TriggerDescription$ Whenever CARDNAME deals combat damage to a player, you may draw a card.\nSVar:TrigDraw:DB$ Draw | Defined$ You | NumCards$ 1\nOracle:Whenever this creature deals combat damage to a player, you may draw a card.\n";

#[rstest]
#[case::cast_asks_once(EFREET, false, true)]
#[case::draw_keeps_its_may(DRAWER, true, false)]
fn fix19_a_cast_from_a_zone_is_not_wrapped_in_a_may(
    #[case] text: &str,
    #[case] may: bool,
    #[case] cast: bool,
) {
    let found = read(text);
    assert_eq!(found.contains(r#""May":"#), may, "{found}");
    assert_eq!(found.contains(r#""CastFromZone":"#), cast, "{found}");
    if cast {
        assert!(found.contains(r#""Shield":"#), "{found}");
    }
}
