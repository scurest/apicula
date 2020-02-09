use std::ffi::OsString;
use std::path::Path;
use std::process::exit;
use wild;

mod parse;
use self::parse::*;
pub use self::parse::Args;


pub fn parse_cli_args() -> Args {
    let args_os: Vec<OsString> = wild::args_os().collect();
    let mut p = Parse::new(args_os);

    let _exe_name = p.argv.next();

    let arg = p.argv.next();
    let arg = match arg {
        Some(x) => x,
        None => show_usage_and_exit(),
    };
    let arg = match arg.to_str() {
        Some(x) => x,
        None => {
            error!("don't understand {:?}", arg);
            info!("use `apicula help` for help");
            exit(1);
        }
    };
    match arg {
        "x" | "extract" => {
            p.args.subcommand = "extract";
            extract(&mut p)
        }
        "v" | "view" => {
            p.args.subcommand = "view";
            view(&mut p);
        }
        "c" | "convert" => {
            p.args.subcommand = "convert";
            convert(&mut p);
        }
        "i" | "info" => {
            p.args.subcommand = "info";
            info(&mut p);
        }
        "help" => {
            p.args.subcommand = "help";
            help(&mut p);
        }
        "-h" | "--help" => show_usage_and_exit(),
        "-V" | "--version" => version(),
        _ => {
            error!("don't understand {}", arg);
            info!("use `apicula help` for help");
            exit(1);
        }
    }

    p.args
}

fn show_opts_help(opts: &[&Opt]) {
    println!("  Options:");
    for opt in opts {
        if !opt.help.is_empty() {
            println!("    {}", opt.help);
        }
    }
}

static HELP_OPT: Opt = Opt {
    short: "h", long: "help", flag: true,
    help: "-h, --help                show help",
};
static OUTPUT_OPT: Opt = Opt {
    short: "o", long: "output", flag: false,
    help: "-o, --output <outdir>     place output files here (will be created)",
};
static ALL_ANIMATIONS_OPT: Opt = Opt {
    short: "", long: "all-animations", flag: true,
    help: "--all-animations          don't guess which joint anims go with a model, just try them all",
};
static MORE_TEXTURES_OPT: Opt = Opt {
    short: "", long: "more-textures", flag: true,
    help: "--more-textures           try to dump images for unused textures too",
};
static FORMAT_OPT: Opt = Opt {
    short: "f", long: "format", flag: false,
    help: "-f, --format <format>     output model format (dae, glb, gltf)",
};


fn version() -> ! {
    println!("apicula {}", ::VERSION);
    exit(0)
}

fn show_usage_and_exit() -> ! {
    print!(concat!(
        "\n",
        "  Usage: apicula <command> ...\n",
        "\n",
        "  Viewer/converter for Nintendo DS Nitro model files (.nsbmd).\n",
        "\n",
        "  Example:\n",
        "\n",
        "    # extract files from a ROM\n",
        "    apicula extract rom.nds -o nitro-files\n",
        "    # view all extracted models\n",
        "    apicula view nitro-files\n",
        "    # convert model to collada\n",
        "    apicula convert nitro-files/my-model.nsbmd -o my-model\n",
        "\n",
        "  Commands:\n",
        "\n",
        "    extract        Extract Nitro files\n",
        "    view           Nitro model viewer\n",
        "    convert        Convert Nitro models to .dae/.gltf\n",
        "    info           Display debugging info for Nitro files\n",
        "    help           Display help\n",
        "\n",
        "  Run `apicula help COMMAND` for more information on specific commands.\n",
        "  Visit https://github.com/scurest/apicula/wiki for more info.\n",
        "\n",
    ));
    exit(0);
}

fn help(p: &mut Parse) -> ! {
    let arg = p.argv.next();
    let arg = arg.as_ref().and_then(|arg| arg.to_str());
    match arg {
        Some("extract") => show_extract_help_and_exit(),
        Some("view") => show_view_help_and_exit(),
        Some("convert") => show_convert_help_and_exit(),
        Some("info") => show_info_help_and_exit(),
        _ => show_usage_and_exit(),
    }
}


static EXTRACT_OPTS: &[&Opt] = &[&OUTPUT_OPT, &HELP_OPT];

fn extract(p: &mut Parse) {
    parse_opts(p, EXTRACT_OPTS);
    if p.args.flags.contains(&"help") { show_extract_help_and_exit(); }
    if p.args.free_args.len() == 0 {
        error!("pass the file you want to extract from");
        exit(1);
    }
    if p.args.free_args.len() > 1 {
        error!("too many input files! I only need one");
        exit(1);
    }
    check_output_dir(p);
}

fn show_extract_help_and_exit() -> ! {
    print!(concat!(
        "\n",
        "  Usage: apicula extract <input> -o <outdir>\n",
        "\n",
        "  Extract Nitro files (models, textures, animations, etc.) from <input>.\n",
        "  Try it on an .nds rom.\n",
        "\n",
    ));
    show_opts_help(EXTRACT_OPTS);
    println!();
    exit(0)
}


static INFO_OPTS: &[&Opt] = &[&HELP_OPT];

fn info(p: &mut Parse) {
    parse_opts(p, INFO_OPTS);
    if p.args.flags.contains(&"help") { show_info_help_and_exit(); }
    check_nitro_input(p);
}

fn show_info_help_and_exit() -> ! {
    print!(concat!(
        "\n",
        "  Usage: apicula info <input...>\n",
        "\n",
        "  Display debugging info for the given set of Nitro files.\n",
        "  You can lookup how textures and palettes were resolved here.\n",
        "  But this is mostly useful for developers.\n",
        "\n",
    ));
    show_opts_help(INFO_OPTS);
    exit(0);
}


static VIEW_OPTS: &[&Opt] = &[&ALL_ANIMATIONS_OPT, &HELP_OPT];

fn view(p: &mut Parse) {
    parse_opts(p, VIEW_OPTS);
    if p.args.flags.contains(&"help") { show_view_help_and_exit(); }
    check_nitro_input(p);
}

fn show_view_help_and_exit() -> ! {
    print!(concat!(
        "\n",
        "  Usage: apicula view <input...>\n",
        "\n",
        "  Open the 3D model viewer.\n",
        "  Each <input> can be either a Nitro file or a directory of Nitro files.\n",
        "\n",
    ));
    show_opts_help(VIEW_OPTS);
    println!();
    exit(0)
}


static CONVERT_OPTS: &[&Opt] = &[&OUTPUT_OPT, &FORMAT_OPT, &MORE_TEXTURES_OPT, &ALL_ANIMATIONS_OPT, &HELP_OPT];

fn convert(p: &mut Parse) {
    parse_opts(p, CONVERT_OPTS);
    if p.args.flags.contains(&"help") { show_convert_help_and_exit(); }
    check_nitro_input(p);
    check_format(p);
    check_output_dir(p);
}

fn show_convert_help_and_exit() -> ! {
    print!(concat!(
        "\n",
        "  Usage: apicula convert <input>... -o <ourdir>\n",
        "\n",
        "  Converts Nitro models to .dae/.gltf. Default is .dae.\n",
        "  The textures and animations on each model will be the same as with `apicula view`.\n",
        "\n",
    ));
    show_opts_help(CONVERT_OPTS);
    println!();
    exit(0);
}


fn check_nitro_input(p: &Parse) {
    if p.args.free_args.is_empty() {
        error!("give me some input files");
        exit(1);
    }
}

fn check_format(p: &Parse) {
    let format = p.args.get_opt("format");
    if let Some(format) = format {
        match format.to_str() {
            Some("dae") | Some("glb") | Some("gltf") => (),
            _ => {
                error!("bad output format, should be one of: dae glb gltf");
                exit(1);
            }
        }
    }
}

fn check_output_dir(p: &Parse) {
    let output = p.args.get_opt("output");
    if output.is_none() {
        error!("where do I put the output files? Pass it with --output");
        exit(1);
    }
    let output = output.unwrap();
    if Path::new(output).exists() {
        error!("output directory already exists, choose a different one");
        exit(1);
    }
}
