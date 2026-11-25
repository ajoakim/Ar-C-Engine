//! Dependency-ordered module registry (Kahn topological sort).

use std::collections::HashMap;

use crate::error::RegistryError;
use crate::module::Module;

/// Holds modules and orders them by declared dependencies.
#[derive(Default)]
pub struct ModuleRegistry {
    modules: Vec<Box<dyn Module>>,
    order: Option<Vec<usize>>,
}

impl ModuleRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a module; fails on duplicate name. Invalidates the cached order.
    pub fn register(&mut self, module: Box<dyn Module>) -> Result<(), RegistryError> {
        let name = module.name();
        if self.modules.iter().any(|m| m.name() == name) {
            return Err(RegistryError::DuplicateModule(name.to_owned()));
        }
        self.modules.push(module);
        self.order = None;
        Ok(())
    }

    /// Number of registered modules.
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Whether no modules are registered.
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    fn compute_order(&self) -> Result<Vec<usize>, RegistryError> {
        let n = self.modules.len();
        let index: HashMap<&str, usize> = self
            .modules
            .iter()
            .enumerate()
            .map(|(i, m)| (m.name(), i))
            .collect();

        // deps[i] = deduplicated dependency indices of module i.
        let mut deps: Vec<Vec<usize>> = Vec::with_capacity(n);
        for m in &self.modules {
            let mut d = Vec::new();
            for dep in m.dependencies() {
                let Some(&j) = index.get(dep) else {
                    return Err(RegistryError::MissingDependency {
                        module: m.name().to_owned(),
                        dependency: (*dep).to_owned(),
                    });
                };
                if !d.contains(&j) {
                    d.push(j);
                }
            }
            deps.push(d);
        }

        let mut in_degree: Vec<usize> = deps.iter().map(Vec::len).collect();
        let mut emitted = vec![false; n];
        let mut order = Vec::with_capacity(n);
        while let Some(i) = (0..n).find(|&i| !emitted[i] && in_degree[i] == 0) {
            emitted[i] = true;
            order.push(i);
            for (k, d) in deps.iter().enumerate() {
                if d.contains(&i) {
                    in_degree[k] -= 1;
                }
            }
        }

        if order.len() < n {
            // Reports every blocked module, including ones that merely depend on a cycle.
            let names = (0..n)
                .filter(|&i| !emitted[i])
                .map(|i| self.modules[i].name().to_owned())
                .collect();
            return Err(RegistryError::DependencyCycle(names));
        }
        Ok(order)
    }

    fn ensure_order(&mut self) -> Result<Vec<usize>, RegistryError> {
        if let Some(o) = &self.order {
            return Ok(o.clone());
        }
        let o = self.compute_order()?;
        self.order = Some(o.clone());
        Ok(o)
    }

    /// Resolve and cache the init order, returning module names.
    pub fn resolve_order(&mut self) -> Result<Vec<&'static str>, RegistryError> {
        let order = self.ensure_order()?;
        Ok(order.into_iter().map(|i| self.modules[i].name()).collect())
    }

    /// Visit modules in dependency order; stops at the first error.
    pub fn for_each_in_order<E: From<RegistryError>>(
        &mut self,
        mut f: impl FnMut(&mut dyn Module) -> Result<(), E>,
    ) -> Result<(), E> {
        let order = self.ensure_order()?;
        for i in order {
            f(self.modules[i].as_mut())?;
        }
        Ok(())
    }

    /// Visit modules in reverse dependency order; stops at the first error.
    pub fn for_each_in_reverse<E: From<RegistryError>>(
        &mut self,
        mut f: impl FnMut(&mut dyn Module) -> Result<(), E>,
    ) -> Result<(), E> {
        let order = self.ensure_order()?;
        for i in order.into_iter().rev() {
            f(self.modules[i].as_mut())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestModule {
        name: &'static str,
        deps: Vec<&'static str>,
    }

    impl Module for TestModule {
        fn name(&self) -> &'static str {
            self.name
        }
        fn dependencies(&self) -> &[&'static str] {
            self.deps.as_slice()
        }
    }

    fn m(name: &'static str, deps: &[&'static str]) -> Box<dyn Module> {
        Box::new(TestModule {
            name,
            deps: deps.to_vec(),
        })
    }

    fn reg(mods: Vec<Box<dyn Module>>) -> ModuleRegistry {
        let mut r = ModuleRegistry::new();
        for x in mods {
            assert!(r.register(x).is_ok());
        }
        r
    }

    #[test]
    fn registry_orders_by_dependencies() {
        let mut r = reg(vec![
            m("render", &["assets", "core"]),
            m("assets", &["core"]),
            m("core", &[]),
        ]);
        assert_eq!(r.resolve_order(), Ok(vec!["core", "assets", "render"]));
    }

    #[test]
    fn registry_stable_tie_break() {
        let mut r = reg(vec![m("c", &[]), m("a", &[]), m("b", &[])]);
        assert_eq!(r.resolve_order(), Ok(vec!["c", "a", "b"]));
    }

    #[test]
    fn registry_duplicate() {
        let mut r = reg(vec![m("core", &[])]);
        assert_eq!(
            r.register(m("core", &[])),
            Err(RegistryError::DuplicateModule("core".into()))
        );
        assert_eq!(r.len(), 1);
        assert!(!r.is_empty());
    }

    #[test]
    fn registry_missing_dependency() {
        let mut r = reg(vec![m("x", &["ghost"])]);
        assert_eq!(
            r.resolve_order(),
            Err(RegistryError::MissingDependency {
                module: "x".into(),
                dependency: "ghost".into()
            })
        );
    }

    #[test]
    fn registry_cycle() {
        let mut r = reg(vec![
            m("a", &["b"]),
            m("b", &["c"]),
            m("c", &["a"]),
            m("d", &[]),
        ]);
        assert_eq!(
            r.resolve_order(),
            Err(RegistryError::DependencyCycle(vec![
                "a".into(),
                "b".into(),
                "c".into()
            ]))
        );
    }

    #[test]
    fn registry_self_dependency() {
        let mut r = reg(vec![m("a", &["a"])]);
        assert_eq!(
            r.resolve_order(),
            Err(RegistryError::DependencyCycle(vec!["a".into()]))
        );
    }

    #[test]
    fn registry_for_each_order() {
        let mut r = reg(vec![m("b", &["a"]), m("a", &[]), m("c", &["b"])]);
        let mut seen = Vec::new();
        let res: Result<(), RegistryError> = r.for_each_in_order(|md| {
            seen.push(md.name());
            Ok(())
        });
        assert!(res.is_ok());
        assert_eq!(seen, ["a", "b", "c"]);

        let mut seen = Vec::new();
        let res: Result<(), RegistryError> = r.for_each_in_reverse(|md| {
            seen.push(md.name());
            Ok(())
        });
        assert!(res.is_ok());
        assert_eq!(seen, ["c", "b", "a"]);

        let mut count = 0;
        let res: Result<(), RegistryError> = r.for_each_in_order(|_| {
            count += 1;
            Err(RegistryError::DuplicateModule("stop".into()))
        });
        assert_eq!(res, Err(RegistryError::DuplicateModule("stop".into())));
        assert_eq!(count, 1);
    }

    #[test]
    fn registry_register_invalidates_order() {
        let mut r = reg(vec![m("a", &[])]);
        assert_eq!(r.resolve_order(), Ok(vec!["a"]));
        assert!(r.register(m("b", &["a"])).is_ok());
        let mut seen = Vec::new();
        let res: Result<(), RegistryError> = r.for_each_in_order(|md| {
            seen.push(md.name());
            Ok(())
        });
        assert!(res.is_ok());
        assert_eq!(seen, ["a", "b"]);
    }
}
