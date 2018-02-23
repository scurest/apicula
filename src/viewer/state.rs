use db::Database;
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
    pub anim_id: usize,
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

    pub fn advance_anim(&mut self, db: &Database, dir: Dir) {
        let num_animations = db.animations.len();
        let num_objects = db.models[self.model_id].objects.len();

        // Represent anim_id as anim_id+1, freeing up 0 to represent "no animation".
        let mut id = self.anim_state.as_ref()
            .map(|anim_state| anim_state.anim_id + 1)
            .unwrap_or(0);

        // An animations can be applied to a model only if they have the same
        // number of objects.
        let is_good = |id: usize| {
            id == 0 || db.animations[id - 1].objects_curves.len() == num_objects
        };

        loop {
            id = advance(id, 0..num_animations + 1, dir);
            if is_good(id) { break; }
        }

        self.anim_state =
            if id == 0 {
                None
            } else {
                Some(AnimState { anim_id: id - 1, cur_frame: 0 })
            };
    }

    pub fn next_frame(&mut self, db: &Database) {
        self.anim_state.as_mut().map(|anim_state| {
            let anim = &db.animations[anim_state.anim_id];

            anim_state.cur_frame += 1;
            if anim_state.cur_frame >= anim.num_frames {
                anim_state.cur_frame = 0;
            }
        });
    }
}
