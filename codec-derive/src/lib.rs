use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Codec, attributes(codec))]
pub fn derive_codec(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Parse attributes
    let mut codec_name = None;
    let mut mime_type = None;
    let mut extension = None;
    let mut is_lossy = false;
    let mut is_lossless = false;

    for attr in &input.attrs {
        if !attr.path().is_ident("codec") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                codec_name = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("mime") {
                mime_type = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("extension") {
                extension = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("lossy") {
                is_lossy = true;
            } else if meta.path.is_ident("lossless") {
                is_lossless = true;
            }
            Ok(())
        })
        .unwrap();
    }

    let codec_name = codec_name.expect("codec name is required");
    let mime_type = mime_type.expect("mime type is required");
    let extension = extension.expect("extension is required");

    if is_lossy == is_lossless {
        panic!("Codec must be marked as either lossy or lossless");
    }

    // Generate trait implementations
    let trait_impl = if is_lossy {
        quote! {
            impl LossyCodec for #name {
                fn target_bitrate(&self) -> Option<u32> {
                    None
                }
            }
        }
    } else {
        quote! {
            impl LosslessCodec for #name {}
        }
    };

    let expanded = quote! {
        impl Codec for #name {
            fn name(&self) -> &'static str {
                #codec_name
            }

            fn mime_type(&self) -> &'static str {
                #mime_type
            }

            fn extension(&self) -> &'static str {
                #extension
            }

            fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
                let mut buffer = self.write_header(data.len(), sample_rate);
                let bytes_per_sample = self.bits_per_sample as usize / 8;
                buffer.reserve(data.len() * bytes_per_sample);

                for &sample in data {
                    buffer.extend(self.encode_sample(sample));
                }

                Ok(buffer)
            }

            fn decode(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
                if data.len() < 44 {
                    return Err(Error::Audio(AudioError::Codec(CodecError::InvalidData(
                        "WAV header too short",
                    ))));
                }

                if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
                    return Err(Error::Audio(AudioError::Codec(CodecError::InvalidData(
                        "Invalid WAV header",
                    ))));
                }

                let mut offset = 12;
                while offset + 8 <= data.len() {
                    let chunk_id = &data[offset..offset + 4];
                    let chunk_size = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap());

                    if chunk_id == b"data" {
                        let data_offset = offset + 8;
                        let bytes_per_sample = self.bits_per_sample as usize / 8;
                        let mut samples = Vec::new();

                        for chunk in data[data_offset..].chunks_exact(bytes_per_sample) {
                            samples.push(self.decode_sample(chunk)?);
                        }

                        return Ok(samples);
                    }

                    offset += 8 + chunk_size as usize;
                }

                Err(Error::Audio(AudioError::Codec(CodecError::InvalidData(
                    "No data chunk found",
                ))))
            }
        }

        #trait_impl
    };

    TokenStream::from(expanded)
}
