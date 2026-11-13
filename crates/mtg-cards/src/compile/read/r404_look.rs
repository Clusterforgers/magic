//! r40-4 looks at face-down permanents: "Look at target face-down creature." (`Effect::LookAtFaceDown`), written by the
//! renderer as "look at" and the object.

use super::effects::object;
use super::tokens::Reader;
use crate::effect::Effect;
use crate::refs::{Pred, Targetable};

/// "look at <a face-down object>": the object must be a target that only face-down permanents match.
pub(super) fn r404_look_face_down(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("look at")?;
    let what = object(r)?;
    let face_down = r.targets.iter().any(|spec| {
        matches!(&spec.what, Targetable::Object(selector) if selector.0.contains(&Pred::FaceDown))
    });
    face_down.then_some(vec![Effect::LookAtFaceDown { what }])
}
