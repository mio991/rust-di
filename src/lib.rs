use std::{
    any::{type_name, Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    error::Error,
    fmt::Debug,
    rc::Rc,
};

use elsa::FrozenMap;

#[derive(Debug, thiserror::Error)]
pub enum ResolveErrorKind {
    #[error("Service not found!")]
    NotFound,
    #[error("Circular reference while resolving!")]
    CircularReferenceFound,
    #[error("Error while resolving service!")]
    ErrorWhileResolving(
        #[from]
        #[source]
        Box<dyn Error>,
    ),
}

#[derive(Debug, thiserror::Error)]
#[error("Could not resolve {type_name} because '{kind}'!")]
pub struct ResolveError {
    type_name: &'static str,
    kind: ResolveErrorKind,
}

impl ResolveError {
    fn for_type<T: 'static>() -> fn(ResolveErrorKind) -> ResolveError {
        |kind| ResolveError {
            type_name: type_name::<T>(),
            kind,
        }
    }
}

pub unsafe trait Factory {
    fn type_name(&self) -> &'static str;
    fn type_id(&self) -> TypeId;
    fn resolve(
        self: Box<Self>,
        service_provider: &ServiceProvider,
    ) -> Result<Rc<dyn Any>, Box<dyn Error>>;
}

unsafe impl<T: Any + 'static, F> Factory for F
where
    F: FnOnce(&ServiceProvider) -> Result<T, Box<dyn Error>>,
{
    fn type_name(&self) -> &'static str {
        type_name::<T>()
    }
    fn type_id(&self) -> TypeId {
        // Safety: the generics enforce that resolve returns a Box<T>
        TypeId::of::<T>()
    }

    fn resolve(
        self: Box<Self>,
        service_provider: &ServiceProvider,
    ) -> Result<Rc<dyn Any>, Box<dyn Error>> {
        let service = self(service_provider)?;

        Ok(Rc::new(service))
    }
}

impl Debug for dyn Factory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Factory for {0}", self.type_name()))
    }
}

pub struct ServiceProvider {
    factories: RefCell<HashMap<TypeId, Box<dyn Factory>>>,
    instances: FrozenMap<TypeId, Box<Rc<dyn Any>>>,
}

impl Debug for ServiceProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceProvider")
            .field("factories", &self.factories)
            .finish()
    }
}

impl ServiceProvider {
    pub fn new<I: IntoIterator<Item = Box<dyn Factory>>>(factories: I) -> Self {
        Self {
            factories: RefCell::new(
                factories
                    .into_iter()
                    .map(|f| {
                        eprintln!("Factory: {0} - {1:?}", f.type_name(), f.type_id());
                        (f.type_id(), f)
                    })
                    .collect(),
            ),
            instances: FrozenMap::new(),
        }
    }

    pub fn resolve<T: 'static>(&self) -> Result<Rc<T>, ResolveError> {
        eprintln!("Resolve {0} in {1:?}", type_name::<T>(), self);

        let type_id = TypeId::of::<T>();

        if let Some(any) = self.instances.get(&type_id).cloned() {
            Ok(any
                .downcast()
                .expect("we resolved by TypeId so it should be a T"))
        } else {
            let factory = {
                let mut factories = self.factories.borrow_mut();
                factories
                    .remove(&type_id)
                    .ok_or(ResolveErrorKind::NotFound)
                    .map_err(ResolveError::for_type::<T>())?
            };

            let service = factory
                .resolve(self)
                .map_err(ResolveErrorKind::ErrorWhileResolving)
                .map_err(ResolveError::for_type::<T>())?;

            self.instances.insert(type_id, Box::new(service.clone()));

            Ok(service
                .downcast()
                .expect("We just resolved the factory for T"))
        }
    }
}

#[cfg(test)]
mod test {
    #[allow(unused_variables)]
    use super::*;
    use std::{error::Error, rc::Rc};

    struct Test1 {
        name: String,
    }

    impl Test1 {
        fn factory(services: &ServiceProvider) -> Result<Test1, Box<dyn Error>> {
            Ok(Test1 {
                name: String::from("Lila"),
            })
        }
    }

    struct Test2 {
        name: String,
        test1: Rc<Test1>,
    }

    impl Test2 {
        fn factory(services: &ServiceProvider) -> Result<Test2, Box<dyn Error>> {
            Ok(Test2 {
                name: String::from("Kuh"),
                test1: services.resolve()?,
            })
        }
    }

    fn service_provider() -> ServiceProvider {
        let factories: Vec<Box<dyn Factory>> =
            vec![Box::new(Test1::factory), Box::new(Test2::factory)];

        ServiceProvider::new(factories)
    }

    #[test]
    fn resolve() -> Result<(), Box<dyn Error>> {
        let services = service_provider();

        let test1 = services.resolve::<Test1>()?;

        assert_eq!(test1.name, "Lila");

        Ok(())
    }
}
