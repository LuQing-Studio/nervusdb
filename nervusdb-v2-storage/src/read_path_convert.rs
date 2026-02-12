use crate::property::PropertyValue;
use crate::snapshot::EdgeKey;
use std::collections::BTreeMap;

pub(crate) fn convert_property_to_api(value: PropertyValue) -> nervusdb_v2_api::PropertyValue {
    match value {
        PropertyValue::Null => nervusdb_v2_api::PropertyValue::Null,
        PropertyValue::Bool(v) => nervusdb_v2_api::PropertyValue::Bool(v),
        PropertyValue::Int(v) => nervusdb_v2_api::PropertyValue::Int(v),
        PropertyValue::Float(v) => nervusdb_v2_api::PropertyValue::Float(v),
        PropertyValue::String(v) => nervusdb_v2_api::PropertyValue::String(v),
        PropertyValue::DateTime(v) => nervusdb_v2_api::PropertyValue::DateTime(v),
        PropertyValue::Blob(v) => nervusdb_v2_api::PropertyValue::Blob(v),
        PropertyValue::List(values) => nervusdb_v2_api::PropertyValue::List(
            values.into_iter().map(convert_property_to_api).collect(),
        ),
        PropertyValue::Map(values) => {
            nervusdb_v2_api::PropertyValue::Map(convert_property_map_to_api(values))
        }
    }
}

pub(crate) fn convert_property_map_to_api(
    props: BTreeMap<String, PropertyValue>,
) -> BTreeMap<String, nervusdb_v2_api::PropertyValue> {
    props
        .into_iter()
        .map(|(key, value)| (key, convert_property_to_api(value)))
        .collect()
}

pub(crate) fn api_edge_to_internal(edge: nervusdb_v2_api::EdgeKey) -> EdgeKey {
    EdgeKey {
        src: edge.src,
        rel: edge.rel,
        dst: edge.dst,
    }
}

pub(crate) fn internal_edge_to_api(edge: EdgeKey) -> nervusdb_v2_api::EdgeKey {
    nervusdb_v2_api::EdgeKey {
        src: edge.src,
        rel: edge.rel,
        dst: edge.dst,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        api_edge_to_internal, convert_property_map_to_api, convert_property_to_api,
        internal_edge_to_api,
    };
    use crate::property::PropertyValue;
    use crate::snapshot::EdgeKey;
    use std::collections::BTreeMap;

    #[test]
    fn convert_property_to_api_preserves_nested_list_and_map() {
        let mut inner_map = BTreeMap::new();
        inner_map.insert("x".to_string(), PropertyValue::Int(7));
        inner_map.insert(
            "y".to_string(),
            PropertyValue::List(vec![PropertyValue::Bool(true), PropertyValue::Null]),
        );

        let value = PropertyValue::List(vec![
            PropertyValue::String("root".to_string()),
            PropertyValue::Map(inner_map),
        ]);

        let converted = convert_property_to_api(value);
        let expected = nervusdb_v2_api::PropertyValue::List(vec![
            nervusdb_v2_api::PropertyValue::String("root".to_string()),
            nervusdb_v2_api::PropertyValue::Map(BTreeMap::from([
                ("x".to_string(), nervusdb_v2_api::PropertyValue::Int(7)),
                (
                    "y".to_string(),
                    nervusdb_v2_api::PropertyValue::List(vec![
                        nervusdb_v2_api::PropertyValue::Bool(true),
                        nervusdb_v2_api::PropertyValue::Null,
                    ]),
                ),
            ])),
        ]);

        assert_eq!(converted, expected);
    }

    #[test]
    fn convert_property_map_to_api_preserves_scalars() {
        let props = BTreeMap::from([
            ("n".to_string(), PropertyValue::Int(42)),
            ("s".to_string(), PropertyValue::String("ok".to_string())),
            ("dt".to_string(), PropertyValue::DateTime(1700000000)),
            ("blob".to_string(), PropertyValue::Blob(vec![1, 2, 3])),
        ]);

        let converted = convert_property_map_to_api(props);
        let expected = BTreeMap::from([
            ("n".to_string(), nervusdb_v2_api::PropertyValue::Int(42)),
            (
                "s".to_string(),
                nervusdb_v2_api::PropertyValue::String("ok".to_string()),
            ),
            (
                "dt".to_string(),
                nervusdb_v2_api::PropertyValue::DateTime(1700000000),
            ),
            (
                "blob".to_string(),
                nervusdb_v2_api::PropertyValue::Blob(vec![1, 2, 3]),
            ),
        ]);

        assert_eq!(converted, expected);
    }

    #[test]
    fn api_edge_to_internal_keeps_all_fields() {
        let api_edge = nervusdb_v2_api::EdgeKey {
            src: 11,
            rel: 22,
            dst: 33,
        };
        let internal = api_edge_to_internal(api_edge);
        assert_eq!(
            internal,
            EdgeKey {
                src: 11,
                rel: 22,
                dst: 33,
            }
        );
    }

    #[test]
    fn internal_edge_to_api_keeps_all_fields() {
        let internal = EdgeKey {
            src: 44,
            rel: 55,
            dst: 66,
        };
        let api = internal_edge_to_api(internal);
        assert_eq!(
            api,
            nervusdb_v2_api::EdgeKey {
                src: 44,
                rel: 55,
                dst: 66,
            }
        );
    }
}
