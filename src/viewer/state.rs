use db::Database;
use connection::Connection;
use std::ops::Range;
use viewer::eye::Eye;

/// Which model/animation is being viewed and where it is being
/// viewed from.
///
/// This is all the info you'd need to be able to tell what should
/// be drawn to the screen.
#[derive(Clone)]
pub struct ViewState {
    pub model_id: usize,
    pub anim_state: Option<AnimState>,
    pub eye: Eye,
}

#[derive(Clone, PartialEq, Eq)]
pub struct AnimState {
    /// Index of the animation ID in the ModelConnection's animation list.
    pub anim_id_idx: usize,
    pub cur_frame: u16,
}

#[derive(Copy, Clone)]
pub enum Dir { Next, Prev }

/// Given `x` in the given range, regard the range as a circle (ie. `start`
/// comes after `end - 1`) and return the element that comes either before
/// or after `x`, depending on `dir`.
fn advance(x: usize, Range { start, end }: Range<usize>, dir: Dir) -> usize {
    assert!(start <= x && x < end);
    match dir {
        Dir::Next =>
            if x + 1 == end { start } else { x + 1 },
        Dir::Prev =>
            if x == start { end - 1 } else { x - 1 },
    }
}

impl ViewState {
    pub fn advance_model(&mut self, db: &Database, dir: Dir) {
        let num_models = db.models.len();

        self.model_id = advance(self.model_id, 0..num_models, dir);
        self.anim_state = None;
    }

    pub fn advance_anim(&mut self, conn: &Connection, dir: Dir) {
        let animations = &conn.models[self.model_id].animations;

        // Represent anim_id_idx as anim_id_idx+1, freeing up 0 to represent "no
        // animation".
        let idx = self.anim_state.as_ref()
            .map(|anim_state| anim_state.anim_id_idx + 1)
            .unwrap_or(0);
        let new_idx = advance(idx, 0..animations.len() + 1, dir);
        self.anim_state =
            if new_idx == 0 {
                None
            } else {
                Some(AnimState { anim_id_idx: new_idx - 1, cur_frame: 0 })
            };
    }

    pub fn next_frame(&mut self, db: &Database, conn: &Connection) {
        let model_id = self.model_id;
        self.anim_state.as_mut().map(|anim_state| {
            let anim_id = conn.models[model_id].animations[anim_state.anim_id_idx];
            let anim = &db.animations[anim_id];

            anim_state.cur_frame += 1;
            if anim_state.cur_frame >= anim.num_frames {
                anim_state.cur_frame = 0;
            }
        });
    }
}
