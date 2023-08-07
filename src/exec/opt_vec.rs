use alloc::vec::Vec;
use core::convert::identity;

#[derive(Debug, PartialEq, Clone)]
pub struct OptVec<T> {
    inner: Vec<Option<T>>,
    free: Vec<usize>,
}

impl<T> OptVec<T> {
    pub fn new() -> Self {
        Self {
            inner: vec![],
            free: vec![],
        }
    }

    pub fn to_vec(self) -> Vec<T> {
        self.inner.into_iter().filter_map(|v| v).collect()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Vec::with_capacity(capacity),
            free: vec![],
        }
    }

    pub fn push(&mut self, value: T) -> usize {
        if let Some(i) = self.free.pop() {
            self.inner[i] = Some(value);
            i
        } else {
            self.inner.push(Some(value));
            self.inner.len() - 1
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        self.inner.pop().and_then(identity)
    }

    pub fn remove(&mut self, i: usize) -> Option<T> {
        if self.inner[i].is_some() {
            self.free.push(i);
            self.inner[i].take()
        } else {
            None
        }
    }
}

use core::ops::{Index, IndexMut};
impl<T> Index<usize> for OptVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.inner[index].as_ref().unwrap()
    }
}
impl<T> IndexMut<usize> for OptVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.inner[index].as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::OptVec;

    #[test]
    fn ok() {
        let mut v = OptVec::new();
        assert_eq!(v.push(1), 0);
        assert_eq!(v.push(2), 1);
        assert_eq!(v.push(3), 2);
        assert_eq!(v.push(4), 3);
        assert_eq!(v.push(5), 4);

        assert_eq!(v.pop(), Some(5));
        assert_eq!(v.inner, vec![Some(1), Some(2), Some(3), Some(4)]);
        assert_eq!(v.free, vec![]);

        assert_eq!(v.remove(1), Some(2));
        assert_eq!(v.inner, vec![Some(1), None, Some(3), Some(4)]);
        assert_eq!(v.free, vec![1]);

        assert_eq!(v.remove(1), None);
        assert_eq!(v.inner, vec![Some(1), None, Some(3), Some(4)]);
        assert_eq!(v.free, vec![1]);

        assert_eq!(v.push(5), 1);
        assert_eq!(v.inner, vec![Some(1), Some(5), Some(3), Some(4)]);
        assert_eq!(v.free, vec![]);
    }

    #[should_panic]
    #[test]
    fn err() {
        let mut v = OptVec::new();
        v.push(1);
        v.remove(1);
    }
}
