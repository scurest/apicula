use util::bits::BitField;

pub fn fix16(x: u16, sign_bits: u32, int_bits: u32, frac_bits: u32) -> f64 {
    assert!(sign_bits + int_bits + frac_bits <= 16);
    fix32(x as u32, sign_bits, int_bits, frac_bits)
}

pub fn fix32(x: u32, sign_bits: u32, int_bits: u32, frac_bits: u32) -> f64 {
    assert!(sign_bits <= 1);
    assert!(int_bits + frac_bits > 0);
    assert!(sign_bits + int_bits + frac_bits <= 32);
    let x = x.bits(0, sign_bits + int_bits + frac_bits);
    let y = if sign_bits == 0 {
        x as f64
    } else {
        // sign extend
        let sign_mask = (1 << (int_bits + frac_bits)) as u32;
        if x & sign_mask != 0 {
            (x | !(sign_mask - 1)) as i32 as f64
        } else {
            x as f64
        }
    };
    y * 0.5f64.powi(frac_bits as i32)
}
