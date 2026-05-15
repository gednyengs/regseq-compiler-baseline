//! MIR `Place` bridge translation.

use regseq_model::{FieldIdx, LocalId, Place, PlaceElem, VariantIdx};
use rustc_middle::mir;

use crate::bridge::{ty::translate_ty, BridgeCtxt, BridgeError, DslRejectionKind};

use super::BodyCtxt;

/// Translate a rustc `mir::Place` into a `regseq_model::Place`. Per
/// spec.md section 19.5.
pub fn translate_place<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    _body_cx: &mut BodyCtxt<'tcx, '_>,
    place: mir::Place<'tcx>,
) -> Result<Place, BridgeError> {
    let local = LocalId::from_u32(place.local.as_u32());
    let projection = place
        .projection
        .iter()
        .map(|elem| translate_projection_elem(cx, elem))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Place {
        local,
        projection: projection.into_boxed_slice(),
    })
}

fn translate_projection_elem<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    elem: mir::PlaceElem<'tcx>,
) -> Result<PlaceElem, BridgeError> {
    match elem {
        mir::ProjectionElem::Deref => Ok(PlaceElem::Deref),
        mir::ProjectionElem::Field(field, ty) => Ok(PlaceElem::Field(
            FieldIdx::from_u32(field.as_u32()),
            translate_ty(cx, ty)?,
        )),
        mir::ProjectionElem::Index(local) => {
            Ok(PlaceElem::Index(LocalId::from_u32(local.as_u32())))
        }
        mir::ProjectionElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        } => Ok(PlaceElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        }),
        mir::ProjectionElem::Subslice { from, to, from_end } => {
            Ok(PlaceElem::Subslice { from, to, from_end })
        }
        mir::ProjectionElem::Downcast(_, variant) => {
            Ok(PlaceElem::Downcast(VariantIdx::from_u32(variant.as_u32())))
        }
        mir::ProjectionElem::OpaqueCast(_) | mir::ProjectionElem::UnwrapUnsafeBinder(_) => {
            Err(BridgeError::DslRejection {
                kind: DslRejectionKind::PlaceProjection,
                ty_repr: format!("{elem:?}"),
            })
        }
    }
}
