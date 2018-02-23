# apicula

[![Build status](https://ci.appveyor.com/api/projects/status/bavh9qh25mbta41x?svg=true)](https://ci.appveyor.com/project/scurest/apicula)
[![Lines of code](https://tokei.rs/b1/github/scurest/apicula)](https://github.com/Aaronepower/tokei)

Convert Nintendo DS NSBMD models and animations to COLLADA.

![Enemy walk-cycle from Ore ga Omae o Mamoru, imported into Blender](http://scurest.github.io/apicula/e07BWalk.gif)

Many NDS games used Nintendo's Nitro SDK format for models (NSBMD), textures (NSBTX), and
animations (NSBCA). apicula let's you convert these models to `.dae` files.

## Downloads

Pre-built binaries are provided for Windows:

* [apicula-latest-i686-pc-windows-msvc](https://s3.amazonaws.com/apicula/apicula-latest-i686-pc-windows-msvc.zip) (Windows, 32-bit)
* [apicula-latest-x86_64-pc-windows-msvc](https://s3.amazonaws.com/apicula/apicula-latest-x86_64-pc-windows-msvc.zip) (Windows, 64-bit)

These are built automatically off the latest `master`. You may need one of the Visual Studio
Redistributable packages installed.

## Building

1. If you don't already have it, install [Rust](https://www.rust-lang.org/), either through
your package manager or by following the installation instructions on the Rust site.

2. Clone the git repo with

        $ git clone https://github.com/scurest/apicula.git

3. Change into the `apicula` directory and build the project with Cargo

        $ cd apicula
        $ cargo build --release

    This may take a while.

4. You're done! The binary is located at `apicula/target/release/apicula`.

## Usage

To view a set of Nitro files

    apicula view <INPUT FILES>

To convert a set of Nitro files to COLLADA, placing the generated files in the given directory

    apicula convert <INPUT FILES> -o <OUTPUT DIR>

To extract files from a ROM or other packed file, placing Nitro files in the given directory

    apicula extract <INPUT FILE> -o <OUTPUT DIR>

To get technical information about the given Nitro files

    apicula info <INPUT FILES>

To receive help

    apicula -h

See also a short [tutorial](https://github.com/scurest/apicula/wiki/TUTORIAL) on using apicula
to convert a model and animation to a COLLADA file.

## Compatibility

apicula was primarily tested with Nitro files from the following games

* Kingdom Hearts: 358/2 Days
* Ore ga Omae o Mamoru
* Rune Factory 3: A Fantasy Harvest Moon

Importing the COLLADA files we generate has been tested in the following programs:

* Blender
* Godot

If you can test in others (Maya, 3DS Max), that would be appreciated :)

## Special Thanks

* **kiwi.ds**, for models and documentation for Nitro formats. All NDS model viewers seem to be
  derived from here. Now defunct :(

* **Gericom and [MKDS Course Modifier](https://gbatemp.net/threads/mkds-course-modifier.299444/)**,
  for animation information, especially for the meaning of the basis rotations.

* **Lowlines and [Console Tool](http://llref.emutalk.net/projects/ctool/)**, for animations and
  documentation for Nitro formats. I also use Console Tool for extracting files from DS ROMs.

* **[GBATEK](http://problemkaputt.de/gbatek.htm#ds3dvideo)**, for DS hardware documentation.

* **[deSmuME](http://desmume.org/)**, for the DS debugger. `_3D_LOG_EXEC` and the GDB stub were
  invaluable.

## License

CC0 
