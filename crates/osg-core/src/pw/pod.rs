// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix

use pipewire::spa::{
    param::ParamType,
    pod::{Pod, Property, Value, ValueArray, object},
    sys::{
        SPA_PARAM_ROUTE_device, SPA_PARAM_ROUTE_index, SPA_PARAM_ROUTE_info, SPA_PARAM_ROUTE_props,
        SPA_PARAM_ROUTE_save, SPA_PROP_channelVolumes, SPA_PROP_mute,
    },
    utils::SpaTypes,
};

pub mod parse {
    use std::io::Cursor;

    use pipewire::spa::pod::{
        Object, Pod, Value, ValueArray, deserialize::PodDeserializer, serialize::PodSerializer,
    };

    pub trait PodExt {
        fn deserialize_value(&self) -> Option<Value>;
    }
    impl PodExt for Pod {
        fn deserialize_value(&self) -> Option<Value> {
            PodDeserializer::deserialize_any_from(self.as_bytes())
                .map(|(_, value)| value)
                .ok()
        }
    }

    /// A [`Pod`] is like a [`str`]: it represents a sequence of bytes that are known to be in a
    /// certain format, but may only exist behind a reference. Therefore, a function cannot return
    /// a [`Pod`] directly. Instead, this type owns those bytes until the [`Pod`] is needed,
    /// similar to what a [`String`] does for a [`&str`].
    #[allow(missing_debug_implementations)] // Opaque byte buffer, Debug would just show hex
    pub struct PodBytes(Box<[u8]>);

    impl PodBytes {
        pub fn from_value(value: &Value) -> Self {
            let mut bytes = Vec::new();
            // Serializing to a Vec<u8> is infallible in practice
            #[allow(clippy::expect_used)]
            PodSerializer::serialize(Cursor::new(&mut bytes), value)
                .expect("Vec<u8> serialization is infallible");
            Self(bytes.into_boxed_slice())
        }

        pub fn pod(&self) -> &Pod {
            // Internal bytes are guaranteed to be well-formed from PodSerializer
            #[allow(clippy::expect_used)]
            Pod::from_bytes(&self.0).expect("internal bytes are known well-formed")
        }
    }

    pub trait PodValueExt {
        fn parse_int(&self) -> Option<i32>;
        fn parse_string(&self) -> Option<&str>;
        fn parse_value_array(&self) -> Option<&ValueArray>;
        fn parse_struct(&self) -> Option<&[Value]>;
        fn parse_object(&self) -> Option<&Object>;
        fn parse_bool(&self) -> Option<bool>;
        fn serialize(&self) -> PodBytes;
    }
    impl PodValueExt for Value {
        fn parse_int(&self) -> Option<i32> {
            match self {
                Value::Int(x) => Some(*x),
                _ => None,
            }
        }
        fn parse_string(&self) -> Option<&str> {
            match self {
                Value::String(s) => Some(s),
                _ => None,
            }
        }
        fn parse_value_array(&self) -> Option<&ValueArray> {
            match self {
                Value::ValueArray(value_array) => Some(value_array),
                _ => None,
            }
        }
        fn parse_struct(&self) -> Option<&[Value]> {
            match self {
                Value::Struct(struct_) => Some(struct_),
                _ => None,
            }
        }
        fn parse_object(&self) -> Option<&Object> {
            match self {
                Value::Object(object) => Some(object),
                _ => None,
            }
        }
        fn parse_bool(&self) -> Option<bool> {
            match self {
                Value::Bool(bool_) => Some(*bool_),
                _ => None,
            }
        }
        fn serialize(&self) -> PodBytes {
            PodBytes::from_value(self)
        }
    }

    pub trait PodValueArrayExt {
        fn parse_floats(&self) -> Option<&[f32]>;
    }
    impl PodValueArrayExt for ValueArray {
        fn parse_floats(&self) -> Option<&[f32]> {
            match self {
                ValueArray::Float(f) => Some(f),
                _ => None,
            }
        }
    }

    pub trait PodStructExt {
        fn get_key(&self, key: &str) -> Option<&Value>;
    }
    impl PodStructExt for [Value] {
        fn get_key(&self, key: &str) -> Option<&Value> {
            let mut iter = self.iter();
            // Consume items of the iterator up to and including the key
            iter.by_ref()
                .take_while(|val| val.parse_string().map(|s| s == key).unwrap_or_default())
                .count();
            // Return the item after the key
            iter.next()
        }
    }

    pub trait PodObjectExt {
        fn get_key(&self, key: u32) -> Option<&Value>;
    }
    impl PodObjectExt for Object {
        fn get_key(&self, key: u32) -> Option<&Value> {
            self.properties
                .iter()
                .find_map(|prop| (prop.key == key).then_some(&prop.value))
        }
    }

    // Safety: Known ASCII string; using match because const context cannot use .unwrap().
    #[allow(clippy::panic)] // const context requires panic! for the unreachable branch
    pub const STRUCT_KEY_DEVICE_ICON_NAME: &str = match c"device.icon-name".to_str() {
        Ok(s) => s,
        Err(_) => panic!("DEVICE_ICON_NAME key is not valid UTF-8"),
    };
}
use parse::*;

#[derive(Debug)]
pub(super) struct NodeProps {
    value: Value,
}

impl NodeProps {
    pub fn new(value: Value) -> Self {
        NodeProps { value }
    }

    pub fn get_channel_volumes(&self) -> Option<&[f32]> {
        self.value
            .parse_object()?
            .get_key(SPA_PROP_channelVolumes)?
            .parse_value_array()?
            .parse_floats()
    }

    pub fn get_mute(&self) -> Option<bool> {
        self.value
            .parse_object()?
            .get_key(SPA_PROP_mute)?
            .parse_bool()
    }
}

/// `Props '{ channelVolumes: <channel_volumes> }'`
pub fn build_node_volume_pod(channel_volumes: Vec<f32>) -> (ParamType, PodBytes) {
    let pod = Value::Object(object! {
        SpaTypes::ObjectParamProps,
        ParamType::Props,
        Property::new(SPA_PROP_channelVolumes, Value::ValueArray(ValueArray::Float(channel_volumes))),
    }).serialize();
    (ParamType::Props, pod)
}

/// `Props '{ mute: <mute> }'`
pub fn build_node_mute_pod(mute: bool) -> (ParamType, PodBytes) {
    let pod = Value::Object(object! {
        SpaTypes::ObjectParamProps,
        ParamType::Props,
        Property::new(SPA_PROP_mute, Value::Bool(mute)),
    })
    .serialize();
    (ParamType::Props, pod)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceActiveRoute {
    pub route_index: i32,
    pub device_index: i32,
    pub icon_name: Option<String>,
}

impl DeviceActiveRoute {
    pub fn from_value(pod: &Pod) -> Option<Self> {
        let value = pod.deserialize_value()?;
        let obj = value.parse_object()?;
        Some(Self {
            route_index: obj.get_key(SPA_PARAM_ROUTE_index)?.parse_int()?,
            device_index: obj.get_key(SPA_PARAM_ROUTE_device)?.parse_int()?,
            icon_name: obj
                .get_key(SPA_PARAM_ROUTE_info)?
                .parse_struct()?
                .get_key(STRUCT_KEY_DEVICE_ICON_NAME)
                .and_then(|v| v.parse_string())
                .map(ToOwned::to_owned),
        })
    }

    /// `Route '{ index: <route_index>, device: <device_index>, props: { channelVolumes: <channel_volumes> }, save: true }'`
    pub fn build_device_volume_pod(&self, channel_volumes: Vec<f32>) -> (ParamType, PodBytes) {
        let pod = Value::Object(object! {
            SpaTypes::ObjectParamRoute,
            ParamType::Route,
            Property::new(SPA_PARAM_ROUTE_index, Value::Int(self.route_index)),
            Property::new(SPA_PARAM_ROUTE_device, Value::Int(self.device_index)),
            Property::new(SPA_PARAM_ROUTE_props, Value::Object(object! {
                SpaTypes::ObjectParamProps,
                ParamType::Route,
                Property::new(SPA_PROP_channelVolumes, Value::ValueArray(ValueArray::Float(channel_volumes)))
            })),
            Property::new(SPA_PARAM_ROUTE_save, Value::Bool(true)),
        }).serialize();
        (ParamType::Route, pod)
    }

    /// `Route '{ index: <route_index>, device: <device_index>, props: { mute: <mute> }, save: true }'`
    pub fn build_device_mute_pod(&self, mute: bool) -> (ParamType, PodBytes) {
        let pod = Value::Object(object! {
            SpaTypes::ObjectParamRoute,
            ParamType::Route,
            Property::new(SPA_PARAM_ROUTE_index, Value::Int(self.route_index)),
            Property::new(SPA_PARAM_ROUTE_device, Value::Int(self.device_index)),
            Property::new(SPA_PARAM_ROUTE_props, Value::Object(object! {
                SpaTypes::ObjectParamProps,
                ParamType::Route,
                Property::new(SPA_PROP_mute, Value::Bool(mute))
            })),
            Property::new(SPA_PARAM_ROUTE_save, Value::Bool(true)),
        })
        .serialize();
        (ParamType::Route, pod)
    }
}
