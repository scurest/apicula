# apicula

[![Build status](https://ci.appveyor.com/api/projects/status/bavh9qh25mbta41x?svg=true)](https://ci.appveyor.com/project/scurest/apicula)
[![Lines of code](https://tokei.rs/b1/github/scurest/apicula)](https://github.com/Aaronepower/tokei)

A program to convert Nintendo DS models and animations to COLLADA.

![Enemy walk-cycle from Ore ga Omae o Mamoru, imported into Blender](http://scurest.github.io/apicula/e07BWalk.gif)

Many NDS games used Nintendo's Nitro SDK format for models and animations. This program
allow you to view these files and convert them to COLLADA `.dae` files for importing into
DCC programs, like Blender.

## Downloads

Pre-built binaries are provided for Windows:

* [apicula-latest-i686-pc-windows-msvc](https://s3.amazonaws.com/apicula/apicula-latest-i686-pc-windows-msvc.zip) (Windows, 32-bit)
* [apicula-latest-x86_64-pc-windows-msvc](https://s3.amazonaws.com/apicula/apicula-latest-x86_64-pc-windows-msvc.zip) (Windows, 64-bit)

These are built automatically off the latest `master`. You may need one of the Visual Studio Redistributable packages installed.

## Building

1. If you don't already have it, install [Rust](https://www.rust-lang.org/), either through
your package manager or by following the installation instructions on the Rust site.

2. Clone the git repo with

        $ git clone https://github.com/scurest/apicula.git

3. Change into the `apicula` directory and build the project with Cargo

        $ cd apicula
        $ cargo build --release

    This may take awhile.

4. You're done! The binary is located at `apicula/target/release/apicula`.

## Usage

TODO

## Scope

apicula was tested with Nitro files from the following games

* Kingdom Hearts: 358/2 Days
* Ore ga Omae o Mamoru

Importing the COLLADA files we generate has been tested in the following programs:

* Blender
* Godot

If you can test in others (Maya, 3DS Max), that would be appreciated :)

## Special Thanks

* **kiwi.ds** For models and documentation for Nitro formats. All NDS model viewers seem to be derived from here. Now defunct :(
* **Gericom and [MKDS Course Modifier](https://gbatemp.net/threads/mkds-course-modifier.299444/)** For animations, especially for the meaning of the basis rotations.
* **Lowlines and [Console Tool](http://llref.emutalk.net/projects/ctool/)** For animations and documentation for Nitro formats. I also use Console Tool for extracting files from DS ROMs.
* **[deSmuME](http://desmume.org/)** `_3D_LOG_EXEC` and the GDB stub were invaluable.

## License

CC0 
