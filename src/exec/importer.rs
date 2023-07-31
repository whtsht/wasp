use crate::binary::Module;
#[cfg(feature = "std")]
use alloc::collections::BTreeMap;

pub trait Importer {
    fn import(&mut self, modname: &str) -> Option<Module>;
}

#[cfg(feature = "std")]
#[derive(Debug)]
pub struct DefaultImporter {
    modules: BTreeMap<String, Module>,
}

#[cfg(feature = "std")]
impl DefaultImporter {
    pub fn new() -> Self {
        Self {
            modules: BTreeMap::new(),
        }
    }

    pub fn add_module(&mut self, module: Module, modname: &str) {
        self.modules.insert(modname.into(), module);
    }
}

#[cfg(feature = "std")]
impl Importer for DefaultImporter {
    fn import(&mut self, modname: &str) -> Option<Module> {
        if let Some(module) = self.modules.get(modname) {
            return Some(module.clone());
        }

        use crate::loader::parser::Parser;
        use std::fs::File;
        use std::io::prelude::*;

        let mut file = File::open(modname).ok()?;
        let mut buf = vec![];
        file.read_to_end(&mut buf).ok()?;
        let mut parser = Parser::new(&buf);

        let module = parser.module().ok()?;
        self.modules.insert(modname.into(), module.clone());

        Some(module)
    }
}
