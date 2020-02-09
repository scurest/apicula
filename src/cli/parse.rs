//! Mini-lib for CLI argument parsing.
//! Handles flags: -x, --x
//! And options with args: -xfoo, -x=foo, -x foo, --x=foo, --x foo
use std;
use std::ffi::{OsString, OsStr};


pub struct Args {
    pub subcommand: &'static str,
    pub free_args: Vec<OsString>,
    pub opt_args: Vec<(&'static str, OsString)>,
    pub flags: Vec<&'static str>,
}

impl Args {
    pub fn get_opt(&self, long: &'static str) -> Option<&OsStr> {
        self.opt_args.iter().find(|p| p.0 == long).map(|p| p.1.as_os_str())
    }
}

pub struct Opt {
    pub short: &'static str,
    pub long: &'static str,
    pub flag: bool,
    pub help: &'static str,
}

pub struct Parse {
    pub argv: std::vec::IntoIter<OsString>,
    pub args: Args,
}

impl Parse {
    pub fn new(args_os: Vec<OsString>) -> Parse {
        Parse {
            argv: args_os.into_iter(),
            args: Args {
                subcommand: "",
                free_args: vec![],
                opt_args: vec![],
                flags: vec![],
            }
        }
    }
}

pub fn parse_opts(p: &mut Parse, opts: &[&Opt]) {
    'argv: while let Some(os_arg) = p.argv.next() {
        if let Some(arg) = os_arg.to_str() {

            let mut arg_opt_name; // "x" in -x or --x
            let mut arg_param = None; // "param" in -x=param or -xparam
            let arg_is_long; // --x vs -x

            if arg.starts_with("--") {
                arg_opt_name = &arg[2..];
                arg_is_long = true;
                if let Some(i) = arg_opt_name.find('=') {
                    arg_param = Some(&arg_opt_name[i+1..]);
                    arg_opt_name = &arg_opt_name[..i];
                }
            } else if arg.starts_with("-") {
                arg_opt_name = &arg[1..];
                arg_is_long = false;
                match arg_opt_name.char_indices().nth(1) {
                    Some((i, c)) if c == '=' => { // -x=foo
                        arg_param = Some(&arg_opt_name[i+1..]);
                        arg_opt_name = &arg_opt_name[..i];
                    }
                    Some((i, _)) => { // -xfoo
                        if !&arg_opt_name[i..].is_empty() {
                            arg_param = Some(&arg_opt_name[i..]);
                        }
                        arg_opt_name = &arg_opt_name[..i];
                    }
                    None => (),
                }
            } else {
                p.args.free_args.push(os_arg);
                continue 'argv;
            }

            for opt in opts {
                let matches = match arg_is_long {
                    true => arg_opt_name == opt.long,
                    false => !opt.short.is_empty() && arg_opt_name == opt.short,
                };
                if !matches { continue; }

                if opt.flag {

                    if arg_param.is_some() {
                        error!("flag --{} doesn't take an argument", opt.long);
                        suggest_help_and_exit();
                    }
                    p.args.flags.push(opt.long);
                    continue 'argv;

                } else {

                    let param: OsString = match arg_param {
                        Some(s) => s.into(),
                        None => {
                            match p.argv.next() {
                                Some(s) => s,
                                None => {
                                    error!("expected an argument after --{}", opt.long);
                                    suggest_help_and_exit();
                                }
                            }
                        }
                    };
                    if p.args.get_opt(opt.long).is_some() {
                        error!("you already passed --{}", opt.long);
                        suggest_help_and_exit();
                    }
                    p.args.opt_args.push((opt.long, param));
                    continue 'argv;

                }
            }

            error!("don't understand option {}{}",
                if arg_is_long { "--" } else { "-" },
                arg_opt_name,
            );
            suggest_help_and_exit();

        } else {
            p.args.free_args.push(os_arg);
        }
    }
}

pub fn suggest_help_and_exit() -> ! {
    info!("Pass --help if you need help.");
    std::process::exit(1)
}
