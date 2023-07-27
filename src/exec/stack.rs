#[cfg(not(feature = "std"))]
use crate::lib::*;

use crate::binary::ValType;
use alloc::rc::Rc;
use core::fmt;
use core::mem::size_of;

use super::runtime::Instance;

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq, Eq)]
pub struct Label {
    pub next: usize,
    pub n: usize,
}

#[derive(Debug, PartialEq)]
pub struct Frame {
    pub instance: Rc<Instance>,
    pub local: Vec<Value>,
    pub n: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Stack {
    bytes: Vec<u8>,
    entries: Vec<StackEntry>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StackEntry {
    Value(ValType),
    Label,
    Frame,
}

impl Stack {
    pub fn new() -> Self {
        Self {
            bytes: vec![],
            entries: vec![],
        }
    }

    pub fn to_usize(&self, start: usize, end: usize) -> usize {
        let len = self.bytes.len();
        usize::from_le_bytes((&self.bytes[len - start..len - end]).try_into().unwrap())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push<T: StackValue>(&mut self, value: T) {
        T::push(self, value);
    }

    pub fn pop<T: StackValue>(&mut self) -> T {
        T::pop(self)
    }

    fn erase_top(&mut self, size: usize) {
        self.entries.pop();
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
    fn push(stack: &mut Stack, value: Self);
    fn top(stack: &Stack) -> Self;
    fn pop(stack: &mut Stack) -> Self {
        let v = Self::top(stack);
        stack.erase_top(size_of::<Self>());
        v
    }
}

macro_rules! impl_stack_value {
    ($type:ty, $val_type:expr) => {
        impl StackValue for $type {
            fn push(stack: &mut Stack, value: Self) {
                stack.bytes.extend(value.to_le_bytes());
                stack.entries.push(StackEntry::Value($val_type));
            }

            fn top(stack: &Stack) -> Self {
                assert_eq!(stack.entries.last(), Some(&StackEntry::Value($val_type)));
                <$type>::from_le_bytes(stack.top_bytes())
            }
        }
    };
}

impl_stack_value!(i32, ValType::I32);
impl_stack_value!(i64, ValType::I64);
impl_stack_value!(f32, ValType::F32);
impl_stack_value!(f64, ValType::F64);

impl StackValue for Label {
    fn top(stack: &Stack) -> Self {
        assert_eq!(stack.entries.last(), Some(&StackEntry::Label));
        Self {
            next: stack.to_usize(16, 8),
            n: stack.to_usize(8, 0),
        }
    }

    fn push(stack: &mut Stack, value: Self) {
        stack.bytes.extend(value.next.to_le_bytes());
        stack.bytes.extend(value.n.to_le_bytes());
        stack.entries.push(StackEntry::Label);
    }
}

impl StackValue for Frame {
    fn top(stack: &Stack) -> Self {
        assert_eq!(stack.entries.last(), Some(&StackEntry::Frame));
        let n = stack.to_usize(8, 0);
        unsafe {
            let local = Vec::from_raw_parts(
                stack.to_usize(32, 24) as *mut _,
                stack.to_usize(24, 16),
                stack.to_usize(16, 8),
            );
            let instance = Rc::from_raw(stack.to_usize(40, 32) as *const _);
            Self { instance, local, n }
        }
    }

    fn push(stack: &mut Stack, value: Self) {
        // -40 .. -32
        stack
            .bytes
            .extend((Rc::into_raw(value.instance) as usize).to_le_bytes());
        // -32 .. -24
        stack
            .bytes
            .extend((value.local.as_ptr() as usize).to_le_bytes());
        // -24 .. -16
        stack.bytes.extend(value.local.len().to_le_bytes());
        // -16 .. -8
        stack.bytes.extend(value.local.capacity().to_le_bytes());
        // -8 .. 0
        stack.bytes.extend(value.n.to_le_bytes());

        core::mem::forget(value.local);

        stack.entries.push(StackEntry::Frame);
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
        stack.push(-3 as i32);
        stack.push(i32::MAX as i64 + 10);
        stack.push(1.32 as f32);
        stack.push(1.64 as f64);

        assert_eq!(stack.pop::<f64>(), 1.64);
        assert_eq!(stack.pop::<f32>(), 1.32);
        assert_eq!(stack.pop::<i64>(), i32::MAX as i64 + 10);
        assert_eq!(stack.pop::<i32>(), -3);

        assert!(stack.is_empty())
    }

    #[test]
    #[should_panic]
    fn stack_value_err() {
        let mut stack = Stack::new();
        stack.push(3 as i32);
        stack.push(3 as i32);

        assert_eq!(stack.pop::<i64>(), 9);
    }

    #[test]
    fn stack_label() {
        let label1 = Label { next: 4, n: 0 };
        let label2 = Label { next: 46, n: 1 };
        let mut stack = Stack::new();
        stack.push(label1);
        stack.push(label2);
        assert_eq!(stack.pop::<Label>(), Label { next: 46, n: 1 });
        assert_eq!(stack.pop::<Label>(), Label { next: 4, n: 0 });

        assert!(stack.is_empty());
    }

    #[test]
    fn stack_frame() {
        //let instance = Rc::new(Instance::default());
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
        stack.push(frame1);
        stack.push(frame2);

        assert_eq!(
            stack.pop::<Frame>(),
            Frame {
                instance: Rc::new(Instance::default()),
                local: vec![Value::I32(1), Value::F32(3.0)],
                n: 0,
            }
        );
        assert_eq!(
            stack.pop::<Frame>(),
            Frame {
                instance: Rc::new(Instance::default()),
                local: vec![],
                n: 1,
            }
        );
        assert!(stack.is_empty());
    }
}
