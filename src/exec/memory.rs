use super::{
    runtime::{Addr, Instance},
    stack::Stack,
    store::{DataInst, MemInst, Store},
    trap::Trap,
};
#[cfg(not(feature = "std"))]
use crate::lib::*;
use crate::{
    binary::{Data, MemArg},
    exec::{runtime::PAGE_SIZE, value::LittleEndian},
};
use opt_vec::OptVec;

macro_rules! impl_load {
    ($fnname: ident, $t:ty, $sx:ty) => {
        pub fn $fnname(
            memarg: &MemArg,
            instance: &mut Instance,
            store: &mut Store,
            stack: &mut Stack,
        ) -> Result<(), Trap> {
            let a = instance.memaddr.unwrap();
            let mem = &store.mems[a];
            let i = stack.pop_value::<i32>() as usize;
            let ea = i
                .checked_add(memarg.offset as usize)
                .ok_or(Trap::MemoryOutOfBounds)?;
            const SIZE: usize = core::mem::size_of::<$sx>();
            if ea.checked_add(SIZE).ok_or(Trap::MemoryOutOfBounds)? > mem.data.len() {
                return Err(Trap::MemoryOutOfBounds);
            }
            let c: $sx = LittleEndian::read(&mem.data, ea);
            stack.push_value(c as $t);
            Ok(())
        }
    };
}

impl_load!(i32_load, i32, i32);
impl_load!(i64_load, i64, i64);
impl_load!(f32_load, f32, f32);
impl_load!(f64_load, f64, f64);
impl_load!(i32_load_8s, i32, i8);
impl_load!(i32_load_8u, i32, u8);
impl_load!(i32_load_16s, i32, i16);
impl_load!(i32_load_16u, i32, u16);
impl_load!(i64_load_8s, i64, i8);
impl_load!(i64_load_8u, i64, u8);
impl_load!(i64_load_16s, i64, i16);
impl_load!(i64_load_16u, i64, u16);
impl_load!(i64_load_32s, i64, i32);
impl_load!(i64_load_32u, i64, u32);

macro_rules! impl_store {
    ($fnname: ident, $t:ty, $sx:ty) => {
        pub fn $fnname(
            memarg: &MemArg,
            instance: &mut Instance,
            store: &mut Store,
            stack: &mut Stack,
        ) -> Result<(), Trap> {
            let a = instance.memaddr.unwrap();
            let mem = &mut store.mems[a];
            let c = stack.pop_value::<$t>();
            let i = stack.pop_value::<i32>() as usize;
            let ea = i
                .checked_add(memarg.offset as usize)
                .ok_or(Trap::MemoryOutOfBounds)?;
            const SIZE: usize = core::mem::size_of::<$sx>();
            if ea.checked_add(SIZE).ok_or(Trap::MemoryOutOfBounds)? > mem.data.len() {
                return Err(Trap::MemoryOutOfBounds);
            }
            LittleEndian::write(&mut mem.data, ea, c as $sx);
            Ok(())
        }
    };
}

impl_store!(i32_store, i32, i32);
impl_store!(i64_store, i64, i64);
impl_store!(f32_store, f32, f32);
impl_store!(f64_store, f64, f64);
impl_store!(i32_store_8, i32, u8);
impl_store!(i32_store_16, i32, u16);
impl_store!(i64_store_8, i64, u8);
impl_store!(i64_store_16, i64, u16);
impl_store!(i64_store_32, i64, u32);

pub fn memory_size(instance: &Instance, store: &Store, stack: &mut Stack) {
    let a = instance.memaddr.unwrap();
    let mem = &store.mems[a];
    stack.push_value(mem.limits.min() as i32);
}

pub fn memory_grow(instance: &Instance, store: &mut Store, stack: &mut Stack) {
    let a = instance.memaddr.unwrap();
    const ERR: i32 = -1;
    let mem = &mut store.mems[a];
    let sz = mem.limits.min();
    let n = stack.pop_value::<i32>() as u32;
    let len = sz + n;
    if len > u16::MAX as u32 + 1 {
        stack.push_value(ERR);
        return;
    }
    let limits_ = mem.limits.set_min(len);
    if !limits_.valid() {
        stack.push_value(ERR);
        return;
    }
    for _ in 0..(n * PAGE_SIZE as u32) {
        mem.data.push(0);
    }
    mem.limits = limits_;
    stack.push_value(sz as i32);
}

pub fn memory_fill(instance: &Instance, store: &mut Store, stack: &mut Stack) -> Result<(), Trap> {
    let ma = instance.memaddr.unwrap();
    let mem = &mut store.mems[ma];
    let n = stack.pop_value::<i32>() as usize;
    let val = stack.pop_value::<i32>();
    let d = stack.pop_value::<i32>() as usize;
    if d + n > mem.data.len() {
        return Err(Trap::MemoryOutOfBounds);
    }
    if n == 0 {
        return Ok(());
    }
    for i in 0..n {
        LittleEndian::write(&mut mem.data, d + i, val);
    }
    Ok(())
}

pub fn memory_copy(instance: &Instance, store: &mut Store, stack: &mut Stack) -> Result<(), Trap> {
    let ma = instance.memaddr.unwrap();
    let mem = &mut store.mems[ma];
    let n = stack.pop_value::<i32>() as usize;
    let s = stack.pop_value::<i32>() as usize;
    let d = stack.pop_value::<i32>() as usize;

    if s + n > mem.data.len() || d + n > mem.data.len() {
        return Err(Trap::MemoryOutOfBounds);
    }
    if n == 0 {
        return Ok(());
    }
    if d <= s {
        for i in 0..n {
            mem.data[d + i] = mem.data[s + i];
        }
    } else {
        for i in (0..n).rev() {
            mem.data[d + n - 1 - i] = mem.data[s + n - 1 - i];
        }
    }
    Ok(())
}

pub fn memory_init(
    x: &u32,
    instance: &Instance,
    store: &mut Store,
    stack: &mut Stack,
) -> Result<(), Trap> {
    let ma = instance.memaddr.unwrap();
    let mem = &mut store.mems[ma];
    let da = instance.dataaddrs[*x as usize];
    let data = &store.datas[da];
    let n = stack.pop_value::<i32>() as usize;
    let s = stack.pop_value::<i32>() as usize;
    let d = stack.pop_value::<i32>() as usize;
    if s + n > data.data.len() || d + n > mem.data.len() {
        return Err(Trap::MemoryOutOfBounds);
    }
    if n == 0 {
        return Ok(());
    }
    for i in 0..n {
        mem.data[d + i] = data.data[s + i];
    }
    Ok(())
}

pub fn data_drop(x: &u32, instance: &mut Instance, store: &mut Store) {
    let a = instance.dataaddrs[*x as usize];
    store.datas.remove(a);
}

pub fn data_passiv(datas: &mut OptVec<DataInst>, data: Data) -> Addr {
    datas.push(DataInst { data: data.init })
}

pub fn data_active(mem: &mut MemInst, data: Data, offset: usize) {
    for i in 0..data.init.len() {
        mem.data[offset + i] = data.init[i];
    }
}
