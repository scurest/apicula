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

apicula is a tool for the [NSBMD 3D model
files](https://wiki.vg-resource.com/wiki/Nintendo_DS#NITRO_File_Formats) found
in many Nintendo DS games, eg. .nsbmd for models, .nsbca for animations, .nsbtx
for textures, etc. Models can be extracted from ROMs, viewed, and converted to
COLLADA.

* [Tutorial](https://github.com/scurest/apicula/wiki/TUTORIAL)
* [Hallow's tutorial on VG Resource](https://www.vg-resource.com/thread-32332.html)
* [Common Blender issues](https://github.com/scurest/apicula/wiki/IMPORT:-Blender)
* [Programmer's documentation on .nsbXX files](https://raw.githubusercontent.com/scurest/nsbmd_docs/master/nsbmd_docs.txt)


### Downloads

Pre-built binaries are provided for Windows:

* [apicula for Windows, 64-bit](https://s3.amazonaws.com/apicula/apicula-latest-x86_64-pc-windows-msvc.zip)
* [apicula for Windows, 32-bit](https://s3.amazonaws.com/apicula/apicula-latest-i686-pc-windows-msvc.zip)

These are built automatically off the latest `master`. You may need one of the Visual Studio
Redistributable packages installed.


### Building

Make sure [Rust (1.34+) is installed](https://rustup.rs/) and [build the usual
way](https://doc.rust-lang.org/cargo/guide/working-on-an-existing-project.html)

    $ git clone https://github.com/scurest/apicula.git
    $ cd apicula
    $ cargo b --release
    $ target/release/apciula -V


### Usage

To search a ROM or other packed file for .nsbXX files and extract them

    apicula extract <INPUT FILE> -o <OUTPUT DIR>

To view models

    apicula view <NITRO FILES>

To convert models to COLLADA `.dae` files

    apicula convert <NITRO FILES> -o <OUTPUT DIR>

To convert models to glTF `.glb` files

    apicula convert -f=glb <NITRO FILES> -o <OUTPUT DIR>

To get technical information about the given .nsbXX files

    apicula info <NITRO FILES>

To receive further help

    apicula help

See also the [tutorial](https://github.com/scurest/apicula/wiki/TUTORIAL) on the
process of extracting .nsbXX files from a ROM, converting them to COLLADA, and
importing them into Blender.


### Compatibility

apicula recognized these file formats

* `.nsbmd`, `.BMD`, or `.BMD0` contain 3D models, and often their textures and
  palettes
* `.nsbca`, `.BCA`, or `.BCA0` contain skeletal animations
* `.nsbtx`, `.BTX`, or `.BTX0` contain textures and palettes
* `.nsbtp`, `.BTP`, or `.BTP0` contain pattern animations, which change the
  textures in a material

Pattern animations are supported in the viewer and extractor, but not in the
converter (neither COLLADA nor glTF support animations that change a material's
textures).

Importing apicula's COLLADA files has been tested in Blender and Maya.


### Special Thanks

* **kiwi.ds**, for models and documentation for Nitro formats. All NDS model viewers seem to be
  derived from this one. Now defunct.

* **Gericom and [MKDS Course Modifier](https://gbatemp.net/threads/mkds-course-modifier.299444/)**,
  for animation information, especially for the meaning of the basis rotations.

* **Lowlines and [Console
  Tool](https://web.archive.org/web/20180319005030/http://llref.emutalk.net/projects/ctool/)**,
  for animations and documentation for Nitro formats. I also used Console Tool
  for extracting files from ROMs.

* **Barubary's [DSDecmp](https://github.com/Barubary/dsdecmp)** for NDS
  decompression algorithms.

* **[GBATEK](http://problemkaputt.de/gbatek.htm#ds3dvideo)**, for DS hardware documentation.

* **[deSmuME](http://desmume.org/)**, for the DS debugger. `_3D_LOG_EXEC` and the GDB stub were
  invaluable.


### License

0BSD
