use crate::kafka::types::DecodedPayload;

/// 自定义解码器 trait
pub trait CustomDecoder: Send + Sync {
    /// 解码器名称
    fn name(&self) -> &str;

    /// 尝试解码，返回 None 表示此解码器不支持该数据
    fn try_decode(&self, data: &[u8]) -> Option<DecodedPayload>;
}

/// 解码器注册表
#[derive(Default)]
pub struct DecoderRegistry {
    decoders: Vec<Box<dyn CustomDecoder>>,
}

impl DecoderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册自定义解码器
    pub fn register(&mut self, decoder: Box<dyn CustomDecoder>) {
        self.decoders.push(decoder);
    }

    /// 尝试使用注册的解码器解码
    pub fn try_decode(&self, data: &[u8]) -> Option<DecodedPayload> {
        for decoder in &self.decoders {
            if let Some(result) = decoder.try_decode(data) {
                return Some(result);
            }
        }
        None
    }

    /// 获取所有解码器名称
    pub fn names(&self) -> Vec<&str> {
        self.decoders.iter().map(|d| d.name()).collect()
    }
}

impl std::fmt::Debug for DecoderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecoderRegistry")
            .field("count", &self.decoders.len())
            .finish()
    }
}
