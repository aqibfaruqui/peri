extern crate chumsky;

use std::process;
use std::env;
use std::fs;

mod frontend;
mod analysis;
mod ir;
mod backend;

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct Config {
    source: String,
    destination: String,
}

impl Config {
    fn build(
        args: impl Iterator<Item = String>,
    ) -> Result<Config, String> {
        let flags = Self::parse_flags(args)?;

        let source = flags.source
            .ok_or("no source file provided")?;

        let destination = flags.destination
            .unwrap_or_else(|| "out.s".to_string());

        Ok(Config { source, destination })
    }

    fn parse_flags(
        args: impl Iterator<Item = String>,
    ) -> Result<Flags, String> {
        let mut flags = Flags::default();
        let mut args = args.skip(1);

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    Self::print_usage();
                    process::exit(0);
                }

                "--version" | "-v" => {
                    println!("peric {}", VERSION);
                    process::exit(0);
                }

                "-o" => {
                    flags.destination = Some(
                        args.next().ok_or("expected destination filename after '-o'")?
                    );
                }
                
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown option '{}'", arg));
                }
                _ => {
                    if flags.source.is_some() {
                        return Err("unexpected extra argument".to_string());
                    }
                    if !arg.ends_with(".peri") {
                        return Err("source file must have a .peri extension".to_string());
                    }
                    flags.source = Some(arg);
                }
            }
        }

        Ok(flags)
    }

    fn print_usage() {
        eprintln!("peric {}", VERSION);
        eprintln!();
        eprintln!("Usage: peric [OPTIONS] <source.peri> <destination.s>");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -o <file>    Write output assembly to <file> (default: out.s)");
        eprintln!("  --help       Print this help message");
        eprintln!("  --version    Print version information");
    }
}

#[derive(Default)]
struct Flags {
    source: Option<String>,
    destination: Option<String>,
}

fn main() {
    let config = Config::build(env::args()).unwrap_or_else(|err| {
        eprintln!("Error: {}", err);
        eprintln!("Run 'peric --help' for usage information.");
        process::exit(1);
    });

    let source_code = fs::read_to_string(&config.source).unwrap_or_else(|err| {
        eprintln!("Error reading '{}': {}", config.source, err);
        process::exit(1);
    });

    let ast = frontend::parser::parse(&source_code).unwrap_or_else(|err| {
        eprintln!("Parse error: {:?}", err);
        process::exit(1);
    });

    if let Err(errors) = analysis::semantic::check(&ast) {
        for err in &errors {
            eprintln!("Semantic error: {}", err);
        }
        process::exit(1);
    }

    let ir = ir::lower::lower(&ast);

    if let Err(err) = analysis::typestate::check(&ast, &ir) {
        eprintln!("Typestate error: {}", err);
        process::exit(1);
    }

    let output = backend::generate(&ir).unwrap_or_else(|err| {
        eprintln!("Code generation error: {}", err);
        process::exit(1);
    });

    fs::write(&config.destination, output).unwrap_or_else(|err| {
        eprintln!("Error writing '{}': {}", config.destination, err);
        process::exit(1);
    });

    println!("Compilation successful!");
}