#![cfg(feature = "serde")]

use rustyfit::{
    profile::{
        mesgdef,
        typedef::{self, FitBaseType},
    },
    proto::{Field, Value},
};

#[test]
fn serde_json_file_id_roundtrip() {
    let mut file_id = mesgdef::FileId::new();
    file_id.r#type = typedef::File::ACTIVITY;
    file_id.manufacturer = typedef::Manufacturer::GARMIN;
    file_id.product = 0;

    let s = serde_json::to_string(&file_id).unwrap();
    assert_eq!(
        &s,
        r#"{"type":{"t":"activity","c":4},"manufacturer":{"t":"garmin","c":1},"product":0}"#
    );

    let file_id_result: mesgdef::FileId = serde_json::from_str(&s).unwrap();
    assert_eq!(file_id.r#type, file_id_result.r#type);
    assert_eq!(file_id.manufacturer, file_id_result.manufacturer);
    assert_eq!(file_id.product, file_id_result.product);

    // check other field should be invalid
    assert_eq!(file_id.serial_number, u32::MIN);
    assert_eq!(file_id.serial_number, file_id_result.serial_number);

    let mut file_id = mesgdef::FileId::new();
    file_id.r#type = typedef::File(253);
    file_id.manufacturer = typedef::Manufacturer(65534);

    let s = serde_json::to_string(&file_id).unwrap();
    assert_eq!(&s, r#"{"type":{"c":253},"manufacturer":{"c":65534}}"#);

    let file_id_result: mesgdef::FileId = serde_json::from_str(&s).unwrap();
    assert_eq!(file_id.r#type, file_id_result.r#type);
    assert_eq!(file_id.manufacturer, file_id_result.manufacturer);
}

#[test]
fn serde_json_record_roundtrip() {
    let mut record = mesgdef::Record::new();
    record.timestamp = typedef::DateTime::from_unix_timestamp(1781838455);
    record.position_lat = 424480360;
    record.position_long = -940295581;
    record.heart_rate = 70;
    record.distance = 50 * 100;
    record.activity_type = typedef::ActivityType::CYCLING;
    record.unknown_fields = [Field {
        num: 254,
        base_type: FitBaseType::UINT8,
        value: Value::Uint8(10),
        is_expanded: false,
    }]
    .into();

    let s = serde_json::to_string(&record).unwrap();
    assert_eq!(
        s,
        "{\
            \"timestamp\":1781838455,\
            \"position_lat\":35.579532757401466,\
            \"position_long\":-78.81466512568295,\
            \"heart_rate\":70,\
            \"distance\":50.0,\
            \"activity_type\":{\
                \"t\":\"cycling\",\
                \"c\":2\
            },\
            \"unknown_fields\":[\
                {\
                    \"num\":254,\
                    \"base_type\":{\
                        \"t\":\"uint8\",\
                        \"c\":2\
                    },\
                    \"value\":{\
                        \"t\":\"uint8\",\
                        \"c\":10\
                    },\
                    \"is_expanded\":false\
                }\
            ]\
        }"
    );

    let record_result: mesgdef::Record = serde_json::from_str(&s).unwrap();

    assert_eq!(record.timestamp, record_result.timestamp);
    assert_eq!(record.position_lat, record_result.position_lat);
    assert_eq!(record.position_long, record_result.position_long);
    assert_eq!(record.heart_rate, record_result.heart_rate);
    assert_eq!(record.distance, record_result.distance);
    assert_eq!(record.activity_type, record_result.activity_type);

    assert_eq!(
        record.unknown_fields.len(),
        record_result.unknown_fields.len()
    );
    assert_eq!(
        record.unknown_fields[0].num,
        record_result.unknown_fields[0].num
    );
    assert_eq!(
        record.unknown_fields[0].base_type,
        record_result.unknown_fields[0].base_type
    );

    let Value::Uint8(v1) = record.unknown_fields[0].value else {
        panic!("record: {:?}", record.unknown_fields[0].value)
    };
    let Value::Uint8(v2) = record_result.unknown_fields[0].value else {
        panic!("record_result: {:?}", record_result.unknown_fields[0].value)
    };
    assert_eq!(v1, v2);
}
