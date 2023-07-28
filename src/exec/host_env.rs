use super::stack::Stack;

pub trait HostEnv {
    fn call(&mut self, name: &str, stack: &mut Stack);
}

#[derive(Debug)]
pub struct DebugHostEnv {}

impl HostEnv for DebugHostEnv {
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
