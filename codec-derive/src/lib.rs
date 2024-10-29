use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Codec, attributes(codec))]
pub fn derive_codec(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

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

    // Generate the test module with our comprehensive test suite
    let test_module = generate_test_module(name, &codec_name, is_lossy);

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

            fn is_lossy(&self) -> bool {
                #is_lossy
            }

            fn is_lossless(&self) -> bool {
                #is_lossless
            }
        }

        #test_module
    };

    TokenStream::from(expanded)
}

fn generate_test_module(
    codec_type: &syn::Ident,
    codec_name: &str,
    is_lossy: bool,
) -> proc_macro2::TokenStream {
    quote! {
        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::prelude::*;
            use crate::audio::codec::{
                test_utils::{AudioTestSuite, AudioQualityMetrics, AudioCategory},
                traits::CodecPerformance,
            };

            #[test]
            fn test_codec_properties() {
                let codec = #codec_type::default();
                println!("\n{} codec properties:", #codec_name);
                println!("Name: {}", codec.name());
                println!("MIME type: {}", codec.mime_type());
                println!("File extension: {}", codec.extension());
                println!("Type: {}", if #is_lossy { "Lossy" } else { "Lossless" });
            }

            #[test]
            fn test_basic_reconstruction() -> Result<()> {
                let codec = #codec_type::default();
                let test_suite = AudioTestSuite::load_default_cases()?;

                println!("\nTesting {} codec with basic signal", #codec_name);

                for test_case in test_suite.iter() {
                    // Normalize input samples to prevent overflow
                    let max_amp = test_case.samples.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
                    let normalized: Vec<f32> = if max_amp > 0.0 {
                        test_case.samples.iter().map(|&x| x / max_amp).collect()
                    } else {
                        test_case.samples.clone()
                    };

                    let encoded = codec.encode_samples(&normalized, test_case.sample_rate)?;
                    println!("\nEncoded {} ({}) size: {} bytes",
                        test_case.name,
                        test_case.category,
                        encoded.len()
                    );

                    let decoded = codec.decode_samples(&encoded, test_case.sample_rate)?;
                    assert_eq!(
                        normalized.len(),
                        decoded.len(),
                        "Length mismatch for {} test case",
                        test_case.name
                    );

                    let metrics = AudioQualityMetrics::calculate(&normalized, &decoded);

                    // Print metrics before assertions
                    println!("Quality metrics for {}:", test_case.name);
                    println!("  SNR: {:.1} dB", metrics.snr);
                    println!("  MSE: {:.6}", metrics.mse);
                    println!("  Max error: {:.6}", metrics.max_abs_error);
                    println!("  Correlation: {:.6}", metrics.correlation);

                    if #is_lossy {
                        let (min_snr, min_correlation) = match test_case.category {
                            AudioCategory::Noise => (10.0, 0.7),  // Noise is harder to encode
                            AudioCategory::Speech => (20.0, 0.95),
                            AudioCategory::Music => (25.0, 0.98),
                            _ => (15.0, 0.9),
                        };

                        assert!(
                            metrics.snr >= min_snr,
                            "{}: SNR {:.1} dB below minimum {:.1} dB",
                            test_case.name, metrics.snr, min_snr
                        );

                        assert!(
                            metrics.correlation >= min_correlation,
                            "{}: Correlation {:.3} below minimum {:.3}",
                            test_case.name, metrics.correlation, min_correlation
                        );
                    } else {
                        // For lossless codecs, we care about bit-perfect reconstruction
                        // but need to account for floating-point precision limits
                        const EPSILON: f32 = 1e-6;

                        assert!(
                            metrics.max_abs_error < EPSILON,
                            "{}: Non-zero error in lossless codec: {}",
                            test_case.name, metrics.max_abs_error
                        );
                    }
                }

                Ok(())
            }

            #[test]
            fn test_performance() -> Result<()> {
                let codec = #codec_type::default();
                let test_suite = AudioTestSuite::load_default_cases()?;

                println!("\nTesting {} codec performance", #codec_name);

                let mut total_perf = Vec::new();

                for test_case in test_suite.iter() {
                    let start = std::time::Instant::now();
                    let encoded = codec.encode_samples(&test_case.samples, test_case.sample_rate)?;
                    let encode_duration = start.elapsed();

                    let start = std::time::Instant::now();
                    let _decoded = codec.decode_samples(&encoded, test_case.sample_rate)?;
                    let decode_duration = start.elapsed();

                    let audio_duration = std::time::Duration::from_secs_f32(
                        test_case.samples.len() as f32 / test_case.sample_rate as f32
                    );

                    let perf = CodecPerformance {
                        encode_speed: audio_duration.as_secs_f32() / encode_duration.as_secs_f32(),
                        decode_speed: audio_duration.as_secs_f32() / decode_duration.as_secs_f32(),
                        compression_ratio: (test_case.samples.len() * std::mem::size_of::<f32>()) as f32
                            / encoded.len() as f32,
                    };

                    total_perf.push(perf);
                }

                // Average the performance metrics
                let avg_perf = CodecPerformance {
                    encode_speed: total_perf.iter().map(|p| p.encode_speed).sum::<f32>()
                        / total_perf.len() as f32,
                    decode_speed: total_perf.iter().map(|p| p.decode_speed).sum::<f32>()
                        / total_perf.len() as f32,
                    compression_ratio: total_perf.iter().map(|p| p.compression_ratio).sum::<f32>()
                        / total_perf.len() as f32,
                };

                println!("\nPerformance metrics:");
                println!("Encode speed: {:.2}x realtime", avg_perf.encode_speed);
                println!("Decode speed: {:.2}x realtime", avg_perf.decode_speed);
                println!("Compression ratio: {:.2}:1", avg_perf.compression_ratio);

                assert!(avg_perf.encode_speed > 1.0,
                    "Encoding slower than realtime: {:.2}x", avg_perf.encode_speed);
                assert!(avg_perf.decode_speed > 1.0,
                    "Decoding slower than realtime: {:.2}x", avg_perf.decode_speed);

                Ok(())
            }
        }
    }
}
