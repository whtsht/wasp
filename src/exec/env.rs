use super::stack::Stack;

pub trait Env {
    fn call(&mut self, name: &str, stack: &mut Stack);
}

#[derive(Debug)]
pub struct DebugEnv {}

impl Env for DebugEnv {
    fn call(&mut self, name: &str, stack: &mut Stack) {
        match name {
            "start" => {
                println!("hello world");
            }
            "print" => {
                println!("{}", stack.pop_value::<i32>());
            }
            _ => {
                panic!("unknown function: {}", name);
            }
        }
    }
}
