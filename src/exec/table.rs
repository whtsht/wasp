use crate::binary::Elem;
#[cfg(not(feature = "std"))]
use crate::lib::*;

use super::{
    runtime::{eval_const, Addr, Instance, RuntimeError},
    stack::Stack,
    store::{ElemInst, Store, TableInst},
    trap::Trap,
    value::{Ref, Value},
};

pub fn table_get(
    x: &u32,
    instance: &mut Instance,
    store: &mut Store,
    stack: &mut Stack,
) -> Result<(), Trap> {
    let a = instance.tableaddrs[*x as usize];
    let tab = &mut store.tables[a];
    let i = stack.pop_value::<i32>() as usize;
    if i < tab.elem.len() {
        return Err(Trap::TableOutOfRange);
    }
    stack.push_value(Value::Ref(tab.elem[i]));
    Ok(())
}

pub fn table_set(
    x: &u32,
    instance: &mut Instance,
    store: &mut Store,
    stack: &mut Stack,
) -> Result<(), Trap> {
    let a = instance.tableaddrs[*x as usize];
    let tab = &mut store.tables[a];
    let val = stack.pop_value::<Ref>();
    let i = stack.pop_value::<i32>() as usize;
    if i < tab.elem.len() {
        return Err(Trap::TableOutOfRange);
    }
    tab.elem[i] = val;
    Ok(())
}

pub fn table_grow(x: &u32, instance: &mut Instance, store: &mut Store, stack: &mut Stack) {
    let a = instance.tableaddrs[*x as usize];
    let tab = &mut store.tables[a];
    let sz = tab.elem.len() as i32;
    const ERR: i32 = -1;
    let n = stack.pop_value::<i32>();
    let init = stack.pop_value::<Ref>();
    let len = n as u64 + tab.elem.len() as u64;
    if len > u32::MAX as u64 {
        stack.push_value(ERR);
        return;
    }
    let limits_ = tab.tabletype.limits.set_min(len as u32);
    if !limits_.valid() {
        stack.push_value(ERR);
        return;
    }
    for _ in tab.tabletype.limits.min()..limits_.min() {
        tab.elem.push(init);
    }
    tab.tabletype.limits = limits_;
    stack.push_value(sz);
}

pub fn table_fill(
    x: &u32,
    instance: &mut Instance,
    store: &mut Store,
    stack: &mut Stack,
) -> Result<(), Trap> {
    let ta = instance.tableaddrs[*x as usize];
    let tab = &mut store.tables[ta];
    let n = stack.pop_value::<i32>();
    let val = stack.pop_value::<Ref>();
    let i = stack.pop_value::<i32>();
    if i + n > tab.elem.len() as i32 {
        return Err(Trap::TableOutOfRange);
    }
    for j in i..(i + n) {
        tab.elem[j as usize] = val;
    }
    Ok(())
}

pub fn table_copy(
    x: &u32,
    y: &u32,
    instance: &mut Instance,
    store: &mut Store,
    stack: &mut Stack,
) -> Result<(), Trap> {
    let ta_x = instance.tableaddrs[*x as usize];
    let tab_x = &store.tables[ta_x];
    let ta_y = instance.tableaddrs[*y as usize];
    let tab_y = &store.tables[ta_y];
    let n = stack.pop_value::<i32>() as usize;
    let s = stack.pop_value::<i32>() as usize;
    let d = stack.pop_value::<i32>() as usize;
    if s + n > tab_y.elem.len() || d + n > tab_x.elem.len() {
        return Err(Trap::TableOutOfRange);
    }

    if d <= s {
        for i in 0..n {
            store.tables[ta_x].elem[d + i] = store.tables[ta_y].elem[s + i];
        }
    } else {
        for i in (0..n).rev() {
            store.tables[ta_x].elem[d + n - 1 - i] = store.tables[ta_y].elem[s + n - 1 - i];
        }
    }

    Ok(())
}

pub fn table_init(
    x: &u32,
    y: &u32,
    instance: &mut Instance,
    store: &mut Store,
    stack: &mut Stack,
) -> Result<(), Trap> {
    let ta = instance.tableaddrs[*x as usize];
    let tab = &mut store.tables[ta];
    let ea = instance.elemaddrs[*y as usize];
    let elem = &store.elems[ea];
    let n = stack.pop_value::<i32>() as usize;
    let s = stack.pop_value::<i32>() as usize;
    let d = stack.pop_value::<i32>() as usize;
    if s + n > elem.elem.len() || d + n > elem.elem.len() {
        return Err(Trap::TableOutOfRange);
    }

    for i in 0..n {
        tab.elem[d + i] = elem.elem[d + s];
    }
    Ok(())
}

pub fn table_init_manual(tab: &mut TableInst, offset: usize, elems: &Vec<Ref>) {
    for (i, elem) in elems.iter().enumerate() {
        tab.elem[i + offset] = *elem;
    }
}

pub fn elem_drop(x: &u32, instance: &mut Instance, store: &mut Store) {
    let a = instance.elemaddrs[*x as usize];
    // TODO
    // drop store.elems[a]
    let _ = &store.elems[a];
}

pub fn table_size(x: &u32, instance: &mut Instance, store: &mut Store, stack: &mut Stack) {
    let a = instance.tableaddrs[*x as usize];
    let tab = &store.tables[a];
    let sz = tab.elem.len() as i32;
    stack.push_value(sz);
}

pub fn elem_passiv(elems: &mut Vec<ElemInst>, elem: Elem) -> Result<(), RuntimeError> {
    let vals = elem
        .init
        .iter()
        .map(|expr| eval_const(expr))
        .collect::<Result<Vec<_>, _>>()?;
    let refs = vals
        .into_iter()
        .map(|value| match value {
            Value::I32(addr) => Ref::Func(addr as Addr),
            Value::I64(addr) => Ref::Func(addr as Addr),
            Value::F32(addr) => Ref::Func(addr as Addr),
            Value::F64(addr) => Ref::Func(addr as Addr),
            Value::Ref(r) => r,
        })
        .collect();
    elems.push(ElemInst {
        reftype: elem.type_.clone(),
        elem: refs,
    });
    Ok(())
}

pub fn elem_active(table: &mut TableInst, offset: usize, elem: Elem) -> Result<(), RuntimeError> {
    let vals = elem
        .init
        .iter()
        .map(|expr| eval_const(expr))
        .collect::<Result<Vec<_>, _>>()?;
    let refs = vals
        .into_iter()
        .map(|value| match value {
            Value::I32(addr) => Ref::Func(addr as Addr),
            Value::I64(addr) => Ref::Func(addr as Addr),
            Value::F32(addr) => Ref::Func(addr as Addr),
            Value::F64(addr) => Ref::Func(addr as Addr),
            Value::Ref(r) => r,
        })
        .collect();
    table_init_manual(table, offset, &refs);
    Ok(())
}
