# apicula

A program to convert Nintendo DS models and animations to COLLADA.

![Enemy walk-cycle from Ore ga Omae o Mamoru, imported into Blender](http://scurest.github.io/apicula/e07BWalk.gif)

Many NDS games used Nintendo's Nitro SDK format for models and animations. This program
allow you to view these files and convert them to COLLADA `.dae` files for importing into
DCC programs, like Blender.

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

TODO

## License

CC0 
