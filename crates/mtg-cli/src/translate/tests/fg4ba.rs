//! Goal: the alternative costs of Forge scripts become `AltCast` and `FreeCast` abilities that parse as the DSL and leave other shapes as gaps.
use mtg_cards::ability::Ability;
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn creature(lines: &str) -> String {
    format!("Name:T\nManaCost:3 R\nTypes:Creature Elf\nPT:3/3\n{lines}\n")
}

#[rstest]
#[case::warp("K:Warp:1 R", "Warp", 2)]
#[case::surge("K:Surge:3 R R", "Surge", 1)]
#[case::web("K:Web-slinging:2 W", "WebSlinging", 1)]
#[case::impending("K:Impending:3:2 U U", "Impending", 3)]
fn keyword_alt_costs(#[case] line: &str, #[case] way: &str, #[case] count: usize) {
    let found = abilities(&creature(line));
    assert_eq!(found.len(), count);
    assert_eq!(found[0]["AltCast"]["way"], json!(way));
    assert!(
        found
            .iter()
            .all(|a| serde_json::from_value::<Ability>(a.clone()).is_ok())
    );
}

#[rstest]
#[case::free(
    "S:Mode$ AlternativeCost | ValidSA$ Spell.Self | EffectZone$ All | Cost$ 0",
    json!({"FreeCast": {}})
)]
#[case::sac(
    "S:Mode$ AlternativeCost | ValidSA$ Spell.Self | EffectZone$ All | Cost$ Sac<1/Plains>",
    json!({"FreeCast": {"cost": [{"Sacrifice": {"filter": [{"Subtype": "Plains"}]}}]}})
)]
#[case::graveyard(
    "S:Mode$ AlternativeCost | ValidSA$ Spell.Self | EffectZone$ Graveyard | Cost$ 2 W",
    json!({"AltCast": {"way": "Alternative", "from": "Graveyard", "cost": "{2}{W}"}})
)]
fn static_alt_costs(#[case] line: &str, #[case] expected: Value) {
    let found = abilities(&creature(line));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0], expected);
    assert!(serde_json::from_value::<Ability>(found[0].clone()).is_ok());
}

#[rstest]
#[case::other_spells(
    "S:Mode$ AlternativeCost | ValidSA$ Spell | ValidPlayer$ You | Cost$ 1",
    "static:AlternativeCost-shape"
)]
#[case::restriction(
    "S:Mode$ AlternativeCost | ValidSA$ Spell.Self | EffectZone$ All | Cost$ R G | ManaRestriction$ Treasure",
    "key:Static.AlternativeCost.ManaRestriction"
)]
fn static_alt_gaps(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&creature(line)), vec![gap.to_owned()]);
}
