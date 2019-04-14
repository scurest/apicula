<p align=center><img src="frontispiece.gif" alt="Ore ga Omae o Mamoru model in Blender"></p>
<h3 align=center>apicula</h3>
<p align=center>
<a href="https://ci.appveyor.com/project/scurest/apicula"><img src="https://ci.appveyor.com/api/projects/status/bavh9qh25mbta41x?svg=true" alt="Build status"></a>
<a href="https://github.com/XAMPPRocky/tokei"><img src="https://tokei.rs/b1/github/scurest/apicula?category=code" alt="Lines of code"></a>
<a href="LICENSE"><img src="https://img.shields.io/badge/license-0BSD-lightgrey.svg" alt="license 0BSD"></a>
<br>
Rip models from DS games.
</p>

-----

Many Nintendo DS games used [Nitro
files](https://wiki.vg-resource.com/wiki/Nintendo_DS#NITRO_File_Formats) for
assets, like NSBMD files for 3D models. apicula can extract these models and
their associated textures and animations from a ROM or memory dump, display them
in its model viewer, and convert them to the common 3D formats COLLADA and glTF
for importing into content-creation tools like Blender.

* [Tutorial](https://github.com/scurest/apicula/wiki/TUTORIAL)
* [Hallow's tutorial on VG Resource](https://www.vg-resource.com/thread-32332.html)
* [Common Blender issues](https://github.com/scurest/apicula/wiki/BLENDER)


### Downloads

Pre-built binaries are provided for Windows:

* [apicula for Windows, 64-bit](https://s3.amazonaws.com/apicula/apicula-latest-x86_64-pc-windows-msvc.zip)
* [apicula for Windows, 32-bit](https://s3.amazonaws.com/apicula/apicula-latest-i686-pc-windows-msvc.zip)

These are built automatically off the latest `master`. You may need one of the Visual Studio
Redistributable packages installed.


### Building

Make sure [Rust is installed](https://rustup.rs/) and [build the usual way](https://doc.rust-lang.org/cargo/guide/working-on-an-existing-project.html)

    $ git clone https://github.com/scurest/apicula.git
    $ cd apicula
    $ cargo b --release
    $ target/release/apciula -V


### Usage

To search a ROM or other packed file for Nitro files and extract them

    apicula extract <INPUT FILE> -o <OUTPUT DIR>

To view models

    apicula view <NITRO FILES>

To convert models to COLLADA `.dae` files

    apicula convert <NITRO FILES> -o <OUTPUT DIR>

To convert models to glTF `.glb` files

    apicula convert -f=glb <NITRO FILES> -o <OUTPUT DIR>

To get technical information about the given Nitro files

    apicula info <NITRO FILES>

To receive further help

    apicula help

See also the [tutorial](https://github.com/scurest/apicula/wiki/TUTORIAL) on the
process of extracting Nitro files from a ROM, converting them to COLLADA, and
importing them into Blender.


### Compatibility

apicula recognized these file formats

* `.nsbmd`, `.BMD`, or `.BMD0` contain 3D models, and often their associated
  textures and palettes
* `.nsbca`, `.BCA`, or `.BCA0` contain skeletal animations
* `.nsbtx`, `.BTX`, or `.BTX0` contain textures and palettes
* `.nsbtp`, `.BTP`, or `.BTP0` contain pattern animations, which change the
  textures in a material

Pattern animations are supported in the viewer and extractor, but not in the
converter (neither COLLADA nor glTF support animations that change a material's
textures).

Exporting is primarily tested with the following games:

* Kingdom Hearts: 358/2 Days
* Ore ga Omae o Mamoru
* Rune Factory 3: A Fantasy Harvest Moon

Importing the resultant COLLADA files has been tested in the following programs:

* Blender 2.79
* Godot

If you can test in others (Maya, 3DS Max), that would be appreciated :)


### Special Thanks

* **kiwi.ds**, for models and documentation for Nitro formats. All NDS model viewers seem to be
  derived from this one. Now defunct.

* **Gericom and [MKDS Course Modifier](https://gbatemp.net/threads/mkds-course-modifier.299444/)**,
  for animation information, especially for the meaning of the basis rotations.

* **Lowlines and [Console Tool](http://llref.emutalk.net/projects/ctool/)**, for animations and
  documentation for Nitro formats. I also use Console Tool for extracting files from DS ROMs.

* **[GBATEK](http://problemkaputt.de/gbatek.htm#ds3dvideo)**, for DS hardware documentation.

* **[deSmuME](http://desmume.org/)**, for the DS debugger. `_3D_LOG_EXEC` and the GDB stub were
  invaluable.


### License

0BSD
