extern crate chumsky;

use std::process;
use std::env;
use std::fs;

mod frontend;
mod ir;
mod backend;

struct Config {
    source: String,
    destination: String,
}

impl Config {
    fn build(
        mut args: impl Iterator<Item = String>,
    ) -> Result<Config, &'static str> {
        args.next();

        let source = match args.next() {
            Some(arg) => arg,
            None => return Err("Didn't get a source file path"),
        };

        let destination = match args.next() {
            Some(arg) => arg,
            None => return Err("Didn't get a destination file path"),
        };

        Ok(Config {
            source,
            destination,
        })
    }
}

fn main() {
    let config = Config::build(env::args()).unwrap_or_else(|err| {
        println!("Error parsing arguments: {err}");
        process::exit(1);
    });

    let source_code = fs::read_to_string(&config.source).unwrap_or_else(|err| {
        println!("Error reading source file: {err}");
        process::exit(1);
    });

    let ast = frontend::parser::parse(&source_code).unwrap_or_else(|err| {
        println!("Error parsing source code: {:?}", err);
        process::exit(1);
    });

    // TODO: Implement verification on AST
    // if let Err(err) = ir::verifier::verify(&ast) {
    //     println!("Error verifying program: {err}");
    //     process::exit(1);
    // }

    let output = backend::compile(&ast).unwrap_or_else(|err| {
        println!("Error during compilation backend: {}", err);
        process::exit(1);
    });

    fs::write(&config.destination, output).unwrap_or_else(|err| {
        println!("Error writing to destination file: {err}");
        process::exit(1);
    });

    println!("Compilation successful!");
}