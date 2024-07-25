use crate::primitives::{Primitives, PolyType};

/// Triangulates the quads in a Primitive in the correct way for
/// FB_ngon_encoding to reconstruct them.
pub fn encode_ngons(mut prims: Primitives) -> Primitives {
    assert!(prims.poly_type == PolyType::TrisAndQuads);

    let mut tris = Vec::<u16>::with_capacity(prims.indices.len());
    for call in prims.draw_calls.iter_mut() {
        let mut tris_range = tris.len()..tris.len();

        // Pick an index in each face and emit a triangulation of the face with
        // that index as the first index of each tri. Don't pick the same index
        // for two subsequent faces.
        // TODO: handle degenerate faces? Probably low priority...
        let mut last_face_index: u16 = 0xffff;
        for face in prims.indices[call.index_range.clone()].chunks_exact(4) {
            tris.reserve(6);
            if last_face_index != face[0] {
                last_face_index = face[0];

                tris.push(face[0]);
                tris.push(face[1]);
                tris.push(face[2]);
                if face[3] != 0xffff {
                    tris.push(face[0]);
                    tris.push(face[2]);
                    tris.push(face[3]);
                }
            } else {
                last_face_index = face[2];

                tris.push(face[2]);
                tris.push(face[0]);
                tris.push(face[1]);
                if face[3] != 0xffff {
                    tris.push(face[2]);
                    tris.push(face[3]);
                    tris.push(face[0]);
                }
            }
        }

        tris_range.end = tris.len();

        call.index_range = tris_range;
    }

    prims.indices = tris;
    prims.poly_type = PolyType::Tris;

    prims
}
