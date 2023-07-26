#[cfg(not(feature = "std"))]
use crate::lib::*;

use crate::binary::ValType;
use core::fmt;
use core::mem::size_of;

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

#[cfg(test)]
mod tests {
    use super::Stack;

    #[test]
    fn test_stack() {
        let mut stack = Stack::new();
        stack.push(-3 as i32);
        stack.push(i32::MAX as i64 + 10);
        stack.push(1.32 as f32);
        stack.push(1.64 as f64);

        assert_eq!(stack.pop::<f64>(), 1.64);
        assert_eq!(stack.pop::<f32>(), 1.32);
        assert_eq!(stack.pop::<i64>(), i32::MAX as i64 + 10);
        assert_eq!(stack.pop::<i32>(), -3);
    }

    #[test]
    #[should_panic]
    fn test_stack_err() {
        let mut stack = Stack::new();
        stack.push(3 as i32);
        stack.push(3 as i32);

        assert_eq!(stack.pop::<i64>(), 9);
    }
}
