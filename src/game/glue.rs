//! Just a bunch of boilerplate
use std::{borrow::Cow, marker::PhantomData};

use serde::{
    de::{EnumAccess, VariantAccess},
    Deserialize, Deserializer,
};

use crate::schema;

use super::Action;

/// A trait that has to be implemented by action enums. It can be automatically implemented with
/// `#[derive(neuro_sama::derive::Actions)]`.
pub trait Actions<'de>: Sized {
    fn deserialize<D: Deserializer<'de>>(discriminant: &str, de: D) -> Result<Self, D::Error>;
}

impl<'de, T: 'de + Deserialize<'de>> Actions<'de> for T {
    fn deserialize<D: Deserializer<'de>>(discriminant: &str, de: D) -> Result<Self, D::Error> {
        struct DeserWrapper<'a, 'de, D: Deserializer<'de>>(D, &'a str, PhantomData<&'de ()>);
        macro_rules! impl_visitor {
            ($($a:tt),*) => {
                $(fn $a<V>(self, visitor: V) -> Result<V::Value, Self::Error>
                    where
                        V: serde::de::Visitor<'de> {
                            self.deserialize_any(visitor)
                })*
            };
        }
        impl<'de, D: Deserializer<'de>> VariantAccess<'de> for DeserWrapper<'_, 'de, D> {
            type Error = D::Error;
            fn struct_variant<V>(
                self,
                fields: &'static [&'static str],
                visitor: V,
            ) -> Result<V::Value, Self::Error>
            where
                V: serde::de::Visitor<'de>,
            {
                self.0.deserialize_struct("", fields, visitor)
            }
            fn unit_variant(self) -> Result<(), Self::Error> {
                Deserialize::deserialize(self.0)
            }
            fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: serde::de::Visitor<'de>,
            {
                self.0.deserialize_tuple(len, visitor)
            }
            fn newtype_variant<T>(self) -> Result<T, Self::Error>
            where
                T: Deserialize<'de>,
            {
                Deserialize::deserialize(self.0)
            }
            fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
            where
                T: serde::de::DeserializeSeed<'de>,
            {
                seed.deserialize(self.0)
            }
        }
        impl<'de, D: Deserializer<'de>> EnumAccess<'de> for DeserWrapper<'_, 'de, D> {
            type Error = D::Error;
            type Variant = Self;
            fn variant<V>(self) -> Result<(V, Self::Variant), Self::Error>
            where
                V: Deserialize<'de>,
            {
                Ok((
                    V::deserialize(serde::de::value::StrDeserializer::new(self.1))?,
                    self,
                ))
            }
            fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
            where
                V: serde::de::DeserializeSeed<'de>,
            {
                Ok((
                    seed.deserialize(serde::de::value::StrDeserializer::new(self.1))?,
                    self,
                ))
            }
        }
        impl<'de, D: Deserializer<'de>> Deserializer<'de> for DeserWrapper<'_, 'de, D> {
            type Error = D::Error;
            impl_visitor!(
                deserialize_i8,
                deserialize_i16,
                deserialize_i32,
                deserialize_i64,
                deserialize_u8,
                deserialize_u16,
                deserialize_u32,
                deserialize_u64,
                deserialize_bool,
                deserialize_f32,
                deserialize_f64,
                deserialize_char,
                deserialize_str,
                deserialize_string,
                deserialize_bytes,
                deserialize_byte_buf,
                deserialize_option,
                deserialize_unit,
                deserialize_seq,
                deserialize_map,
                deserialize_identifier,
                deserialize_ignored_any
            );
            fn deserialize_enum<V>(
                self,
                _name: &'static str,
                _variants: &'static [&'static str],
                visitor: V,
            ) -> Result<V::Value, Self::Error>
            where
                V: serde::de::Visitor<'de>,
            {
                self.deserialize_any(visitor)
            }
            fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: serde::de::Visitor<'de>,
            {
                self.deserialize_any(visitor)
            }
            fn deserialize_struct<V>(
                self,
                _name: &'static str,
                _fields: &'static [&'static str],
                visitor: V,
            ) -> Result<V::Value, Self::Error>
            where
                V: serde::de::Visitor<'de>,
            {
                self.deserialize_any(visitor)
            }
            fn deserialize_unit_struct<V>(
                self,
                _name: &'static str,
                visitor: V,
            ) -> Result<V::Value, Self::Error>
            where
                V: serde::de::Visitor<'de>,
            {
                self.deserialize_any(visitor)
            }
            fn deserialize_tuple_struct<V>(
                self,
                _name: &'static str,
                _len: usize,
                visitor: V,
            ) -> Result<V::Value, Self::Error>
            where
                V: serde::de::Visitor<'de>,
            {
                self.deserialize_any(visitor)
            }
            fn deserialize_newtype_struct<V>(
                self,
                _name: &'static str,
                visitor: V,
            ) -> Result<V::Value, Self::Error>
            where
                V: serde::de::Visitor<'de>,
            {
                self.deserialize_any(visitor)
            }
            fn is_human_readable(&self) -> bool {
                self.0.is_human_readable()
            }
            fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: serde::de::Visitor<'de>,
            {
                visitor.visit_enum(self)
            }
        }
        let wrapper = DeserWrapper(de, discriminant, PhantomData);
        <Self as Deserialize<'_>>::deserialize(wrapper)
    }
}

/// A trait that has to be implemented by sets of type-level action metadata. A set of action
/// metadata is something that you can register or unregister to tell Neuro which actions are or
/// aren't available. To put it simpy, this trait is implemented by actions, action enums, and
/// tuples of actions, and by passing any type that implements this trait as the type parameter to
/// `register_action` or `unregister_action`, you can register/unregister actions in a type-safe
/// way.
pub trait ActionMetadata {
    fn actions() -> Vec<schema::Action>;
    fn names() -> Vec<Cow<'static, str>>;
}

impl<T: Action> ActionMetadata for T {
    fn actions() -> Vec<schema::Action> {
        vec![schema::Action {
            name: Self::name().into(),
            description: Self::description().into(),
            schema: schemars::schema_for!(Self),
        }]
    }
    fn names() -> Vec<Cow<'static, str>> {
        vec![Self::name().into()]
    }
}
macro_rules! tuple_actions {
    ($($a:tt),*) => {
        impl<$($a: Action),*> ActionMetadata for ($($a,)*) {
            fn actions() -> Vec<schema::Action> {
                vec![$(schema::Action {
                    name: $a::name().into(),
                    description: $a::description().into(),
                    schema: schemars::schema_for!($a),
                }),*]
            }
            fn names() -> Vec<Cow<'static, str>> {
                vec![$($a::name().into()),*]
            }
        }
    };
}
tuple_actions!();
tuple_actions!(A);
tuple_actions!(A, B);
tuple_actions!(A, B, C);
tuple_actions!(A, B, C, D);
tuple_actions!(A, B, C, D, E);
tuple_actions!(A, B, C, D, E, F);
tuple_actions!(A, B, C, D, E, F, G);
tuple_actions!(A, B, C, D, E, F, G, H);
tuple_actions!(A, B, C, D, E, F, G, H, I);
tuple_actions!(A, B, C, D, E, F, G, H, I, J);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z);
tuple_actions!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, A0);
