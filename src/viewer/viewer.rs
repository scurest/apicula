use super::model_viewer::{ModelViewer, MaterialTextureBinding};
use db::{Database, ModelId, AnimationId, PatternId};
use connection::Connection;
use glium::Display;
use glium::glutin::ElementState;
use glium::{Frame, Surface};
use glium::glutin::VirtualKeyCode;
use nitro::{Model, Animation, Pattern};
use primitives::{Primitives, PolyType};
use cgmath::{Matrix4, InnerSpace, One, Vector3, vec3, vec2};
use super::fps::FpsCounter;
use super::{FRAMERATE, BG_COLOR};

pub struct Viewer {
    db: Database,
    conn: Connection,
    model_viewer: ModelViewer,

    /// ID of the viewed model.
    model_id: ModelId,
    /// The connection has a list of all the animations that can be applied to
    /// this model. This is the index of the current animation in that list (or
    /// None if no animation is selected).
    anim_idx: Option<usize>,
    /// Current animation frame.
    anim_frame: u16,
    pat_idx: Option<usize>,
    pat_anim_frame: u16,
    /// When single-stepping is enabled, the user advanced the animation
    /// manually. When disabled, the animation advanced with time (ie. plays
    /// normally).
    single_stepping: bool,
    /// Accumulator for time.
    time_acc: f64,
    fps_counter: FpsCounter,
    /// Direction of motion.
    move_vector: Vector3<f32>,
    /// Index of the current speed in SPEEDS.
    speed_idx: usize,
}

static SPEEDS: [f32; 9] = [0.5, 1.0, 3.0, 15.0, 32.0, 64.0, 96.0, 200.0, 550.0];
static DEFAULT_SPEED_IDX: usize = 1;

pub static CONTROL_HELP: &'static str =
    concat!(
        "--------\n",
        "Controls\n",
        "--------\n",
        "  WASD         Forward/Left/Back/Right\n",
        "  EQ           Up/Down\n",
        "  L.Shift      Increase Speed\n",
        "  L.Ctrl       Decrease Speed\n",
        "  L.Mouse      Free Look\n",
        "  ,.           Prev/Next Model\n",
        "  OP           Prev/Next Animation\n",
        "  []           Single-step Animation\n",
        "  KL           Prev/Next Pattern Animation\n",
    );


impl Viewer {
    pub fn new(display: &Display, db: Database, conn: Connection) -> Viewer {
        let model_viewer = ModelViewer::new(&display);

        // Create a viewer for model 0
        assert!(db.models.len() > 0);
        let mut viewer = Viewer {
            db,
            conn,
            model_viewer,
            model_id: 9999, // arbitrary; we're about to set it to 0
            anim_idx: None,
            anim_frame: 0,
            pat_idx: None,
            pat_anim_frame: 0,
            single_stepping: false,
            move_vector: vec3(0.0, 0.0, 0.0),
            speed_idx: DEFAULT_SPEED_IDX,
            time_acc: 0.0,
            fps_counter: FpsCounter::new(),
        };
        viewer.change_model(display, 0);
        viewer
    }

    /// Update state with the delta-time since last update. Called once before
    /// each frame.
    pub fn update(&mut self, display: &Display, dt: f64) {
        self.time_acc += dt;
        self.fps_counter.update(dt);

        // Update position
        let mag = self.move_vector.magnitude();
        if mag != 0.0 {
            let dv = self.speed() * (self.move_vector / mag);
            self.model_viewer.eye.move_by((dt as f32) * dv)
        }

        // Don't let the accumulator get too full if we lag or something.
        if self.time_acc > 1.0 {
            self.time_acc = 1.0;
        }

        while self.time_acc > FRAMERATE {
            if !self.single_stepping {
                self.next_frame();
            }
            self.next_pattern_frame(display);
            self.time_acc -= FRAMERATE;
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        frame.clear_color_srgb_and_depth(BG_COLOR, 1.0);
        self.model_viewer.draw(frame, self.cur_model())
    }

    /// Handle key press/release events.
    pub fn key(&mut self, display: &Display, (state, keycode): (ElementState, VirtualKeyCode)) {
        use self::VirtualKeyCode as Key;

        // Use WASD controls to update the move_vector.
        static MOVE_KEYS: [(Key, Key, usize); 3] = [
            // Key to move forward, key to move backward, affected XYZ component
            (Key::W, Key::S, 0),
            (Key::D, Key::A, 1),
            (Key::E, Key::Q, 2),
        ];
        for &(forward_key, backward_key, component) in &MOVE_KEYS {
            let x =
                if keycode == forward_key { 1.0 }
                else if keycode == backward_key { -1.0 }
                else { continue; };

            match state {
                ElementState::Pressed => self.move_vector[component] = x,
                ElementState::Released => self.move_vector[component] = 0.0,
            }
        }

        if state != ElementState::Pressed { return; }

        match keycode {
            // Next/prev model
            Key::Period => {
                let new_model_id = next(self.model_id, 0..self.db.models.len());
                self.change_model(display, new_model_id);
            }
            Key::Comma => {
                let new_model_id = prev(self.model_id, 0..self.db.models.len());
                self.change_model(display, new_model_id);
            }

            // Next/prev animation
            Key::P => {
                let num_anims = self.conn.models[self.model_id].animations.len();
                let new_anim_idx = maybe_next(self.anim_idx, 0..num_anims);
                self.change_anim_idx(new_anim_idx);
            }
            Key::O => {
                let num_anims = self.conn.models[self.model_id].animations.len();
                let new_anim_idx = maybe_prev(self.anim_idx, 0..num_anims);
                self.change_anim_idx(new_anim_idx);
            }

            // Single-step forward/backward in animation
            Key::RBracket => {
                self.single_stepping = true;
                self.next_frame();
            }
            Key::LBracket => {
                self.single_stepping = true;
                self.prev_frame();
            }

            // Next/prev pattern animation
            Key::L => {
                let num_pats = self.conn.models[self.model_id].patterns.len();
                let new_pat_idx = maybe_next(self.pat_idx, 0..num_pats);
                self.change_pat_idx(display, new_pat_idx);
            }
            Key::K => {
                let num_pats = self.conn.models[self.model_id].patterns.len();
                let new_pat_idx = maybe_prev(self.pat_idx, 0..num_pats);
                self.change_pat_idx(display, new_pat_idx);
            }

            // Speed up/down
            Key::LShift => {
                if self.speed_idx != SPEEDS.len() - 1 {
                    self.speed_idx += 1;
                }
            }
            Key::LControl => {
                if self.speed_idx != 0 {
                    self.speed_idx -= 1;
                }
            }

            _ => (),
        }
    }

    /// Handle mouse drag while the LMB is clicked.
    pub fn mouse_drag(&mut self, (dx, dy): (f64, f64)) {
        self.model_viewer.eye.free_look(0.01 * vec2(dx as f32, dy as f32));
    }

    /// Handler for window blur (loss of focus).
    pub fn blur(&mut self) {
        // Stop moving.
        self.move_vector = vec3(0.0, 0.0, 0.0);
    }

    /// Write the window title.
    pub fn title(&self, s: &mut String) {
        use std::fmt::Write;

        let model = self.cur_model();
        write!(s, "{model_name}[{model_num}/{num_models}] === ",
            model_name = model.name,
            model_num = self.model_id,
            num_models = self.db.models.len(),
        ).unwrap();
        if let Some(anim_id) = self.animation_id() {
            let anim = self.cur_animation(&self.db).unwrap();
            write!(s, "{anim_name}[{anim_id}/{num_anims}] ({cur_frame}/{num_frames}) === ",
                anim_name = anim.name,
                anim_id = anim_id,
                num_anims = self.db.animations.len(),
                cur_frame = self.anim_frame,
                num_frames = anim.num_frames,
            ).unwrap()
        } else {
            write!(s, "Rest Pose === ").unwrap()
        }
        if let Some(pat_id) = self.pattern_id() {
            let pat = self.cur_pattern().unwrap();
            write!(s, "{pat_name}[{pat_id}/{num_pats}] ({cur_frame}/{num_frames}) === ",
                pat_name = pat.name,
                pat_id = pat_id,
                num_pats = self.db.patterns.len(),
                cur_frame = self.pat_anim_frame,
                num_frames = pat.num_frames,
            ).unwrap()
        } else {
            write!(s, "No Pattern === ").unwrap()
        }
        write!(s, "{:5.2}fps", self.fps_counter.fps()).unwrap();
    }

    /// Changes to a new model.
    pub fn change_model(&mut self, display: &Display, model_id: ModelId) {
        if self.model_id == model_id {
            return;
        }

        self.stop_animations();

        self.model_id = model_id;
        let objects = self.cur_model().objects.iter()
            .map(|ob| ob.matrix)
            .collect::<Vec<Matrix4<f64>>>();
        let material_map = self.conn.models[self.model_id].materials.iter().map(|mat_conn| {
            match mat_conn.image_id() {
                Ok(Some(image_id)) => MaterialTextureBinding::ImageId(image_id),
                Ok(None) => MaterialTextureBinding::None,
                Err(_) => MaterialTextureBinding::Missing,
            }
        }).collect();
        let prims = Primitives::build(self.cur_model(), PolyType::Tris, &objects);
        self.model_viewer.change_model(display, &self.db, prims, material_map);
    }

    /// Updates the vertices of the current model (to their position in the
    /// current animation frame).
    fn update_vertices(&mut self) {
        let objects = match self.cur_animation(&self.db) {
            None => {
                // Rest pose, use object values in model file
                self.cur_model().objects.iter()
                    .map(|ob| ob.matrix)
                    .collect::<Vec<Matrix4<f64>>>()
            }
            Some(anim) => {
                // Animated, sample animation curves
                let num_objects = self.cur_model().objects.len();
                (0..num_objects)
                    .map(|i| {
                        anim.objects_curves.get(i)
                            .map(|curve| curve.sample_at(self.anim_frame))
                            .unwrap_or(Matrix4::one())
                    })
                    .collect::<Vec<Matrix4<f64>>>()
            }
        };
        let prims = Primitives::build(self.cur_model(), PolyType::Tris, &objects);
        self.model_viewer.update_vertices(&prims.vertices);
    }

    /// Update what texture to use for each material (to the values in the
    /// current pattern animation frame).
    fn update_materials(&mut self, display: &Display) {
        let material_map = match self.cur_pattern() {
            None => {
                self.conn.models[self.model_id].materials.iter().map(|mat_conn| {
                    match mat_conn.image_id() {
                        Ok(Some(image_id)) => MaterialTextureBinding::ImageId(image_id),
                        Ok(None) => MaterialTextureBinding::None,
                        Err(_) => MaterialTextureBinding::Missing,
                    }
                }).collect()
            }
            Some(pat) => {
                let pat_conn = &self.conn.models[self.model_id].patterns[self.pat_idx.unwrap()];
                pat.material_tracks.iter().map(|track| {
                    let (texture_idx, palette_idx) = track.sample(self.pat_anim_frame);
                    let texture_id = pat_conn.texture_ids[texture_idx as usize]?;
                    let palette_id = pat_conn.palette_ids[palette_idx as usize]?;
                    Some((texture_id, palette_id))
                }).map(|result| {
                    match result {
                        Some((texture_id, palette_id)) =>
                            MaterialTextureBinding::ImageId((texture_id, Some(palette_id))),
                        None =>
                            MaterialTextureBinding::Missing,
                    }
                }).collect()
            }
        };
        self.model_viewer.update_materials(display, &self.db, material_map);
    }

    fn stop_animations(&mut self) {
        self.anim_idx = None;
        self.anim_frame = 0;
        self.single_stepping = false;
        self.pat_idx = None;
        self.pat_anim_frame = 0;
    }

    fn change_anim_idx(&mut self, anim_idx: Option<usize>) {
        if self.anim_idx == anim_idx {
            return;
        }

        self.single_stepping = false;
        self.anim_idx = anim_idx;
        self.anim_frame = 0;
        self.update_vertices();
    }

    pub fn next_frame(&mut self) {
        let num_anim_frames = self.cur_animation(&self.db).map(|anim| anim.num_frames);
        if let Some(num_anim_frames) = num_anim_frames {
            self.anim_frame += 1;
            self.anim_frame %= num_anim_frames;
            self.update_vertices();
        }
    }

    pub fn prev_frame(&mut self) {
        let num_anim_frames = self.cur_animation(&self.db).map(|anim| anim.num_frames);
        if let Some(num_anim_frames) = num_anim_frames {
            if self.anim_frame == 0 {
                self.anim_frame = num_anim_frames - 1;
            } else {
                self.anim_frame -= 1;
            }
            self.update_vertices();
        }
    }

    fn change_pat_idx(&mut self, display: &Display, pat_idx: Option<usize>) {
        if self.pat_idx == pat_idx {
            return;
        }

        self.pat_idx = pat_idx;
        self.pat_anim_frame = 0;
        self.update_materials(display);
    }

    pub fn next_pattern_frame(&mut self, display: &Display) {
        let num_frames = self.cur_pattern().map(|pat| pat.num_frames);
        if let Some(num_frames) = num_frames {
            self.pat_anim_frame += 1;
            self.pat_anim_frame %= num_frames;
            self.update_materials(display);
        }
    }

    pub fn set_aspect_ratio(&mut self, aspect_ratio: f64) {
        self.model_viewer.aspect_ratio = aspect_ratio as f32;
    }

    fn cur_model(&self) -> &Model {
        &self.db.models[self.model_id]
    }

    fn cur_animation<'a>(&self, db: &'a Database) -> Option<&'a Animation> {
        Some(&db.animations[self.animation_id()?])
    }

    fn cur_pattern(&self) -> Option<&Pattern> {
        Some(&self.db.patterns[self.pattern_id()?])
    }

    fn animation_id(&self) -> Option<AnimationId> {
        let idx = self.anim_idx?;
        Some(self.conn.models[self.model_id].animations[idx])
    }

    fn pattern_id(&self) -> Option<PatternId> {
        let idx = self.pat_idx?;
        Some(self.conn.models[self.model_id].patterns[idx].pattern_id)
    }

    pub fn speed(&self) -> f32 {
        SPEEDS[self.speed_idx]
    }
}

use std::ops::Range;

// Goes the next/prev element of a range, wrapping around.

fn next(x: usize, range: Range<usize>) -> usize {
    if x == range.end - 1 { range.start } else { x + 1 }
}

fn prev(x: usize, range: Range<usize>) -> usize {
    if x == range.start { range.end - 1 } else { x - 1 }
}

// Goes to the next/prev element of a range. None is before the first element of
// the range and after the last.

fn maybe_next(x: Option<usize>, range: Range<usize>) -> Option<usize> {
    match x {
        None if range.start == range.end => None,
        None => Some(range.start),
        Some(y) if y == range.end - 1 => None,
        Some(y) => Some(y + 1),
    }
}

fn maybe_prev(x: Option<usize>, range: Range<usize>) -> Option<usize> {
    match x {
        None if range.start == range.end => None,
        None => Some(range.end - 1),
        Some(y) if y == range.start => None,
        Some(y) => Some(y - 1),
    }
}
