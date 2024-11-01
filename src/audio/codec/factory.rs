use super::flac::FlacCodec;
use super::moshi::MoshiCodec;
use super::opus::OpusCodec;
use super::traits::Codec;
use super::wav::WavCodec;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;

pub struct CodecFactory {
    codecs: HashMap<String, Arc<dyn Codec>>,
}

impl CodecFactory {
    pub fn new() -> Self {
        let mut codecs = HashMap::new();

        // Register built-in codecs
        codecs.insert(
            "wav".into(),
            Arc::new(WavCodec::default()) as Arc<dyn Codec>,
        );
        codecs.insert(
            "flac".into(),
            Arc::new(FlacCodec::default()) as Arc<dyn Codec>,
        );
        codecs.insert(
            "opus".into(),
            Arc::new(OpusCodec::default()) as Arc<dyn Codec>,
        );
        codecs.insert(
            "moshi".into(),
            Arc::new(MoshiCodec::default()) as Arc<dyn Codec>,
        );

        Self { codecs }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Codec>> {
        self.codecs.get(name).cloned()
    }

    pub fn register<C: Codec + 'static>(&mut self, name: &str, codec: C) {
        self.codecs
            .insert(name.to_string(), Arc::new(codec) as Arc<dyn Codec>);
    }
}

impl Default for CodecFactory {
    fn default() -> Self {
        Self::new()
    }
}

pub static CODEC_FACTORY: Lazy<CodecFactory> = Lazy::new(CodecFactory::default);
