error_chain! {
    foreign_links {
        Fmt(::std::fmt::Error);
        Io(::std::io::Error);
        TimeFmt(::time::ParseError);
        Png(::pnglib::EncodingError);
        GliumVertexCreate(::glium::vertex::BufferCreationError);
        GliumIndexCreate(::glium::index::BufferCreationError);
        GliumTextureCreate(::glium::texture::TextureCreationError);
    }
}

macro_rules! check {
    ($b:expr) => {
        if !$b {
            error!("expected: {})", stringify!($b));
            return Err("sanity check failed".into());
        }
    };
}
