use super::proto::google::protobuf::Timestamp;
use bson::oid::ObjectId;
use std::time::{Duration, SystemTime};

pub fn object_id_to_timestamp_proto(object_id: &ObjectId) -> Timestamp {
    chrono_timestamp_to_timestamp_proto(&object_id.timestamp())
}

pub fn chrono_timestamp_to_timestamp_proto(
    chrono_timestamp: &chrono::DateTime<chrono::offset::Utc>,
) -> Timestamp {
    let mut seconds = chrono_timestamp.timestamp();
    let mut nanos = chrono_timestamp.timestamp_subsec_nanos();

    if nanos > 999999999 {
        nanos -= 1000000000;
        seconds += 1;
    }

    Timestamp {
        seconds,
        nanos: nanos as i32,
    }
}

pub fn get_current_timestamp_proto() -> Timestamp {
    system_time_to_timestamp_proto(&SystemTime::now())
}

pub fn timestamp_proto_to_system_time(timestamp_proto: &Timestamp) -> SystemTime {
    let mut system_time = SystemTime::UNIX_EPOCH;
    if timestamp_proto.seconds >= 0 {
        system_time += Duration::from_secs(timestamp_proto.seconds as u64);
    } else {
        system_time -= Duration::from_secs(-timestamp_proto.seconds as u64);
    };
    if timestamp_proto.nanos >= 0 {
        system_time += Duration::from_nanos(timestamp_proto.nanos as u64);
    } else {
        system_time -= Duration::from_nanos(-timestamp_proto.nanos as u64);
    };
    system_time
}

pub fn system_time_to_timestamp_proto(system_time: &SystemTime) -> Timestamp {
    let (duration_since_epoch, is_negative) =
        match system_time.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => (duration, false),
            Err(e) => (e.duration(), true),
        };
    if is_negative {
        let mut seconds = -(duration_since_epoch.as_secs() as i64);
        let mut nanos = -(duration_since_epoch.subsec_nanos() as i32);
        if nanos < 0 {
            nanos += 1000000000;
            seconds -= 1;
        }
        Timestamp { seconds, nanos }
    } else {
        Timestamp {
            seconds: duration_since_epoch.as_secs() as i64,
            nanos: duration_since_epoch.subsec_nanos() as i32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDateTime, Utc};

    #[test]
    fn test_object_id_to_timestamp_proto() {
        let object_id = ObjectId::with_string("507c7f79bcf86cd7994f6c0e").unwrap();
        let timestamp = object_id_to_timestamp_proto(&object_id);
        assert_eq!(
            timestamp,
            Timestamp {
                seconds: 1350336377,
                nanos: 0
            }
        );
    }

    #[test]
    fn test_chrono_timestamp_to_timestamp_proto() {
        let mut utc_date_time =
            DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(1350336377, 1234), Utc);
        let mut timestamp = chrono_timestamp_to_timestamp_proto(&utc_date_time);
        assert_eq!(
            timestamp,
            Timestamp {
                seconds: 1350336377,
                nanos: 1234
            }
        );
        utc_date_time =
            DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(1, 1234567890), Utc);
        timestamp = chrono_timestamp_to_timestamp_proto(&utc_date_time);
        assert_eq!(
            timestamp,
            Timestamp {
                seconds: 2,
                nanos: 234567890
            }
        );
    }

    #[test]
    fn test_convert_to_system_time() {
        // Initialize proto with some arbitrary values.
        // Only accurate within 100 nanoseconds.
        let time_before = Timestamp {
            seconds: 1234,
            nanos: 5600,
        };
        let time_after = timestamp_proto_to_system_time(&time_before);
        assert_eq!(
            time_before.seconds,
            time_after
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64
        );
    }

    #[test]
    fn test_convert_to_timestamp_proto() {
        let time_before = SystemTime::now();
        let time_after = system_time_to_timestamp_proto(&time_before);
        assert_eq!(
            time_before
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            time_after.seconds
        );
    }

    #[test]
    fn test_convert_back_and_forth() {
        let system_time_before = SystemTime::now();
        let system_time_after =
            timestamp_proto_to_system_time(&system_time_to_timestamp_proto(&system_time_before));
        assert_eq!(system_time_before, system_time_after);

        // Initialize proto with some arbitrary values.
        // Only accurate within 100 nanoseconds.
        let proto_time_before = Timestamp {
            seconds: 1234,
            nanos: 5600,
        };
        let proto_time_after =
            system_time_to_timestamp_proto(&timestamp_proto_to_system_time(&proto_time_before));
        assert_eq!(proto_time_before, proto_time_after);
    }

    #[test]
    fn test_convert_back_and_forth_negative_seconds() {
        let system_time_before = SystemTime::now();
        let system_time_after =
            timestamp_proto_to_system_time(&system_time_to_timestamp_proto(&system_time_before));
        assert_eq!(system_time_before, system_time_after);

        // Initialize proto with some arbitrary values.
        // Only accurate within 100 nanoseconds.
        let proto_time_before = Timestamp {
            seconds: -1234,
            nanos: 5600,
        };
        let proto_time_after =
            system_time_to_timestamp_proto(&timestamp_proto_to_system_time(&proto_time_before));
        assert_eq!(proto_time_before, proto_time_after);
    }

    #[test]
    fn test_proto_nanos_is_never_negative() {
        // According to proto3 documentation for Timestamp, nanos
        // should never be negative. So, any negative value should
        // be converted to positive by subtracting one second and adding
        // one billion nanoseconds. Here we test various scenarios
        // to make sure that conversions never result in a negative
        // value for nanos.

        // Initialize proto with some arbitrary values.
        // Only accurate within 100 nanoseconds.
        let mut proto_time_before = Timestamp {
            seconds: -1234,
            nanos: -2000000100,
        };
        let mut proto_time_after =
            system_time_to_timestamp_proto(&timestamp_proto_to_system_time(&proto_time_before));
        assert_eq!(-1237, proto_time_after.seconds);
        assert_eq!(999999900, proto_time_after.nanos);

        proto_time_before = Timestamp {
            seconds: 1234,
            nanos: -2000000100,
        };
        proto_time_after =
            system_time_to_timestamp_proto(&timestamp_proto_to_system_time(&proto_time_before));
        assert_eq!(1231, proto_time_after.seconds);
        assert_eq!(999999900, proto_time_after.nanos);

        proto_time_before = Timestamp {
            seconds: 1234,
            nanos: -999999900,
        };
        proto_time_after =
            system_time_to_timestamp_proto(&timestamp_proto_to_system_time(&proto_time_before));
        assert_eq!(1233, proto_time_after.seconds);
        assert_eq!(100, proto_time_after.nanos);

        proto_time_before = Timestamp {
            seconds: -1,
            nanos: -999999900,
        };
        proto_time_after =
            system_time_to_timestamp_proto(&timestamp_proto_to_system_time(&proto_time_before));
        assert_eq!(-2, proto_time_after.seconds);
        assert_eq!(100, proto_time_after.nanos);
    }
}
