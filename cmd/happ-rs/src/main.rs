mod assets;
mod cli;
mod composeimport;
mod composeinspect;
mod convert;
mod dyfflike;
mod inspectweb;
mod output;
mod query;
mod service;
mod source;
mod verify;
mod yamlmerge;

use std::process;

fn main() {
    if let Err(err) = service::run() {
        eprintln!("happ failed: {err}");
        process::exit(1);
    }
}
