use std::{
    any::{type_name, Any, TypeId},
    collections::HashMap,
    fmt::Debug,
    marker::PhantomData,
    mem::replace,
};

#[derive(Debug)]
pub enum ResolveError {
    NotFound,
    CircularReferenceFound,
    TypeMissmatch(&'static str),
}

impl From<std::cell::BorrowMutError> for ResolveError {
    fn from(_value: std::cell::BorrowMutError) -> Self {
        Self::CircularReferenceFound
    }
}

pub trait ServiceProvider {
    fn resolve_by_id(&mut self, type_id: TypeId) -> Result<&Variant, ResolveError>;
}

impl dyn ServiceProvider {
    pub fn resolve<T: 'static>(&mut self) -> Result<&T, ResolveError> {
        let type_id = TypeId::of::<T>();
        let variant = self.resolve_by_id(type_id)?;

        variant
            .value
            .downcast_ref()
            .ok_or(ResolveError::TypeMissmatch(variant.type_name))
    }
}

pub struct Variant {
    pub type_name: &'static str,
    pub value: Box<dyn Any>,
}

enum MapEntry {
    Unresolved(Box<dyn Fn(&dyn ServiceProvider) -> Variant>),
    Resolving,
    Resolved(Variant),
}

impl Debug for MapEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MapEntry::Unresolved(_) => "Unresolved",
            MapEntry::Resolving => "Resolving",
            MapEntry::Resolved(_) => "Resolved",
        })
    }
}

#[derive(Debug)]
pub struct ServiceCollection {
    map: HashMap<TypeId, MapEntry>,
}

fn make_box_factory<T, F>(factory: F) -> Box<dyn Fn(&dyn ServiceProvider) -> Variant>
where
    T: 'static,
    F: Fn(&dyn ServiceProvider) -> T + 'static,
{
    Box::new(move |services| Variant {
        type_name: type_name::<T>(),
        value: Box::new(factory(services)),
    })
}

impl ServiceCollection {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn add<T>(&mut self) -> ProviderBuilder<'_, T>
    where
        T: 'static,
    {
        ProviderBuilder {
            map: &mut self.map,
            phantom: PhantomData::default(),
        }
    }

    pub fn build(self) -> Box<dyn ServiceProvider> {
        Box::new(self.map)
    }
}

pub struct ProviderBuilder<'a, T: 'static> {
    map: &'a mut HashMap<TypeId, MapEntry>,
    phantom: PhantomData<T>,
}

impl<'a, T: 'static> ProviderBuilder<'a, T> {
    pub fn with_factory<F>(self, factory: F)
    where
        F: Fn(&dyn ServiceProvider) -> T + 'static,
    {
        self.map.insert(
            TypeId::of::<T>(),
            MapEntry::Unresolved(make_box_factory(factory)),
        );
    }

    pub fn with_instance(self, service: T) {
        self.map.insert(
            TypeId::of::<T>(),
            MapEntry::Resolved(Variant {
                type_name: type_name::<T>(),
                value: Box::new(service),
            }),
        );
    }
}

impl ServiceProvider for HashMap<TypeId, MapEntry> {
    fn resolve_by_id(&mut self, type_id: TypeId) -> Result<&Variant, ResolveError> {
        let current = {
            let entry = self.get_mut(&type_id).ok_or(ResolveError::NotFound)?;

            replace(entry, MapEntry::Resolving)
        };

        let service = match current {
            MapEntry::Unresolved(factory) => Ok(factory(self)),
            MapEntry::Resolving => Err(ResolveError::CircularReferenceFound),
            MapEntry::Resolved(service) => Ok(service),
        }?;

        *self.get_mut(&type_id).expect("just accessed it!") = MapEntry::Resolved(service);

        match self.get(&type_id).expect("just accessed it!") {
            MapEntry::Resolved(service) => Ok(service),
            _ => panic!("Expected self to be resolved!"),
        }
    }
}
