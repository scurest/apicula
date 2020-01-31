use std::ops::Range;
use super::model_viewer::{ModelViewer, MaterialTextureBinding};
use db::{Database, ModelId, AnimationId, PatternId, MatAnimId};
use connection::Connection;
use glium::Display;
use glium::glutin::{ElementState, ModifiersState};
use glium::{Frame, Surface};
use glium::glutin::VirtualKeyCode;
use nitro::{Model, Animation, Pattern, MaterialAnimation};
use primitives::{Primitives, PolyType, DynamicState};
use cgmath::{Matrix4, InnerSpace, Vector3, vec3, vec2};
use super::fps::FpsCounter;
use super::{FRAMERATE, BG_COLOR};

pub struct Viewer {
    db: Database,
    conn: Connection,
    model_viewer: ModelViewer,

    /// ID of the current model.
    model_id: ModelId,
    /// Current object matrices (changed as Animations play).
    object_mats: Vec<Matrix4<f64>>,
    /// Current texture binding for each material (changed as Pattern plays).
    material_map: Vec<MaterialTextureBinding>,
    /// Current UV transform matrices for each material (changed as
    /// MaterialAnimation plays).
    uv_mats: Vec<Matrix4<f64>>,

    // States for each different kind of animation.
    anim_state: AnimState,
    pat_state: AnimState,
    mat_anim_state: AnimState,

    /// Accumulator for time.
    time_acc: f64,
    /// Keeps track of the FPS.
    fps_counter: FpsCounter,
    /// Direction of motion (for the WASD controls).
    move_vector: Vector3<f32>,
    /// Movement speed (for WASD) as an index into the SPEEDS array.
    speed_idx: usize,
}

struct AnimState {
    /// Index into the current model's connection array.
    connection_idx: Option<usize>,
    /// Current frame
    frame: u16,
    /// When single-stepping is enabled, the user advances the animation
    /// manually instead of it advancing automatically with time.
    single_stepping: bool,
}

impl AnimState {
    fn none() -> AnimState {
        AnimState {
            connection_idx: None,
            frame: 0,
            single_stepping: false,
        }
    }

    fn next(&mut self, num: usize) {
        let new_idx = match self.connection_idx {
            None if num == 0 => None,
            None => Some(0),
            Some(x) if x == num - 1 => None,
            Some(x) => Some(x + 1),
        };
        self.connection_idx = new_idx;
        self.frame = 0;
        self.single_stepping = false;
    }

    fn prev(&mut self, num: usize) {
        let new_idx = match self.connection_idx {
            None if num == 0 => None,
            None => Some(num - 1),
            Some(0) => None,
            Some(x) => Some(x - 1),
        };
        self.connection_idx = new_idx;
        self.frame = 0;
        self.single_stepping = false;
    }
}

// Goes the next/prev element of a range, wrapping around.
fn next(x: usize, range: Range<usize>) -> usize {
    if x == range.end - 1 { range.start } else { x + 1 }
}
fn prev(x: usize, range: Range<usize>) -> usize {
    if x == range.start { range.end - 1 } else { x - 1 }
}
// grumble grumble
fn next_u16(x: u16, range: Range<u16>) -> u16 {
    if x == range.end - 1 { range.start } else { x + 1 }
}
fn prev_u16(x: u16, range: Range<u16>) -> u16 {
    if x == range.start { range.end - 1 } else { x - 1 }
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
        "  OP           Prev/Next Animation           (+Alt to single-step instead)\n",
        "  KL           Prev/Next Pattern Animation   (+Alt to single-step instead)\n",
        "  ;'           Prev/Next Material Animation  (+Alt to single-step instead)\n",
        "  Space        Print Info\n",
        "  T            Toggle Lights                 (Models with normals only)\n"
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
            model_id: 9999, // anything non-zero is okay, we're about to change it
            object_mats: vec![],
            material_map: vec![],
            uv_mats: vec![],
            anim_state: AnimState::none(),
            pat_state: AnimState::none(),
            mat_anim_state: AnimState::none(),
            time_acc: 0.0,
            fps_counter: FpsCounter::new(),
            move_vector: vec3(0.0, 0.0, 0.0),
            speed_idx: DEFAULT_SPEED_IDX,
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
            if !self.anim_state.single_stepping {
                self.next_anim_frame();
            }
            if !self.pat_state.single_stepping {
                self.next_pattern_frame(display);
            }
            if !self.mat_anim_state.single_stepping {
                self.next_mat_anim_frame();
            }
            self.time_acc -= FRAMERATE;
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        frame.clear_color_srgb_and_depth(BG_COLOR, 1.0);
        self.model_viewer.draw(frame, self.cur_model(&self.db))
    }

    /// Handle key press/release events.
    pub fn key(
        &mut self,
        display: &Display,
        (state, keycode, modifiers): (ElementState, VirtualKeyCode, ModifiersState),
    ) {
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

        let alt = modifiers.alt;

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
            Key::P if !alt => {
                let num_anims = self.conn.models[self.model_id].animations.len();
                self.anim_state.next(num_anims);
                self.update_object_mats();
            }
            Key::O if !alt => {
                let num_anims = self.conn.models[self.model_id].animations.len();
                self.anim_state.prev(num_anims);
                self.update_object_mats();
            }

            // Single-step forward/backward in animation
            Key::P if alt => {
                self.anim_state.single_stepping = true;
                self.next_anim_frame();
            }
            Key::O if alt => {
                self.anim_state.single_stepping = true;
                self.prev_anim_frame();
            }

            // Next/prev pattern animation
            Key::L if !alt => {
                let num_pats = self.conn.models[self.model_id].patterns.len();
                self.pat_state.next(num_pats);
                self.update_material_map(display);
            }
            Key::K if !alt => {
                let num_pats = self.conn.models[self.model_id].patterns.len();
                self.pat_state.prev(num_pats);
                self.update_material_map(display);
            }

            // Single-step pattern animation
            Key::L if alt => {
                self.pat_state.single_stepping = true;
                self.next_pattern_frame(display);
            }
            Key::K if alt => {
                self.pat_state.single_stepping = true;
                self.prev_pattern_frame(display);
            }

            // Next/prev material animation
            Key::Apostrophe if !alt => {
                let num_mat_anims = self.conn.models[self.model_id].mat_anims.len();
                self.mat_anim_state.next(num_mat_anims);
                self.update_uv_mats();
            }
            Key::Semicolon if !alt => {
                let num_mat_anims = self.conn.models[self.model_id].mat_anims.len();
                self.mat_anim_state.prev(num_mat_anims);
                self.update_uv_mats();
            }

            // Single-step material animation
            Key::Apostrophe if alt => {
                self.mat_anim_state.single_stepping = true;
                self.next_mat_anim_frame();
            }
            Key::Semicolon if alt => {
                self.mat_anim_state.single_stepping = true;
                self.prev_mat_anim_frame();
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

            Key::Space => {
                self.print_info();
            }

            Key::T => {
                self.model_viewer.light_on = !self.model_viewer.light_on;
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

        let model = self.cur_model(&self.db);
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
                cur_frame = self.anim_state.frame,
                num_frames = anim.num_frames,
            ).unwrap()
        } else {
            write!(s, "Rest Pose === ").unwrap()
        }
        if let Some(pat_id) = self.pattern_id() {
            let pat = self.cur_pattern(&self.db).unwrap();
            write!(s, "{pat_name}[{pat_id}/{num_pats}] ({cur_frame}/{num_frames}) === ",
                pat_name = pat.name,
                pat_id = pat_id,
                num_pats = self.db.patterns.len(),
                cur_frame = self.pat_state.frame,
                num_frames = pat.num_frames,
            ).unwrap()
        } else {
            write!(s, "No Pattern === ").unwrap()
        }
        if let Some(mat_anim_id) = self.mat_anim_id() {
            let mat_anim = self.cur_mat_anim(&self.db).unwrap();
            write!(s, "{anim_name}[{anim_id}/{num_anims}] ({cur_frame}/{num_frames}) === ",
                anim_name = mat_anim.name,
                anim_id = mat_anim_id,
                num_anims = self.db.mat_anims.len(),
                cur_frame = self.mat_anim_state.frame,
                num_frames = mat_anim.num_frames,
            ).unwrap()
        } else {
            write!(s, "No Material Animation === ").unwrap()
        }
        write!(s, "{:5.2}fps", self.fps_counter.fps()).unwrap();
    }

    pub fn print_info(&self) {
        println!("=============");
        println!("Model: {:?} [{}/{}]",
            self.cur_model(&self.db).name,
            self.model_id,
            self.db.models.len());
        println!("Found in file: {}", self.db.file_paths[self.db.models_found_in[self.model_id]].display());

        if let Some(anim_id) = self.animation_id() {
            let anim = self.cur_animation(&self.db).unwrap();
            println!("Animation: {:?} [{}/{}]",
                anim.name,
                anim_id,
                self.db.animations.len(),
            );
            println!("Found in file: {}", self.db.file_paths[self.db.animations_found_in[anim_id]].display());
        } else {
            println!("No Animation Playing");
        }

        if let Some(pat_id) = self.pattern_id() {
            let pat = self.cur_pattern(&self.db).unwrap();
            println!("Pattern Animation: {:?} [{}/{}]",
                pat.name,
                pat_id,
                self.db.patterns.len(),
            );
            println!("Found in file: {}", self.db.file_paths[self.db.patterns_found_in[pat_id]].display());
        } else {
            println!("No Pattern Animation Playing")
        }

        if let Some(mat_anim_id) = self.mat_anim_id() {
            let mat_anim = self.cur_mat_anim(&self.db).unwrap();
            println!("Material Animation: {:?} [{}/{}]",
                mat_anim.name,
                mat_anim_id,
                self.db.mat_anims.len(),
            );
            println!("Found in file: {}", self.db.file_paths[self.db.mat_anims_found_in[mat_anim_id]].display());
        } else {
            println!("No Material Animation Playing")
        }

        println!();
    }

    /// Changes to a new model.
    pub fn change_model(&mut self, display: &Display, model_id: ModelId) {
        if self.model_id == model_id {
            return;
        }

        // Reset animations
        self.anim_state = AnimState::none();
        self.pat_state = AnimState::none();
        self.mat_anim_state = AnimState::none();

        self.model_id = model_id;
        self.reset_state_from_model();
        self.update_material_map(display);

        let state = DynamicState { objects: &self.object_mats, uv_mats: &self.uv_mats };
        let prims = Primitives::build(self.cur_model(&self.db), PolyType::Tris, state);
        self.model_viewer.change_model(display, &self.db, prims, self.material_map.clone());
    }

    // Reset dynamic state to the static value in the model

    fn reset_object_mats_from_model(&mut self) {
        // Set from model values
        self.object_mats.clear();
        for obj in &self.cur_model(&self.db).objects {
            self.object_mats.push(obj.matrix);
        }
    }

    fn reset_material_map_from_model(&mut self) {
        self.material_map.clear();
        for mat_conn in &self.conn.models[self.model_id].materials {
            self.material_map.push(
                match mat_conn.image_id() {
                    Ok(Some(image_id)) => MaterialTextureBinding::ImageId(image_id),
                    Ok(None) => MaterialTextureBinding::None,
                    Err(_) => MaterialTextureBinding::Missing,
                }
            );
        }
    }

    fn reset_uv_mats_from_model(&mut self) {
        self.uv_mats.clear();
        for material in &self.cur_model(&self.db).materials {
            self.uv_mats.push(material.texture_mat)
        }
    }

    fn reset_state_from_model(&mut self) {
        self.reset_object_mats_from_model();
        self.reset_material_map_from_model();
        self.reset_uv_mats_from_model();
    }

    // Update dynamic state to match the current animation state.

    fn update_object_mats(&mut self) {
        if let Some(anim) = self.cur_animation(&self.db) {
            for i in 0..self.object_mats.len() {
                if i >= anim.objects_curves.len() { break }
                self.object_mats[i] = anim.objects_curves[i].sample_at(self.anim_state.frame);
            }
        } else {
            self.reset_object_mats_from_model();
        }
        self.update_vertices();
    }

    fn update_material_map(&mut self, display: &Display) {
        if let Some(pat) = self.cur_pattern(&self.db) {
            let mats = &self.cur_model(&self.db).materials;
            for (mat_id, mat) in mats.iter().enumerate() {
                // Find a track for this material (with the same name)
                let track =
                    pat.material_tracks.iter()
                    .find(|track| track.name == mat.name);
                let track = match track {
                    Some(x) => x,
                    None => continue,
                };

                let (texture_idx, palette_idx) = track.sample(self.pat_state.frame);

                let pat_conn_idx = self.pat_state.connection_idx.unwrap();
                let pat_conn = &self.conn.models[self.model_id].patterns[pat_conn_idx];
                let (texture_id, palette_id) = (
                    pat_conn.texture_ids[texture_idx as usize],
                    pat_conn.palette_ids[palette_idx as usize],
                );

                self.material_map[mat_id] = match (texture_id, palette_id) {
                    (Some(t), Some(p)) => MaterialTextureBinding::ImageId((t, Some(p))),
                    _ => MaterialTextureBinding::Missing,
                };
            }
        } else {
            self.reset_material_map_from_model()
        }
        self.update_materials(display);
    }

    fn update_uv_mats(&mut self) {
        if let Some(mat_anim) = self.cur_mat_anim(&self.db) {
            for track in &mat_anim.tracks {
                for (i, mat) in self.cur_model(&self.db).materials.iter().enumerate() {
                    if mat.name == track.name {
                        self.uv_mats[i] = track.eval_uv_mat(self.mat_anim_state.frame);
                        break;
                    }
                }
            }
        } else {
            self.reset_uv_mats_from_model();
        }
        self.update_vertices();
    }

    /// Updates the vertices of the current model (eg. because an animation or
    /// material animation has advanced).
    fn update_vertices(&mut self) {
        let state = DynamicState { objects: &self.object_mats, uv_mats: &self.uv_mats };
        let prims = Primitives::build(self.cur_model(&self.db), PolyType::Tris, state);
        self.model_viewer.update_vertices(&prims.vertices);
    }

    /// Updates the materials after the material map has changed (eg. because a
    /// pattern animation has advanced).
    fn update_materials(&mut self, display: &Display) {
        self.model_viewer.update_materials(display, &self.db, self.material_map.clone());
    }

    pub fn next_anim_frame(&mut self) {
        if let Some(anim) = self.cur_animation(&self.db) {
            self.anim_state.frame = next_u16(self.anim_state.frame, 0..anim.num_frames);
            self.update_object_mats();
        }
    }

    pub fn prev_anim_frame(&mut self) {
        if let Some(anim) = self.cur_animation(&self.db) {
            self.anim_state.frame = prev_u16(self.anim_state.frame, 0..anim.num_frames);
            self.update_object_mats();
        }
    }

    pub fn next_pattern_frame(&mut self, display: &Display) {
        if let Some(pat) = self.cur_pattern(&self.db) {
            self.pat_state.frame = next_u16(self.pat_state.frame, 0..pat.num_frames);
            self.update_material_map(display);
        }
    }

    pub fn prev_pattern_frame(&mut self, display: &Display) {
        if let Some(pat) = self.cur_pattern(&self.db) {
            self.pat_state.frame = prev_u16(self.pat_state.frame, 0..pat.num_frames);
            self.update_material_map(display);
        }
    }

    pub fn next_mat_anim_frame(&mut self) {
        if let Some(mat_anim) = self.cur_mat_anim(&self.db) {
            self.mat_anim_state.frame = next_u16(self.mat_anim_state.frame, 0..mat_anim.num_frames);
            self.update_uv_mats();
        }
    }

    pub fn prev_mat_anim_frame(&mut self) {
        if let Some(mat_anim) = self.cur_mat_anim(&self.db) {
            self.mat_anim_state.frame = prev_u16(self.mat_anim_state.frame, 0..mat_anim.num_frames);
            self.update_uv_mats();
        }
    }

    pub fn set_aspect_ratio(&mut self, aspect_ratio: f64) {
        self.model_viewer.aspect_ratio = aspect_ratio as f32;
    }

    // Getters

    // These take the DB to make borrowck happy.
    fn cur_model<'a>(&self, db: &'a Database) -> &'a Model {
        &db.models[self.model_id]
    }
    fn cur_animation<'a>(&self, db: &'a Database) -> Option<&'a Animation> {
        Some(&db.animations[self.animation_id()?])
    }
    fn cur_pattern<'a>(&self, db: &'a Database) -> Option<&'a Pattern> {
        Some(&db.patterns[self.pattern_id()?])
    }

    fn cur_mat_anim<'a>(&self, db: &'a Database) -> Option<&'a MaterialAnimation> {
        Some(&db.mat_anims[self.mat_anim_id()?])
    }

    fn animation_id(&self) -> Option<AnimationId> {
        let idx = self.anim_state.connection_idx?;
        Some(self.conn.models[self.model_id].animations[idx])
    }

    fn pattern_id(&self) -> Option<PatternId> {
        let idx = self.pat_state.connection_idx?;
        Some(self.conn.models[self.model_id].patterns[idx].pattern_id)
    }

    fn mat_anim_id(&self) -> Option<MatAnimId> {
        let idx = self.mat_anim_state.connection_idx?;
        Some(self.conn.models[self.model_id].mat_anims[idx].mat_anim_id)
    }

    pub fn speed(&self) -> f32 {
        SPEEDS[self.speed_idx]
    }
}
