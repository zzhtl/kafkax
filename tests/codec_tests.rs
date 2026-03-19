use kafkax::codec::*;
use kafkax::kafka::types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_decode_valid() {
        let data = br#"{"name": "test", "value": 42}"#;
        let result = json::decode_json(data);
        match result {
            DecodedPayload::Json(v) => {
                assert_eq!(v["name"], "test");
                assert_eq!(v["value"], 42);
            }
            _ => panic!("期望 JSON 解码结果"),
        }
    }

    #[test]
    fn test_json_decode_invalid() {
        let data = b"not json at all";
        let result = json::decode_json(data);
        assert!(matches!(result, DecodedPayload::Error(_)));
    }

    #[test]
    fn test_json_decode_array() {
        let data = br#"[1, 2, 3]"#;
        let result = json::decode_json(data);
        assert!(matches!(result, DecodedPayload::Json(_)));
    }

    #[test]
    fn test_encoding_utf8() {
        let data = "Hello, 世界!".as_bytes();
        let result = encoding::detect_and_decode(data);
        assert_eq!(result, "Hello, 世界!");
    }

    #[test]
    fn test_encoding_ascii() {
        let data = b"Hello, World!";
        let result = encoding::detect_and_decode(data);
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_decoder_pipeline_auto_json() {
        let pipeline = DecoderPipeline::default();
        let msg = KafkaMessage {
            topic: "test".to_string(),
            partition: 0,
            offset: 100,
            timestamp: None,
            key: Some(b"key-1".to_vec()),
            payload: Some(br#"{"hello": "world"}"#.to_vec()),
        };
        let decoded = pipeline.decode(msg);
        assert!(matches!(decoded.decoded_value, DecodedPayload::Json(_)));
        assert_eq!(decoded.decoded_key, Some("key-1".to_string()));
    }

    #[test]
    fn test_decoder_pipeline_auto_text() {
        let pipeline = DecoderPipeline::default();
        let msg = KafkaMessage {
            topic: "test".to_string(),
            partition: 0,
            offset: 101,
            timestamp: None,
            key: None,
            payload: Some(b"plain text message".to_vec()),
        };
        let decoded = pipeline.decode(msg);
        assert!(matches!(decoded.decoded_value, DecodedPayload::Text(_)));
    }

    #[test]
    fn test_decoder_pipeline_null_payload() {
        let pipeline = DecoderPipeline::default();
        let msg = KafkaMessage {
            topic: "test".to_string(),
            partition: 0,
            offset: 102,
            timestamp: None,
            key: None,
            payload: None,
        };
        let decoded = pipeline.decode(msg);
        match &decoded.decoded_value {
            DecodedPayload::Text(s) => assert_eq!(s, "<null>"),
            _ => panic!("期望 Text(<null>)"),
        }
    }

    #[test]
    fn test_decoder_pipeline_empty_payload() {
        let pipeline = DecoderPipeline::default();
        let msg = KafkaMessage {
            topic: "test".to_string(),
            partition: 0,
            offset: 103,
            timestamp: None,
            key: None,
            payload: Some(vec![]),
        };
        let decoded = pipeline.decode(msg);
        match &decoded.decoded_value {
            DecodedPayload::Text(s) => assert_eq!(s, "<empty>"),
            _ => panic!("期望 Text(<empty>)"),
        }
    }

    #[test]
    fn test_decoder_pipeline_binary_mode() {
        let pipeline = DecoderPipeline::new(DecoderType::Binary);
        let msg = KafkaMessage {
            topic: "test".to_string(),
            partition: 0,
            offset: 104,
            timestamp: None,
            key: None,
            payload: Some(vec![0x00, 0xFF, 0xAB, 0xCD]),
        };
        let decoded = pipeline.decode(msg);
        assert!(matches!(decoded.decoded_value, DecodedPayload::Binary(_)));
    }

    #[test]
    fn test_decoder_pipeline_forced_json() {
        let pipeline = DecoderPipeline::new(DecoderType::Json);
        let msg = KafkaMessage {
            topic: "test".to_string(),
            partition: 0,
            offset: 105,
            timestamp: None,
            key: None,
            payload: Some(br#"{"x": 1}"#.to_vec()),
        };
        let decoded = pipeline.decode(msg);
        assert!(matches!(decoded.decoded_value, DecodedPayload::Json(_)));
    }

    #[test]
    fn test_decoder_batch() {
        let pipeline = DecoderPipeline::default();
        let msgs = vec![
            KafkaMessage {
                topic: "t".to_string(),
                partition: 0,
                offset: 0,
                timestamp: None,
                key: None,
                payload: Some(br#"{"a":1}"#.to_vec()),
            },
            KafkaMessage {
                topic: "t".to_string(),
                partition: 0,
                offset: 1,
                timestamp: None,
                key: None,
                payload: Some(b"text".to_vec()),
            },
        ];
        let decoded = pipeline.decode_batch(msgs);
        assert_eq!(decoded.len(), 2);
        assert!(matches!(decoded[0].decoded_value, DecodedPayload::Json(_)));
        assert!(matches!(decoded[1].decoded_value, DecodedPayload::Text(_)));
    }

    #[test]
    fn test_decoded_payload_summary() {
        let payload = DecodedPayload::Json(serde_json::json!({"key": "value"}));
        let summary = payload.summary(10);
        assert!(summary.len() <= 13); // 10 + "..."
    }

    #[test]
    fn test_decoded_payload_format_name() {
        assert_eq!(DecodedPayload::Json(serde_json::Value::Null).format_name(), "JSON");
        assert_eq!(DecodedPayload::Text("".to_string()).format_name(), "Text");
        assert_eq!(DecodedPayload::Binary(vec![]).format_name(), "Binary");
        assert_eq!(DecodedPayload::Protobuf("".to_string()).format_name(), "Protobuf");
        assert_eq!(DecodedPayload::Avro("".to_string()).format_name(), "Avro");
        assert_eq!(DecodedPayload::MsgPack(serde_json::Value::Null).format_name(), "MsgPack");
        assert_eq!(DecodedPayload::Error("".to_string()).format_name(), "Error");
    }

    #[test]
    fn test_hex_dump_display() {
        let payload = DecodedPayload::Binary(vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]);
        let display = payload.full_display();
        assert!(display.contains("48 65 6C 6C 6F"));
        assert!(display.contains("|Hello|"));
    }

    #[test]
    fn test_msgpack_decode() {
        // 编码一个简单的 msgpack 值
        let value = serde_json::json!({"hello": "world"});
        let encoded = rmp_serde::to_vec(&value).unwrap();
        let result = msgpack::decode_msgpack(&encoded);
        match result {
            DecodedPayload::MsgPack(v) => {
                assert_eq!(v["hello"], "world");
            }
            _ => panic!("期望 MsgPack 解码结果"),
        }
    }

    #[test]
    fn test_msgpack_decode_invalid() {
        // 空数据应该解码失败
        let result = msgpack::decode_msgpack(&[]);
        assert!(matches!(result, DecodedPayload::Error(_)));
    }

    #[test]
    fn test_protobuf_heuristic() {
        // 手工构造一个简单的 protobuf: field 1 = varint 150
        let data = vec![0x08, 0x96, 0x01];
        let result = protobuf::decode_protobuf(&data);
        match result {
            DecodedPayload::Protobuf(s) => {
                assert!(s.contains("field_1"));
                assert!(s.contains("150"));
            }
            _ => panic!("期望 Protobuf 解码结果, got {:?}", result),
        }
    }
}
