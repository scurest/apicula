use errors::Result;
use util::cur::Cur;

pub trait Sink {
    /// cur_matrix = matrix_stack[stack_pos]
    fn load_matrix(&mut self, stack_pos: u8) -> Result<()>;

    /// matrix_stack[stack_pos] = cur_matrix
    fn store_matrix(&mut self, stack_pos: u8) -> Result<()>;

    /// cur_matrix = cur_matrix * object_matrices[object_id]
    fn mul_by_object(&mut self, object_id: u8) -> Result<()>;

    /// cur_matrix = ∑_{t in terms} term.2 * matrix_stack[t.0] * inv_bind_matrices[t.1]
    fn blend(&mut self, terms: &[(u8, u8, f64)]) -> Result<()>;

    /// cur_matrix = cur_matrix * scale(model.up_scale)
    fn scale_up(&mut self) -> Result<()>;

    /// cur_matrix = cur_matrix * scale(model.down_scale)
    fn scale_down(&mut self) -> Result<()>;

    /// Draw meshes[mesh_id] using materials[material_id]
    fn draw(&mut self, mesh_id: u8, material_id: u8) -> Result<()>;
}

pub fn run_commands<S: Sink>(cur: Cur, sink: &mut S) -> Result<()> {
    let mut state = RenderInterpreterState::new();
    state.run_commands(sink, cur)
}

struct RenderInterpreterState {
    cur_material: u8,
}

impl RenderInterpreterState {
    fn new() -> RenderInterpreterState {
        RenderInterpreterState {
            cur_material: 0,
        }
    }

    fn run_commands<S: Sink>(&mut self, sink: &mut S, mut cur: Cur) -> Result<()> {
        loop {
            let opcode = cur.next::<u8>()?;
            let num_params = cmd_size(opcode, cur)?;
            let params = cur.next_n_u8s(num_params)?;
            trace!("{:#2x} {:?}", opcode, params);
            match opcode {
                0x00 => {
                    // NOP
                }
                0x01 => {
                    // End render commands
                    return Ok(())
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
                    //   other people called this command "visibility")
                    // * running it emits no GPU commands
                }
                0x03 => {
                    // Load a matrix from the stack
                    sink.load_matrix(params[0])?;
                }
                0x04 | 0x24 | 0x44 => {
                    // Set the current material
                    self.cur_material = params[0];
                }
                0x05 => {
                    // Draw a mesh
                    sink.draw(params[0], self.cur_material)?;
                }
                0x06 | 0x26 | 0x46 | 0x66 => {
                    // Multiply the current matrix by an object matrix, possibly
                    // loading a matrix from the stack beforehand, and possibly
                    // storing the result to a stack location after.
                    let object_id = params[0];
                    let _parent_id = params[1];
                    let _dummy = params[2];
                    let (stack_id, restore_id) = match opcode {
                        0x06 => (None,            None),
                        0x26 => (Some(params[3]), None),
                        0x46 => (None,            Some(params[3])),
                        0x66 => (Some(params[3]), Some(params[4])),
                        _ => unreachable!(),
                    };

                    if let Some(restore_id) = restore_id {
                        sink.load_matrix(restore_id)?;
                    }
                    sink.mul_by_object(object_id)?;
                    if let Some(stack_id) = stack_id {
                        sink.store_matrix(stack_id)?;
                    }
                }
                0x09 => {
                    // Blends multiple transforms and stores the result to a
                    // stack location.
                    //
                    // The formula is
                    //
                    //   ∑ weight * matrix_stack[stack_id] * inv_bind_matrix[inv_bind_id]
                    //
                    // The inverse bind matrix should be the inverse of the value of
                    // matrix_stack[stack_id] when the model is in the bind pose.
                    // Vertices to which these blended matrices are applied are specified
                    // in their bind pose world space position (unlike vertices which an
                    // unblended matrix is applied to, which are specified in the local
                    // space for that joint), and the inverse bind matrices are needed to
                    // to transform into the local space that each stack entry acts on.
                    let stack_pos = params[0];
                    let num_terms = params[1] as usize;

                    let mut param_idx = 2;
                    let terms = (0..num_terms)
                        .map(|_| {
                            let stack_id = params[param_idx];
                            let blend_id = params[param_idx+1];
                            let weight = params[param_idx+2] as f64 / 256.0;
                            param_idx += 3;

                            (stack_id, blend_id, weight)
                        })
                        .collect::<Vec<_>>();

                    sink.blend(&terms[..])?;
                    sink.store_matrix(stack_pos)?;
                }
                0x0b | 0x2b => {
                    // Scale by a constant in the model file
                    match opcode {
                        0x0b => sink.scale_up()?,
                        0x2b => sink.scale_down()?,
                        _ => unreachable!(),
                    }
                }
                0x0c => {} // TODO: ???
                0x0d => {} // TODO: ???
                _ => {
                    info!("unknown render command: {:#x} {:?}", opcode, params);
                }
            }
        }
    }
}

/// Returns a cursor to one-past-the-end of the last render opcode.
pub fn find_end(mut cur: Cur) -> Result<Cur> {
    loop {
        let opcode = cur.next::<u8>()?;
        if opcode == 0x01 { return Ok(cur); }
        let num_params = cmd_size(opcode, cur)?;
        let _params = cur.jump_forward(num_params);
    }
}

fn cmd_size(opcode: u8, cur: Cur) -> Result<usize> {
    let len = match opcode {
        0x00 => 0,
        0x01 => 0,
        0x02 => 2,
        0x03 => 1,
        0x04 => 1,
        0x05 => 1,
        0x06 => 3,
        0x07 => 1,
        0x08 => 1,
        0x09 => {
            // The only variable-length command.
            // 1 byte + 1 byte (count) + count u8[3]s
            2 + 3 * cur.clone().next_n_u8s(2)?[1] as usize
        }
        0x0b => 0,
        0x0c => 2,
        0x0d => 2,
        0x24 => 1,
        0x26 => 4,
        0x2b => 0,
        0x40 => 0,
        0x44 => 1,
        0x46 => 4,
        0x47 => 1,
        0x66 => 5,
        0x80 => 0,
        _ => bail!("unknown render command opcode: {:#x}", opcode),
    };
    Ok(len)
}
