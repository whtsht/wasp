#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::runtime::Addr;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Value {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    // TODO Vector
    NullRef,
    FuncRef,
    ExternRef,
}

impl From<Value> for i32 {
    fn from(value: Value) -> Self {
        if let Value::I32(value) = value {
            value
        } else {
            unreachable!()
        }
    }
}

impl Into<Value> for i32 {
    fn into(self) -> Value {
        Value::I32(self)
    }
}

impl From<Value> for i64 {
    fn from(value: Value) -> Self {
        if let Value::I64(value) = value {
            value
        } else {
            unreachable!("{:?}", value)
        }
    }
}

impl Into<Value> for i64 {
    fn into(self) -> Value {
        Value::I64(self)
    }
}

impl From<Value> for f32 {
    fn from(value: Value) -> Self {
        if let Value::F32(value) = value {
            value
        } else {
            unreachable!()
        }
    }
}

impl Into<Value> for f32 {
    fn into(self) -> Value {
        Value::F32(self)
    }
}

impl From<Value> for f64 {
    fn from(value: Value) -> Self {
        if let Value::F64(value) = value {
            value
        } else {
            unreachable!()
        }
    }
}

impl Into<Value> for f64 {
    fn into(self) -> Value {
        Value::F64(self)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Label {
    pub n: usize,
    pub offset: usize,
    pub pc: usize,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Frame {
    pub n: usize,
    pub instance_addr: Addr,
    pub local: Vec<Value>,
    pub pc: usize,
}

#[derive(Debug, PartialEq, Default, Clone)]
pub struct Stack {
    values: Vec<Value>,
    pub labels: Vec<Label>,
    frames: Vec<Frame>,
}

impl Stack {
    pub fn new() -> Self {
        Self {
            values: vec![],
            labels: vec![],
            frames: vec![],
        }
    }

    pub fn values_len(&self) -> usize {
        self.values.len()
    }

    pub fn labels_len(&self) -> usize {
        self.labels.len()
    }

    pub fn frames_len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.labels.is_empty() && self.frames.is_empty()
    }

    pub fn push_value<T: Into<Value>>(&mut self, value: T) {
        self.values.push(value.into());
    }

    pub fn push_label(&mut self, lable: Label) {
        self.labels.push(lable);
    }

    pub fn push_frame(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    pub fn pop_value<T: From<Value>>(&mut self) -> T {
        self.values.pop().unwrap().into()
    }

    pub fn pop_label(&mut self) -> Label {
        self.labels.pop().unwrap()
    }

    pub fn pop_frame(&mut self) -> Frame {
        self.frames.pop().unwrap()
    }

    pub fn set_params(&mut self, params: Vec<Value>) {
        self.values = params;
    }

    pub fn get_returns(&mut self) -> Vec<Value> {
        self.values.drain(..).collect()
    }

    pub fn th_label(&self, th: usize) -> Label {
        self.labels[self.labels.len() - 1 - th].clone()
    }

    pub fn top_frame(&mut self) -> &Frame {
        self.frames.last().unwrap()
    }

    pub fn top_frame_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::exec::stack::{Frame, Label, Value};

    use super::Stack;

    #[test]
    fn stack_label() {
        let label1 = Label {
            n: 0,
            offset: 0,
            pc: 10,
        };
        let label2 = Label {
            n: 0,
            offset: 1,
            pc: 0,
        };
        let mut stack = Stack::new();
        stack.push_label(label1);
        stack.push_label(label2);
        assert_eq!(
            stack.pop_label(),
            Label {
                n: 0,
                offset: 1,
                pc: 0
            }
        );
        assert_eq!(
            stack.pop_label(),
            Label {
                n: 0,
                offset: 0,
                pc: 10
            }
        );

        assert!(stack.is_empty());
    }

    #[test]
    fn stack_frame() {
        let frame1 = Frame {
            n: 0,
            instance_addr: 0,
            local: vec![],
            pc: 0,
        };
        let frame2 = Frame {
            n: 0,
            instance_addr: 0,
            local: vec![Value::I32(1), Value::F32(3.0)],
            pc: 0,
        };
        let mut stack = Stack::new();
        stack.push_frame(frame1);
        stack.push_frame(frame2);

        assert_eq!(
            stack.pop_frame(),
            Frame {
                n: 0,
                instance_addr: 0,
                local: vec![Value::I32(1), Value::F32(3.0)],
                pc: 0
            }
        );
        assert_eq!(
            stack.pop_frame(),
            Frame {
                n: 0,
                instance_addr: 0,
                local: vec![],
                pc: 0
            }
        );
        assert!(stack.is_empty());
    }
}
