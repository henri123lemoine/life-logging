use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Attribute, Data, DeriveInput, Field, Fields};

#[derive(Debug)]
pub struct CodecAttributes {
    pub name: String,
    pub mime_type: String,
    pub extension: String,
    pub codec_type: CodecType,
    pub params: Vec<CodecParam>,
    pub external_program: Option<String>,
    pub requires_model: bool,
}

#[derive(Debug)]
pub enum CodecType {
    Lossy,
    Lossless,
}

#[derive(Debug)]
pub struct CodecParam {
    pub field_name: String,
    pub param_type: syn::Type,
    pub default_value: Option<syn::Expr>,
}

impl CodecAttributes {
    pub fn from_derive_input(input: &DeriveInput) -> syn::Result<Self> {
        let mut name = None;
        let mut mime_type = None;
        let mut extension = None;
        let mut codec_type = None;
        let mut params = Vec::new();
        let mut external_program = None;
        let mut requires_model = false;

        for attr in &input.attrs {
            if !attr.path().is_ident("codec") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    name = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("mime") {
                    mime_type = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("extension") {
                    extension = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("lossy") {
                    codec_type = Some(CodecType::Lossy);
                } else if meta.path.is_ident("lossless") {
                    codec_type = Some(CodecType::Lossless);
                } else if meta.path.is_ident("external_program") {
                    external_program = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("requires_model") {
                    requires_model = true;
                }
                Ok(())
            })?;
        }

        if let Data::Struct(data_struct) = &input.data {
            if let Fields::Named(fields) = &data_struct.fields {
                for field in &fields.named {
                    if let Some(param) = Self::parse_field_attributes(field)? {
                        params.push(param);
                    }
                }
            }
        }

        Ok(CodecAttributes {
            name: name.ok_or_else(|| syn::Error::new(Span::call_site(), "Missing codec name"))?,
            mime_type: mime_type
                .ok_or_else(|| syn::Error::new(Span::call_site(), "Missing mime type"))?,
            extension: extension
                .ok_or_else(|| syn::Error::new(Span::call_site(), "Missing extension"))?,
            codec_type: codec_type.ok_or_else(|| {
                syn::Error::new(Span::call_site(), "Must specify either lossy or lossless")
            })?,
            params,
            external_program,
            requires_model,
        })
    }

    fn parse_field_attributes(field: &Field) -> syn::Result<Option<CodecParam>> {
        let mut is_param = false;
        let mut default_value = None;

        for attr in &field.attrs {
            if !attr.path().is_ident("codec") {
                continue;
            }

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("param") {
                    is_param = true;
                    if let Ok(value) = meta.value() {
                        default_value = Some(value.parse()?);
                    }
                }
                Ok(())
            })?;
        }

        if is_param {
            Ok(Some(CodecParam {
                field_name: field.ident.as_ref().unwrap().to_string(),
                param_type: field.ty.clone(),
                default_value,
            }))
        } else {
            Ok(None)
        }
    }
}
