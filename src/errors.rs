error_chain! {
    foreign_links {
        Fmt(::std::fmt::Error);
        Io(::std::io::Error);
        TimeFmt(::time::ParseError);
        Png(::png_crate::EncodingError);
        GliumVertexCreate(::glium::vertex::BufferCreationError);
        GliumIndexCreate(::glium::index::BufferCreationError);
        GliumTextureCreate(::glium::texture::TextureCreationError);
    }
}

macro_rules! check {
    ($b:expr) => {
        if !$b {
            use ::errors::Error;
            use ::errors::ErrorKind;
            Err(Error::from_kind(
                ErrorKind::Msg(format!("expected: {}", stringify!($b)))
            ))
        } else {
            Ok(())
        }
    };
}
