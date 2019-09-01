// HIGHLY INCOMPLETE!!!

use util::cur::Cur;
use util::view::Viewable;
use nitro::Name;
use errors::Result;
use super::info_block;

/// Material animation. Does things like UV matrix animation.
pub struct MaterialAnimation {
    pub name: Name,
    pub num_frames: u16,
    pub tracks: Vec<MaterialTrack>,
}

/// Targets one material in a model and animates it.
pub struct MaterialTrack {
    pub name: Name,
    pub num_frames: u16,
}

pub fn read_mat_anim(cur: Cur, name: Name) -> Result<MaterialAnimation> {
    debug!("material animation: {:?}", name);

    fields!(cur, mat_anim {
        _unknown: [u8; 4],  // b'M\0AT' ??
        num_frames: u16,
        unknown: u16,
        end: Cur,
    });

    // The info block contains a whopping 40 bytes of data per track! We're
    // gonna need a special struct for that.
    #[derive(Debug)]
    struct TrackData {
        num_frames: u16,
        other: [u8; 32], // split into two because Rust is dumb
        other2: [u8; 6],
    }
    impl Viewable for TrackData {
        fn size() -> usize { 40 }
        fn view(buf: &[u8]) -> TrackData {
            let mut cur = Cur::new(buf);
            let num_frames = cur.next::<u16>().unwrap();
            let mut other = [0; 32];
            other.copy_from_slice(cur.next_n_u8s(32).unwrap());
            let mut other2 = [0; 6];
            other2.copy_from_slice(cur.next_n_u8s(6).unwrap());
            TrackData { num_frames, other, other2 }
        }
    }

    let tracks = info_block::read::<TrackData>(end)?
        .map(|(data, name)| {
            Ok(MaterialTrack {
                name,
                num_frames: data.num_frames
            })
        })
        .collect::<Result<Vec<MaterialTrack>>>()?;

    Ok(MaterialAnimation {
        name,
        num_frames,
        tracks,
    })
}
