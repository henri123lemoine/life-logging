use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident};

use crate::attr::{CodecAttributes, CodecType};

pub struct CodecImpl {
    name: Ident,
    attrs: CodecAttributes,
}

impl CodecImpl {
    pub fn new(input: &DeriveInput, attrs: CodecAttributes) -> syn::Result<Self> {
        Ok(CodecImpl {
            name: input.ident.clone(),
            attrs,
        })
    }

    pub fn into_token_stream(self) -> TokenStream {
        let struct_name = &self.name;
        let codec_name = &self.attrs.name;
        let mime_type = &self.attrs.mime_type;
        let extension = &self.attrs.extension;

        let (trait_impl, extra_impls) = match self.attrs.codec_type {
            CodecType::Lossy => self.generate_lossy_impl(),
            CodecType::Lossless => self.generate_lossless_impl(),
        };

        let encode_impl = self.generate_encode_impl();
        let decode_impl = self.generate_decode_impl();

        quote! {
            impl Codec for #struct_name {
                fn name(&self) -> &'static str {
                    #codec_name
                }

                fn mime_type(&self) -> &'static str {
                    #mime_type
                }

                fn extension(&self) -> &'static str {
                    #extension
                }

                #encode_impl

                #decode_impl
            }

            #trait_impl

            #extra_impls
        }
    }

    fn generate_lossy_impl(&self) -> (TokenStream, TokenStream) {
        let struct_name = &self.name;

        (
            quote! {
                impl LossyCodec for #struct_name {}
            },
            if let Some(ref program) = self.attrs.external_program {
                self.generate_external_program_impl(program)
            } else if self.attrs.requires_model {
                self.generate_ml_model_impl()
            } else {
                quote! {}
            },
        )
    }

    fn generate_lossless_impl(&self) -> (TokenStream, TokenStream) {
        let struct_name = &self.name;

        (
            quote! {
                impl LosslessCodec for #struct_name {}
            },
            if let Some(ref program) = self.attrs.external_program {
                self.generate_external_program_impl(program)
            } else {
                quote! {}
            },
        )
    }

    fn generate_encode_impl(&self) -> TokenStream {
        if self.attrs.external_program.is_some() {
            quote! {
                fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
                    self.encode_external(data, sample_rate)
                }
            }
        } else if self.attrs.requires_model {
            quote! {
                fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
                    self.encode_with_model(data, sample_rate)
                }
            }
        } else {
            quote! {
                fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>>;
            }
        }
    }

    fn generate_decode_impl(&self) -> TokenStream {
        if self.attrs.external_program.is_some() {
            quote! {
                fn decode(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
                    self.decode_external(data, sample_rate)
                }
            }
        } else if self.attrs.requires_model {
            quote! {
                fn decode(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
                    self.decode_with_model(data, sample_rate)
                }
            }
        } else {
            quote! {
                fn decode(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>>;
            }
        }
    }

    fn generate_external_program_impl(&self, program: &str) -> TokenStream {
        let struct_name = &self.name;
        quote! {
            impl #struct_name {
                fn encode_external(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
                    use std::process::{Command, Stdio};
                    use std::io::Write;
                    use tempfile::NamedTempFile;

                    let mut temp_wav = NamedTempFile::new()?;
                    // Convert to WAV first
                    let wav_data = WavCodec::default().encode(data, sample_rate)?;
                    temp_wav.write_all(&wav_data)?;

                    let output = Command::new(#program)
                        .arg(temp_wav.path())
                        .output()?;

                    if !output.status.success() {
                        return Err(Error::Codec(format!("{} encoding failed", #program)));
                    }

                    Ok(output.stdout)
                }

                fn decode_external(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
                    use std::process::{Command, Stdio};
                    use std::io::Write;
                    use tempfile::NamedTempFile;

                    let mut temp_input = NamedTempFile::new()?;
                    temp_input.write_all(data)?;

                    let output = Command::new(#program)
                        .arg("--decode")
                        .arg(temp_input.path())
                        .output()?;

                    if !output.status.success() {
                        return Err(Error::Codec(format!("{} decoding failed", #program)));
                    }

                    WavCodec::default().decode(&output.stdout, sample_rate)
                }
            }
        }
    }

    fn generate_ml_model_impl(&self) -> TokenStream {
        let struct_name = &self.name;
        quote! {
            impl #struct_name {
                fn encode_with_model(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
                    let model = self.model.lock().map_err(|_| Error::Codec("Failed to acquire model lock"))?;
                    // Model-specific encoding logic here
                    unimplemented!("ML model encoding not implemented")
                }

                fn decode_with_model(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
                    let model = self.model.lock().map_err(|_| Error::Codec("Failed to acquire model lock"))?;
                    // Model-specific decoding logic here
                    unimplemented!("ML model decoding not implemented")
                }
            }
        }
    }
}
