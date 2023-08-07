// use std::{env, fs::File, io::Read};
//
// use wasp::loader::parser::Parser;

fn main() {
    // let mut args = env::args();
    // if let Some(modname) = args.nth(1) {
    //     let mut file = File::open(modname).expect("failed to read file");
    //     let mut buffer = vec![];
    //     file.read_to_end(&mut buffer).expect("failed to read file");
    //     let module = Parser::new(&buffer)
    //         .module()
    //         .expect("failed to parse module");
    //
    //     let mut runtime = debug_runtime(module).expect("failed to load module");
    //     match runtime.invoke("_start", vec![]) {
    //         Ok(_) => {}
    //         Err(err) => println!("{:?}", err),
    //     }
    // } else {
    //     println!("error: no input files");
    //     std::process::exit(1);
    // }
}
