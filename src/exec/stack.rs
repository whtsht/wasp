#[cfg(not(feature = "std"))]
use crate::lib::*;

use crate::binary::ValType;
use alloc::rc::Rc;
use core::fmt;
use core::mem::size_of;

use super::runtime::Instance;

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

impl Value {
    #[inline]
    pub const fn size(&self) -> usize {
        match self {
            Value::I32(_) => 4,
            Value::I64(_) => 8,
            Value::F32(_) => 4,
            Value::F64(_) => 8,
            _ => todo!(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Label {
    pub typeidx: usize,
}

#[derive(Debug, PartialEq)]
pub struct Frame {
    pub instance: Rc<Instance>,
    pub local: Vec<Value>,
    pub n: usize,
}

#[derive(Debug, PartialEq)]
pub struct Stack {
    values: VStack,
    labels: Vec<Label>,
    frames: Vec<Frame>,
}

impl Stack {
    pub fn new() -> Self {
        Self {
            values: VStack::new(),
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

    pub fn push_value<T: StackValue>(&mut self, value: T) {
        T::push(&mut self.values, value);
    }

    pub fn push_label(&mut self, lable: Label) {
        self.labels.push(lable);
    }

    pub fn push_frame(&mut self, frame: Frame) {
        self.frames.push(frame);
    }

    pub fn pop_value<T: StackValue>(&mut self) -> T {
        T::pop(&mut self.values)
    }

    pub fn pop_label(&mut self) -> Label {
        self.labels.pop().unwrap()
    }

    pub fn pop_frame(&mut self) -> Frame {
        self.frames.pop().unwrap()
    }

    pub fn top_frame(&mut self) -> &Frame {
        self.frames.last().unwrap()
    }

    pub fn top_frame_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().unwrap()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct VStack {
    bytes: Vec<u8>,
    types: Vec<ValType>,
}

impl VStack {
    pub fn new() -> Self {
        Self {
            bytes: vec![],
            types: vec![],
        }
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    pub fn push<T: StackValue>(&mut self, value: T) {
        T::push(self, value);
    }

    pub fn pop<T: StackValue>(&mut self) -> T {
        T::pop(self)
    }

    fn erase_top(&mut self, size: usize) {
        self.types.pop();
        self.bytes.truncate(self.top_addr() - size);
    }

    fn top_addr(&self) -> usize {
        self.bytes.len()
    }

    fn top_bytes<'a, T>(&'a self) -> T
    where
        T: TryFrom<&'a [u8]>,
        T::Error: fmt::Debug,
    {
        let len = self.top_addr() - size_of::<T>();
        self.bytes[len..].try_into().expect("top bytes")
    }
}

pub trait StackValue: Sized {
    fn push(stack: &mut VStack, value: Self);
    fn top(stack: &VStack) -> Self;
    fn pop(stack: &mut VStack) -> Self;
}

macro_rules! impl_stack_value {
    ($type:ty, $val_type:expr) => {
        impl StackValue for $type {
            fn push(stack: &mut VStack, value: Self) {
                stack.bytes.extend(value.to_le_bytes());
                stack.types.push($val_type);
            }

            fn top(stack: &VStack) -> Self {
                assert_eq!(stack.types.last(), Some(&$val_type));
                <$type>::from_le_bytes(stack.top_bytes())
            }

            fn pop(stack: &mut VStack) -> Self {
                let v = Self::top(stack);
                stack.erase_top(size_of::<Self>());
                v
            }
        }
    };
}

impl_stack_value!(i32, ValType::I32);
impl_stack_value!(i64, ValType::I64);
impl_stack_value!(f32, ValType::F32);
impl_stack_value!(f64, ValType::F64);

impl StackValue for Value {
    fn push(stack: &mut VStack, value: Self) {
        match value {
            Value::I32(value) => stack.push(value),
            Value::I64(value) => stack.push(value),
            Value::F32(value) => stack.push(value),
            Value::F64(value) => stack.push(value),
            _ => todo!(),
        }
    }

    fn top(stack: &VStack) -> Self {
        match &stack.types.last().unwrap() {
            ValType::I32 => Value::I32(i32::top(stack)),
            ValType::I64 => Value::I64(i64::top(stack)),
            ValType::F32 => Value::F32(f32::top(stack)),
            ValType::F64 => Value::F64(f64::top(stack)),
            _ => todo!(),
        }
    }

    fn pop(stack: &mut VStack) -> Self {
        let v = Self::top(stack);
        stack.erase_top(v.size());
        v
    }
}

#[cfg(test)]
mod tests {
    use alloc::rc::Rc;

    use crate::exec::{
        runtime::Instance,
        stack::{Frame, Label, Value},
    };

    use super::Stack;

    #[test]
    fn stack_value() {
        let mut stack = Stack::new();
        stack.push_value(-3 as i32);
        stack.push_value(i32::MAX as i64 + 10);
        stack.push_value(1.32 as f32);
        stack.push_value(1.64 as f64);

        assert_eq!(stack.pop_value::<f64>(), 1.64);
        assert_eq!(stack.pop_value::<f32>(), 1.32);
        assert_eq!(stack.pop_value::<i64>(), i32::MAX as i64 + 10);
        assert_eq!(stack.pop_value::<i32>(), -3);

        assert!(stack.is_empty())
    }

    #[test]
    #[should_panic]
    fn stack_value_err() {
        let mut stack = Stack::new();
        stack.push_value(3 as i32);
        stack.push_value(3 as i32);

        assert_eq!(stack.pop_value::<i64>(), 9);
    }

    #[test]
    fn stack_label() {
        let label1 = Label { typeidx: 0 };
        let label2 = Label { typeidx: 1 };
        let mut stack = Stack::new();
        stack.push_label(label1);
        stack.push_label(label2);
        assert_eq!(stack.pop_label(), Label { typeidx: 1 });
        assert_eq!(stack.pop_label(), Label { typeidx: 0 });

        assert!(stack.is_empty());
    }

    #[test]
    fn stack_frame() {
        let frame1 = Frame {
            instance: Rc::new(Instance::default()),
            local: vec![],
            n: 1,
        };
        let frame2 = Frame {
            instance: Rc::new(Instance::default()),
            local: vec![Value::I32(1), Value::F32(3.0)],
            n: 0,
        };
        let mut stack = Stack::new();
        stack.push_frame(frame1);
        stack.push_frame(frame2);

        assert_eq!(
            stack.pop_frame(),
            Frame {
                instance: Rc::new(Instance::default()),
                local: vec![Value::I32(1), Value::F32(3.0)],
                n: 0,
            }
        );
        assert_eq!(
            stack.pop_frame(),
            Frame {
                instance: Rc::new(Instance::default()),
                local: vec![],
                n: 1,
            }
        );
        assert!(stack.is_empty());
    }
}
