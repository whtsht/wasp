#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::{runtime::Addr, trap::Trap};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Value {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
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

#[derive(Debug, PartialEq, Clone, Default)]
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

    pub fn values(&self) -> &Vec<Value> {
        &self.values
    }

    pub fn labels(&self) -> &Vec<Label> {
        &self.labels
    }

    pub fn frames(&self) -> &Vec<Frame> {
        &self.frames
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

impl Stack {
    pub fn unop<T, F: Fn(T) -> T>(&mut self, func: F)
    where
        T: From<Value> + Into<Value>,
    {
        let v = self.pop_value::<T>();
        let r = func(v);
        self.push_value(r);
    }

    pub fn binop<T, F: Fn(T, T) -> T>(&mut self, func: F)
    where
        T: From<Value> + Into<Value>,
    {
        let rhs = self.pop_value::<T>();
        let lhs = self.pop_value::<T>();
        let r = func(lhs, rhs);
        self.push_value(r);
    }

    pub fn binop_trap<F: Fn(T, T) -> Result<T, Trap>, T>(&mut self, func: F) -> Result<(), Trap>
    where
        T: From<Value> + Into<Value>,
    {
        let rhs = self.pop_value::<T>();
        let lhs = self.pop_value::<T>();
        let r = func(lhs, rhs)?;
        self.push_value(r);
        Ok(())
    }

    pub fn relop<F: Fn(T, T) -> i32, T>(&mut self, func: F)
    where
        T: From<Value> + Into<Value>,
    {
        let rhs = self.pop_value::<T>();
        let lhs = self.pop_value::<T>();
        let r = func(lhs, rhs);
        self.push_value(r);
    }

    pub fn testop<F: Fn(T) -> i32, T>(&mut self, func: F)
    where
        T: From<Value> + Into<Value>,
    {
        let v = self.pop_value::<T>();
        let r = func(v);
        self.push_value(r);
    }

    pub fn cvtop<F: Fn(T) -> U, T, U>(&mut self, func: F)
    where
        T: From<Value> + Into<Value>,
        U: From<Value> + Into<Value>,
    {
        let t = self.pop_value::<T>();
        let u = func(t);
        self.push_value(u);
    }

    pub fn cvtop_trap<F: Fn(T) -> Result<U, Trap>, T, U>(&mut self, func: F) -> Result<(), Trap>
    where
        T: From<Value> + Into<Value>,
        U: From<Value> + Into<Value>,
    {
        let t = self.pop_value::<T>();
        let u = func(t)?;
        self.push_value(u);
        Ok(())
    }

    pub fn jump(&mut self, l: usize) -> usize {
        let label = self.th_label(l);
        let mut values: Vec<Value> = vec![];
        for _ in 0..label.n {
            values.push(self.pop_value());
        }

        let len = self.values_len() - label.offset;
        for _ in 0..len {
            self.pop_value::<Value>();
        }

        for _ in 0..=l {
            self.pop_label();
        }

        for value in values.into_iter().rev() {
            self.push_value(value);
        }
        label.pc
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
