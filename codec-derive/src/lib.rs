// codec-derive/src/lib.rs
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

    // Generate the marker trait implementation
    let marker_impl = if is_lossy {
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
        impl #name {
            pub const CODEC_NAME: &'static str = #codec_name;
            pub const MIME_TYPE: &'static str = #mime_type;
            pub const EXTENSION: &'static str = #extension;
        }

        impl Codec for #name {
            fn name(&self) -> &'static str {
                Self::CODEC_NAME
            }

            fn mime_type(&self) -> &'static str {
                Self::MIME_TYPE
            }

            fn extension(&self) -> &'static str {
                Self::EXTENSION
            }

            fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
                self.encode_samples(data, sample_rate)
            }

            fn decode(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
                self.decode_samples(data, sample_rate)
            }
        }

        #marker_impl

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::audio::codec::test_utils::TestSignal;

            #[test]
            fn test_basic_reconstruction() -> Result<()> {
                let codec = #name::default();
                let signal = TestSignal::unit_test();

                println!("\nTesting {} codec with basic signal", #codec_name);
                let encoded = codec.encode(&signal.samples, signal.sample_rate)?;
                println!("Encoded size: {} bytes", encoded.len());
                let decoded = codec.decode(&encoded, signal.sample_rate)?;

                assert_eq!(signal.samples.len(), decoded.len(), "Length mismatch");

                println!("\nSample comparison (original → decoded):");
                println!("{:>8} {:>12} {:>12} {:>12}", "Index", "Original", "Decoded", "Diff");
                println!("{:-^47}", "");

                for (i, (original, decoded)) in signal.samples.iter().zip(decoded.iter()).enumerate() {
                    let diff = (original - decoded).abs();
                    println!(
                        "{:>8} {:>12.6} {:>12.6} {:>12.6}",
                        i, original, decoded, diff
                    );

                    let tolerance = if #is_lossy { 1.0 / 100.0 } else { 1.0 / 32768.0 };
                    assert!(
                        diff < tolerance,
                        "Sample {} differs too much: {} vs {} (diff: {}, tolerance: {})",
                        i, original, decoded, diff, tolerance
                    );
                }

                Ok(())
            }

            #[test]
            fn test_performance() -> Result<()> {
                let codec = #name::default();
                let signal = TestSignal::sine(44100, 440.0, 1.0);
                println!("\nTesting {} codec performance", #codec_name);

                let perf = codec.measure_performance(&signal.samples, signal.sample_rate)?;

                println!("\nPerformance metrics:");
                println!("Encode speed: {:.2}x realtime", perf.encode_speed);
                println!("Decode speed: {:.2}x realtime", perf.decode_speed);
                println!("Compression ratio: {:.2}:1", perf.compression_ratio);

                assert!(
                    perf.encode_speed > 1.0,
                    "Encoding too slow: {:.2}x realtime",
                    perf.encode_speed
                );
                assert!(
                    perf.decode_speed > 1.0,
                    "Decoding too slow: {:.2}x realtime",
                    perf.decode_speed
                );

                Ok(())
            }

            #[test]
            fn test_codec_properties() {
                let codec = #name::default();
                println!("\n{} codec properties:", #codec_name);
                println!("Name: {}", codec.name());
                println!("MIME type: {}", codec.mime_type());
                println!("File extension: {}", codec.extension());
                println!("Type: {}", if #is_lossy { "Lossy" } else { "Lossless" });
            }
        }
    };

    TokenStream::from(expanded)
}
