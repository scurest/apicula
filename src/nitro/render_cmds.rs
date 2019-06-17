//! Render commands for model files.

use errors::Result;
use util::cur::Cur;

pub struct SkinTerm {
    pub weight: f32,
    pub stack_pos: u8,
    pub inv_bind_idx: u8,
}

/// A render command is analyzed into zero or more render ops (think micro-ops
/// to a CPU instruction).
pub enum Op {
    /// cur_matrix = matrix_stack[stack_pos]
    LoadMatrix { stack_pos: u8 },
    /// matrix_stack[stack_pos] = cur_matrix
    StoreMatrix { stack_pos: u8 },
    /// cur_matrix = cur_matrix * object_matrices[object_idx]
    MulObject { object_idx: u8 },
    /// cur_matrix = ∑_{term}
    ///     term.weight *
    ///     matrix_stack[term.stack_pos] *
    ///     inv_bind_matrices[term.inv_bind_idx]
    /// (ie. the skinning equation)
    Skin { terms: Box<[SkinTerm]> },
    /// cur_matrix = cur_matrix * scale(up_scale)
    ScaleUp,
    /// cur_matrix = cur_matrix * scale(model.down_scale)
    ScaleDown,

    /// Bind materials[material_idx] for subsequent draw calls.
    BindMaterial { material_idx: u8 },

    /// Draw meshes[mesh_idx].
    Draw { mesh_idx: u8 },
}

/// Parses a bytestream of render commands into a list of render ops.
pub fn parse_render_cmds(mut cur: Cur) -> Result<Vec<Op>> {
    trace!("render commands @ {:#x}", cur.pos());

    let mut ops: Vec<Op> = vec![];

    loop {
        let (opcode, params) = next_opcode_params(&mut cur)?;
        trace!("cmd {:#2x} {:?}", opcode, params);

        match opcode {
            0x00 => {
                // NOP
            }
            0x01 => {
                // End render commands
                return Ok(ops);
            }
            0x02 => {
                // Unknown
                // Here's what I know about it:
                // * there is always one of them in a model, after the initial matrix
                //   stack setup but before the 0x03/0x04/0x05 command sequences
                // * the first parameter is num_objects - 1 (ie. the index of the last
                //   object)
                // * the second parameter is always 1; if this 1 is changed to a zero
                //   using a debugger, the model is not drawn (this is probably why
                //   other people have called this "visibility")
                // * running it emits no GPU commands
            }
            0x03 => {
                // Load a matrix from the stack
                ops.push(Op::LoadMatrix { stack_pos: params[0] });
            }
            0x04 | 0x24 | 0x44 => {
                // Bind a material
                ops.push(Op::BindMaterial { material_idx: params[0] });
            }
            0x05 => {
                // Draw a mesh
                ops.push(Op::Draw { mesh_idx: params[0] });
            }
            0x06 | 0x26 | 0x46 | 0x66 => {
                // Multiply the current matrix by an object matrix, possibly
                // loading a matrix from the stack beforehand, and possibly
                // storing the result to a stack location after.
                let object_idx = params[0];

                let _parent_id = params[1];
                let _unknown = params[2];

                let (store_pos, load_pos) = match opcode {
                    0x06 => (None, None),
                    0x26 => (Some(params[3]), None),
                    0x46 => (None, Some(params[3])),
                    0x66 => (Some(params[3]), Some(params[4])),
                    _ => unreachable!(),
                };

                if let Some(stack_pos) = load_pos {
                    ops.push(Op::LoadMatrix { stack_pos });
                }
                ops.push(Op::MulObject { object_idx });
                if let Some(stack_pos) = store_pos {
                    ops.push(Op::StoreMatrix { stack_pos });
                }
            }
            0x09 => {
                // Creates a matrix from the skinning equation and stores it to
                // a stack slot. The skinning equation is
                //
                //   ∑ weight * matrix_stack[stack_pos] * inv_binds[inv_bind_idx]
                //
                // Note that vertices to which a skinning matrix are applied are
                // given in model space -- the inverse bind matrices bring it
                // into the local space of each of its influencing objects.
                // Normal vertices are given directly in the local space of the
                // object that gets applied to them.
                let store_pos = params[0];

                let num_terms = params[1] as usize;
                let mut i = 2;
                let terms = (0..num_terms)
                    .map(|_| {
                        let stack_pos = params[i];
                        let inv_bind_idx = params[i + 1];
                        let weight = params[i + 2] as f32 / 256.0; // denormalize
                        i += 3;

                        SkinTerm { weight, stack_pos, inv_bind_idx }
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();

                ops.push(Op::Skin { terms });
                ops.push(Op::StoreMatrix { stack_pos: store_pos });
            }
            0x0b => {
                // Scale up by a per-model constant.
                ops.push(Op::ScaleUp);
            }
            0x2b => {
                // Scale down by a per-model constant.
                ops.push(Op::ScaleDown);
            }
            _ => {
                debug!("skipping unknown render command {:#x}", opcode);
            }
        }
    }
}

/// Fetch the next opcode and its parameters from the bytestream.
fn next_opcode_params<'a>(cur: &mut Cur<'a>) -> Result<(u8, &'a [u8])> {
    let opcode = cur.next::<u8>()?;

    // The only variable-length command
    if opcode == 0x09 {
        // 1 byte + 1 byte (count) + count u8[3]s
        let count = cur.nth::<u8>(1)?;
        let params_len = 1 + 1 + 3 * count;
        let params = cur.next_n_u8s(params_len as usize)?;
        return Ok((opcode, params));
    }

    let params_len = match opcode {
        0x00 => 0,
        0x01 => 0,
        0x02 => 2,
        0x03 => 1,
        0x04 => 1,
        0x05 => 1,
        0x06 => 3,
        0x07 => 1,
        0x08 => 1,
        0x0b => 0,
        0x0c => 2,
        0x0d => 2,
        0x24 => 1,
        0x26 => 4,
        0x2b => 0,
        0x40 => 0,
        0x44 => 1,
        0x46 => 4,
        0x47 => 2,
        0x66 => 5,
        0x80 => 0,
        _ => bail!("unknown render command opcode: {:#x}", opcode),
    };
    let params = cur.next_n_u8s(params_len)?;
    Ok((opcode, params))
}
