use super::info_block;
use errors::Result;
use nitro::Name;
use util::cur::Cur;

/// A pattern animation changes the images that the materials of a model use as
/// it plays.
pub struct Pattern {
    pub name: Name,
    pub num_frames: u16,
    pub texture_names: Vec<Name>,
    pub palette_names: Vec<Name>,
    pub material_tracks: Vec<PatternTrack>,
}

/// Gives the keyframes at which an image should change.
pub struct PatternTrack {
    pub name: Name,
    pub keyframes: Vec<PatternKeyframe>,
}

pub struct PatternKeyframe {
    pub frame: u16,
    /// Index into the Pattern.texture_names array.
    pub texture_idx: u8,
    /// Index into the Pattern.palette_names array.
    pub palette_idx: u8,
}

pub fn read_pattern(cur: Cur, name: Name) -> Result<Pattern> {
    debug!("pattern: {:?}", name);

    fields!(
        cur,
        pattern {
            _unknown: [u8; 4],
            num_frames: u16,
            num_texture_names: u8,
            num_palette_names: u8,
            texture_names_off: u16,
            palette_names_off: u16,
            end: Cur,
        }
    );

    let texture_names = (cur + texture_names_off)
        .next_n::<Name>(num_texture_names as usize)?
        .collect::<Vec<Name>>();

    let palette_names = (cur + palette_names_off)
        .next_n::<Name>(num_palette_names as usize)?
        .collect::<Vec<Name>>();

    let material_tracks = info_block::read::<(u32, u16, u16)>(end)?
        .map(|((num_keyframes, _, off), name)| {
            let keyframes = (cur + off)
                .next_n::<(u16, u8, u8)>(num_keyframes as usize)?
                .map(|(frame, texture_idx, palette_idx)| PatternKeyframe {
                    frame,
                    texture_idx,
                    palette_idx,
                })
                .collect();

            Ok(PatternTrack { name, keyframes })
        })
        .collect::<Result<Vec<PatternTrack>>>()?;

    Ok(Pattern {
        name,
        num_frames,
        texture_names,
        palette_names,
        material_tracks,
    })
}

impl PatternTrack {
    pub fn sample(&self, frame: u16) -> (u8, u8) {
        // Linear search for the first keyframe after the given frame. The
        // desired keyframe is the one before that.
        let next_pos = self.keyframes.iter().position(|key| key.frame > frame);
        let keyframe = match next_pos {
            Some(0) => &self.keyframes[0],
            Some(n) => &self.keyframes[n - 1],
            None => &self.keyframes[self.keyframes.len() - 1],
        };
        (keyframe.texture_idx, keyframe.palette_idx)
    }
}
