use errors::Result;
use gfx::GfxState;
use nitro::mdl::Object;
use util::cur::Cur;

pub trait Sink {
    fn draw(&mut self, gs: &mut GfxState, mesh_id: u8, material_id: u8) -> Result<()>;
}

pub struct Renderer {
    pub gfx_state: GfxState,
    pub cur_material: u8,
    pub cur_stack_ptr: u8,
}

impl Renderer {
    pub fn new() -> Renderer {
        Renderer {
            gfx_state: GfxState::new(),
            cur_material: 0,
            cur_stack_ptr: 0,
        }
    }

    pub fn run_render_cmds<S: Sink>(&mut self, sink: &mut S, objects: &[Object], mut cur: Cur) -> Result<()> {
        loop {
            let opcode = cur.next::<u8>()?;
            let num_params = cmd_size(opcode, cur)?;
            let params = cur.next_n_u8s(num_params)?;
            trace!("{:#2x} {:?}", opcode, params);
            match opcode {
                0x00 => { /* NOP */ }
                0x01 => { return Ok(()) }
                0x02 => {
                    // unknown
                }
                0x03 => {
                    self.gfx_state.restore(params[0] as u32);
                }
                0x04 | 0x24 | 0x44 => {
                    self.cur_material = params[0];
                }
                0x05 => {
                    sink.draw(&mut self.gfx_state, params[0], self.cur_material)?;
                }
                0x06 | 0x26 | 0x46 | 0x66 => {
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
                        self.gfx_state.restore(restore_id as u32);
                    }
                    self.gfx_state.cur_mat = self.gfx_state.cur_mat * objects[object_id as usize].xform;
                    if let Some(stack_id) = stack_id {
                        self.cur_stack_ptr = stack_id;
                    }
                    self.gfx_state.mat_stack[self.cur_stack_ptr as usize] = self.gfx_state.cur_mat;
                    self.cur_stack_ptr += 1;
                }
                0x09 => {
                    // unknown
                }
                _ => {
                    trace!("unknown render command: {:#x} {:?}", opcode, params);
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
