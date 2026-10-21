use super::*;
use arrow::array::{Int64Array, LargeStringArray, StringArray};
use serde_json::json;

fn spec(v: serde_json::Value) -> DataType {
    data_type_from_spec(&v).unwrap()
}

/// The stored schema of the cases below: a record of a string and an integer, and a list
/// of records of two strings.
fn stored() -> DataType {
    spec(json!({"Struct": [
        ["r", {"Struct": [["x", "String"], ["y", "Int64"]]}],
        ["l", {"List": {"Struct": [["p", "String"], ["q", "String"]]}}],
    ]}))
}

fn check(inferred: serde_json::Value) -> Vec<String> {
    incompatibilities(&spec(inferred), &stored(), SchemaCheck::Compatible)
}

/// wikidata-pq's `is_acceptable_diff` accepts these (DeepDiff reports them as removed
/// items, or as type changes to `Null`)
#[test]
fn test_missing_fields_and_nulls_are_compatible() {
    assert!(check(json!({"Struct": [["r", {"Struct": [["x", "String"]]}]]})).is_empty());
    assert!(check(json!({"Struct": [["r", "Null"]]})).is_empty());
    assert!(check(json!({"Struct": [["l", {"List": {"Struct": [["p", "String"]]}}]]})).is_empty());
    assert!(check(json!({"Struct": [["l", {"List": "Null"}]]})).is_empty());
    assert!(check(json!({"Struct": [["l", "Null"]]})).is_empty());
    assert!(check(json!({"Struct": []})).is_empty());
}

/// ...and rejects these: a field the stored schema lacks, at any depth, and any other
/// change of type (no widening)
#[test]
fn test_extra_fields_and_other_types_are_incompatible() {
    assert_eq!(
        check(json!({"Struct": [["r", {"Struct": [["x", "String"], ["z", "Int64"]]}]]})),
        ["r.z: a field the target schema lacks (Int64)"]
    );
    assert_eq!(
        check(json!({"Struct": [["l", {"List": {"Struct": [["p", "String"], ["z", "Int64"]]}}]]})),
        ["l[].z: a field the target schema lacks (Int64)"]
    );
    assert_eq!(
        check(json!({"Struct": [["r", {"Struct": [["y", "Float64"]]}]]})),
        ["r.y: Float64, the target has Int64"]
    );
    assert_eq!(check(json!({"Struct": [["r", "String"]]})), ["r: String, the target has Struct"]);
    assert_eq!(
        check(json!({"Struct": [["l", {"Struct": [["p", "String"]]}]]})),
        ["l: Struct, the target has List"]
    );
}

/// A target field typed `Null` (never seen with a value when it was stored) takes only
/// `Null`: a value there is drift
#[test]
fn test_a_null_target_takes_only_null() {
    let target = spec(json!({"Struct": [["altitude", "Null"]]}));
    let inferred = spec(json!({"Struct": [["altitude", "Float64"]]}));
    assert_eq!(
        incompatibilities(&inferred, &target, SchemaCheck::Compatible),
        ["altitude: Float64, the target has Null"]
    );
    let null = spec(json!({"Struct": [["altitude", "Null"]]}));
    assert!(incompatibilities(&null, &target, SchemaCheck::Compatible).is_empty());
}

#[test]
fn test_maps_compare_keys_and_values() {
    let target = spec(json!({"Map": ["String", {"Struct": [["a", "Int64"]]}]}));
    assert!(incompatibilities(&spec(json!({"Map": ["String", "Null"]})), &target, SchemaCheck::Compatible).is_empty());
    assert_eq!(
        incompatibilities(&spec(json!({"Map": ["String", {"Struct": [["b", "Int64"]]}]})), &target, SchemaCheck::Compatible),
        ["{}.b: a field the target schema lacks (Int64)"]
    );
}

#[test]
fn test_strict_requires_the_target_and_none_checks_nothing() {
    let missing = spec(json!({"Struct": [["r", {"Struct": [["x", "String"]]}], ["l", "Null"]]}));
    assert_eq!(
        incompatibilities(&missing, &stored(), SchemaCheck::Strict),
        ["r.y: missing, and the schema check is strict", "l: Null, the target has List"]
    );
    assert!(incompatibilities(&stored(), &stored(), SchemaCheck::Strict).is_empty());
    let extra = spec(json!({"Struct": [["z", "Int64"]]}));
    assert!(incompatibilities(&extra, &stored(), SchemaCheck::None).is_empty());
}

/// A list of records missing a field, with a null row and a null list: the values are
/// kept, the missing field is null in every record, the rows' nulls are kept, and the
/// list's and strings' offsets are widened to the target's
#[test]
fn test_conform_fills_missing_nested_fields() {
    let inner = Fields::from(vec![Field::new("p", DataType::Utf8, true)]);
    let records = StructArray::new(inner.clone(), vec![Arc::new(StringArray::from(vec!["a", "b", "c"])) as ArrayRef], None);
    let list = ListArray::new(
        Arc::new(Field::new("item", DataType::Struct(inner), true)),
        arrow::buffer::OffsetBuffer::from_lengths([2, 0, 1]),
        Arc::new(records),
        Some(NullBuffer::from(vec![true, false, true])),
    );
    let row = StructArray::new(
        Fields::from(vec![Field::new("l", list.data_type().clone(), true), Field::new("r", DataType::Null, true)]),
        vec![Arc::new(list) as ArrayRef, new_null_array(&DataType::Null, 3)],
        None,
    );
    let out = conform(&(Arc::new(row) as ArrayRef), &stored()).unwrap();
    assert_eq!(out.data_type(), &stored());
    let out = out.as_any().downcast_ref::<StructArray>().unwrap();
    // The target's order: r, then l
    assert_eq!(out.column(0).null_count(), 3);
    let l = out.column(1).as_any().downcast_ref::<LargeListArray>().unwrap();
    assert_eq!(l.null_count(), 1);
    assert_eq!(l.value_offsets(), [0, 2, 2, 3]);
    let items = l.values().as_any().downcast_ref::<StructArray>().unwrap();
    let p = items.column_by_name("p").unwrap().as_any().downcast_ref::<LargeStringArray>().unwrap();
    assert_eq!(p.iter().collect::<Vec<_>>(), [Some("a"), Some("b"), Some("c")]);
    assert_eq!(items.column_by_name("q").unwrap().null_count(), 3);
}

#[test]
fn test_conform_leaves_a_matching_array_as_it_is() {
    let array: ArrayRef = Arc::new(Int64Array::from(vec![1, 2]));
    let out = conform(&array, &DataType::Int64).unwrap();
    assert!(Arc::ptr_eq(&array, &out));
}

#[test]
fn test_conform_maps_values() {
    let target = spec(json!({"Map": ["String", {"Struct": [["a", "Int64"], ["b", "String"]]}]}));
    let have = spec(json!({"Map": ["String", {"Struct": [["a", "Int64"]]}]}));
    let DataType::Map(entries, _) = &have else { unreachable!() };
    let DataType::Struct(kv) = entries.data_type() else { unreachable!() };
    let DataType::Struct(value_fields) = kv[1].data_type() else { unreachable!() };
    let values = StructArray::new(value_fields.clone(), vec![Arc::new(Int64Array::from(vec![7])) as ArrayRef], None);
    let entries_array = StructArray::new(
        kv.clone(),
        vec![Arc::new(LargeStringArray::from(vec!["k"])) as ArrayRef, Arc::new(values)],
        None,
    );
    let map = MapArray::new(entries.clone(), arrow::buffer::OffsetBuffer::from_lengths([1]), entries_array, None, false);
    let out = conform(&(Arc::new(map) as ArrayRef), &target).unwrap();
    assert_eq!(out.data_type(), &target);
}

#[test]
fn test_unnest_nulls_a_null_rows_fields() {
    let row = StructArray::new(
        Fields::from(vec![Field::new("a", DataType::Int64, true)]),
        vec![Arc::new(Int64Array::from(vec![1, 2])) as ArrayRef],
        Some(NullBuffer::from(vec![true, false])),
    );
    let columns = unnest(&row).unwrap();
    assert_eq!(columns.len(), 1);
    assert_eq!(columns[0].0.name(), "a");
    assert!(columns[0].1.is_valid(0) && columns[0].1.is_null(1));
}

#[test]
fn test_unsupported_target_dtypes_are_errors() {
    assert!(data_type_from_spec(&json!("Datetime(time_unit='us', time_zone=None)")).is_err());
    assert!(data_type_from_spec(&json!({"Decimal": [10, 2]})).is_err());
    assert_eq!("strict".parse::<SchemaCheck>(), Ok(SchemaCheck::Strict));
    assert!("loose".parse::<SchemaCheck>().is_err());
}

/// Target dtypes have the offsets Polars writes to Parquet: 64-bit for strings, binaries
/// and lists
#[test]
fn test_target_dtypes_have_polars_offsets() {
    assert_eq!(spec(json!("String")), DataType::LargeUtf8);
    assert_eq!(spec(json!("Binary")), DataType::LargeBinary);
    assert_eq!(spec(json!({"List": "Int64"})), DataType::new_large_list(DataType::Int64, true));
    // and an inferred schema's 32-bit offsets are no incompatibility
    let inferred = DataType::new_list(DataType::Utf8, true);
    assert!(incompatibilities(&inferred, &spec(json!({"List": "String"})), SchemaCheck::Strict).is_empty());
}
