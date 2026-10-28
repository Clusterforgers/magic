//! An enchantment that becomes an Aura on a manifested card: "it becomes an Aura with enchant creature. Manifest the
//! top card of your library and attach this enchantment to it." (Cloudform, Lightform).

use mtg_core::card::{CardType, Subtype};

use super::tokens::Reader;
use crate::effect::Effect;
use crate::keyword::Keyword;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Selector};

/// The name the manifested permanent is bound to.
pub const C6_BLK_MANIFESTED: &str = "manifested";

/// The three effects: the card becomes an Aura with enchant creature for good, a manifest bound as
/// [`C6_BLK_MANIFESTED`], and the card attached to it.
pub fn c6_blk_aura_effects() -> Vec<Effect> {
    vec![
        Effect::Continuous {
            affected: Ref::This,
            mods: vec![
                Modification::AddSubtype(Subtype("Aura".to_owned())),
                Modification::AddKeyword(Keyword::Enchant(Selector(vec![Pred::Type(
                    CardType::Creature,
                )]))),
            ],
            duration: Duration::Forever,
        },
        Effect::Manifest {
            who: Ref::You,
            count: crate::value::Amount::one(),
            from: None,
            bind: Some(C6_BLK_MANIFESTED.to_owned()),
        },
        Effect::Attach {
            what: Ref::This,
            to: Ref::Bound(C6_BLK_MANIFESTED.to_owned()),
        },
    ]
}

/// "it becomes an Aura with enchant creature. Manifest the top card of your library and attach ~ to it", in a trigger
/// of the card itself.
pub fn c6_blk_aura_manifest(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.acts_itself.then_some(())?;
    r.need("it becomes an")?;
    (r.name()? == "Aura").then_some(())?;
    r.need("with enchant creature")?;
    r.punct('.').then_some(())?;
    r.need("manifest the top card of your library and attach")?;
    r.source().then_some(())?;
    r.need("to it")?;
    Some(c6_blk_aura_effects())
}
