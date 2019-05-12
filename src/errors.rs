error_chain! {
    foreign_links {
        Cur(::util::cur::Error);
        Fmt(::std::fmt::Error);
        Io(::std::io::Error);
        TimeFmt(::time::ParseError);
        Png(::png::EncodingError);
        GliumDisplayCreate(::glium::backend::glutin::DisplayCreationError);
        GliumVertexCreate(::glium::vertex::BufferCreationError);
        GliumIndexCreate(::glium::index::BufferCreationError);
        GliumTextureCreate(::glium::texture::TextureCreationError);
    }
}

macro_rules! check {
    ($b:expr) => {
        if !$b {
            use errors::Error;
            use errors::ErrorKind;
            Err(Error::from_kind(ErrorKind::Msg(format!(
                "expected: {}",
                stringify!($b)
            ))))
        } else {
            Ok(())
        }
    };
}
