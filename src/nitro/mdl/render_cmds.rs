use errors::Result;
use util::bits::BitField;
use util::cur::Cur;

pub trait Sink {
    /// cur_matrix = matrix_stack[stack_pos]
    fn load_matrix(&mut self, stack_pos: u8) -> Result<()>;
    /// matrix_stack[stack_pos] = cur_matrix
    fn store_matrix(&mut self, stack_pos: u8) -> Result<()>;
    /// cur_matrix = cur_matrix * object_matrices[object_id]
    fn mul_by_object(&mut self, object_id: u8) -> Result<()>;
    /// cur_matrix = ∑_{t in terms} term.2 * matrix_stack[t.0] * blend_matrix[t.1]
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
    cur_stack_pos: u8,
}

impl RenderInterpreterState {
    fn new() -> RenderInterpreterState {
        RenderInterpreterState {
            cur_material: 0,
            cur_stack_pos: 0,
        }
    }

    /// Set the stack position. The DS only reads the low 5-bits
    /// (the stack is 32 elements) so we mask down here.
    fn set_stack_pos(&mut self, new_pos: u8) {
        self.cur_stack_pos = new_pos.bits(0,5);
    }

    fn inc_stack_pos(&mut self) {
        let new_pos = self.cur_stack_pos + 1;
        self.set_stack_pos(new_pos);
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
                    // loading a matrix from the stack beforehand, and store the
                    // result to a stack location.
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
                        self.set_stack_pos(stack_id);
                    }
                    sink.store_matrix(self.cur_stack_pos)?;
                    self.inc_stack_pos();
                }
                0x09 => {
                    // The current matrix is set to the sum of
                    //    weight * matrix_stack[stack_id] * blend_matrix[blend_id]
                    // and stored to the given stack slot.
                    let stack_pos = params[0];
                    let num_terms = params[1] as usize;

                    check!(num_terms <= 4)?;
                    let mut terms = [(0, 0, 0.0); 4];

                    let mut param_idx = 2;
                    for i in 0..num_terms {
                        let stack_id = params[param_idx];
                        let blend_id = params[param_idx+1];
                        let weight = params[param_idx+2] as f64 / 256.0;

                        terms[i] = (stack_id, blend_id, weight);

                        param_idx += 3;
                    }

                    sink.blend(&terms[0..num_terms])?;
                    self.set_stack_pos(stack_pos);
                    sink.store_matrix(self.cur_stack_pos)?;
                    self.inc_stack_pos();
                }
                0x0b | 0x2b => {
                    // Scale by a constant in the model file
                    match opcode {
                        0x0b => sink.scale_up()?,
                        0x2b => sink.scale_down()?,
                        _ => unreachable!(),
                    }
                }
                _ => {
                    info!("unknown render command: {:#x} {:?}", opcode, params);
                }
            }
        }
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
        0x24 => 1,
        0x26 => 4,
        0x2b => 0,
        0x40 => 0,
        0x44 => 1,
        0x46 => 4,
        0x66 => 5,
        0x80 => 0,
        _ => return Err(format!("unknown render command opcode: {:#x}", opcode).into()),
    };
    Ok(len)
}
