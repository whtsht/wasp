use super::stack::Stack;

pub trait Env {
    fn call(&mut self, name: &str, stack: &mut Stack);
}

#[derive(Debug)]
#[cfg(feature = "std")]
pub struct DebugEnv {}

#[cfg(feature = "std")]
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
