use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Error, *,
};

const MAX_BITS: usize = 64;

fn parse_tokens(input: proc_macro::TokenStream) -> Result<TokenStream> {
    let input_copy: TokenStream = input.clone().into();
    let bitfield_struct = syn::parse::<BitfieldStruct>(input)?;
    let struct_vis = &bitfield_struct.vis;
    let name = &bitfield_struct.ident;
    let base_type = &bitfield_struct.base_type;
    let base_type_size = bitfield_struct.base_type_size;

    let expanded_struct = quote! {
        #struct_vis struct #name(#base_type);
    };

    let mut full_mask = ((1u64 << (base_type_size - 1)) << 1).wrapping_sub(1);
    let fns = bitfield_struct
        .fields
        .named
        .iter()
        .map(|f| {
            let vis = &f.vis;
            let name = f.ident.as_ref();
            let field_type = &f.used_type;
            let range = &f.range;
            let lo = range.lo;
            let hi = range.hi;

            let make_range_error = |message| {
                return Err(Error::new_spanned(range.to_token_stream(), message));
            };

            if field_type.to_token_stream().to_string() == "bool" && lo != hi {
                return make_range_error("Bitfield range is too large for a bool");
            }
            if range.range_limit.is_some() && lo == hi {
                return make_range_error(
                    "Bitfield range bounds cannot be the same. A range is not needed",
                );
            }
            if lo > hi {
                return make_range_error("Bitfield range bounds are invalid");
            }
            let range_diff = hi - lo;
            if lo >= base_type_size || hi >= base_type_size {
                return make_range_error("Bitfield range exceeds base type size");
            }

            let getter_ret = if range_diff == 0 {
                quote! { value != 0 }
            } else {
                quote! { value as #field_type }
            };
            let mask = ((1u64 << range_diff) << 1).wrapping_sub(1);

            let update_full_mask = mask << lo;
            if full_mask & update_full_mask != update_full_mask {
                return make_range_error("Bitfield range overlaps with another bitfield range");
            }
            full_mask &= !(update_full_mask);

            let set_name = if let Some(name) = name {
                format_ident!("set_{}", name)
            } else {
                return Ok(TokenStream::new());
            };

            Ok(quote! {
                #vis fn #name(&self) -> #field_type {
                    let mask = #mask as #base_type;
                    let value = (self.0 >> #lo) & mask;
                    #getter_ret
                }

                #vis fn #set_name(&mut self, value: #field_type) {
                    let value = value as #base_type;
                    let mask = #mask as #base_type;
                    self.0 = (self.0 & !(mask << #lo)) | ((value & mask) << #lo);
                }
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let byte_fns = (0..(base_type_size / 8)).map(|i| {
        let get_name = format_ident!("byte{}", i);
        let set_name = format_ident!("set_byte{}", i);
        quote! {
            #struct_vis fn #get_name(&self) -> u8 {
                (self.0 >> (8 * #i)) as u8
            }

            #struct_vis fn #set_name(&mut self, value: u8) {
                let shift = 8 * #i;
                let cleared = self.0 & !(0xFF << shift);
                let val_shifted = (value as #base_type) << (8 * #i);
                self.0 = cleared | val_shifted;
            }
        }
    });

    let expanded = quote! {
        #expanded_struct
        impl #name {
            pub fn new() -> Self {
                Self(0)
            }

            #(#fns)*
            #(#byte_fns)*
        }
    };

    if full_mask != 0 {
        return Err(Error::new_spanned(
            input_copy,
            "Bitfield must account for all bits in base type",
        ));
    }

    Ok(expanded)
}

#[proc_macro]
pub fn bitfield(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    parse_tokens(input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

struct BitfieldRange {
    pub lo: u8,
    pub hi: u8,
    pub lo_token: LitInt,
    pub range_limit: Option<Token![..=]>,
    pub hi_token: Option<LitInt>,
}

struct BitfieldField {
    pub vis: Visibility,
    pub ident: Option<Ident>,
    pub _colon_token: Token![:],
    pub used_type: Type,
    pub _range_sep_token: Token![@],
    pub range: BitfieldRange,
}

struct BitfieldFieldsNamed {
    pub _brace_token: token::Brace,
    pub named: Punctuated<BitfieldField, Token![,]>,
}

struct BitfieldStruct {
    pub vis: Visibility,
    pub _struct_token: Token![struct],
    pub ident: Ident,
    pub _colon_token: Token![:],
    pub base_type: syn::Type,
    pub base_type_size: u8,
    pub fields: BitfieldFieldsNamed,
    pub _semi_token: Option<Token![;]>,
}

impl Parse for BitfieldRange {
    fn parse(input: ParseStream) -> Result<Self> {
        let lo_token = input.parse::<LitInt>()?;
        let lo = lo_token.base10_parse()?;
        if let Ok(range_limit) = input.parse::<Token![..=]>() {
            let hi_token = input.parse::<LitInt>()?;
            let hi = hi_token.base10_parse()?;
            Ok(BitfieldRange {
                lo,
                hi,
                lo_token,
                range_limit: Some(range_limit),
                hi_token: Some(hi_token),
            })
        } else {
            Ok(BitfieldRange {
                lo,
                hi: lo,
                lo_token,
                range_limit: None,
                hi_token: None,
            })
        }
    }
}

impl ToTokens for BitfieldRange {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.lo_token.to_tokens(tokens);
        self.range_limit.to_tokens(tokens);
        self.hi_token.to_tokens(tokens);
    }
}

impl Parse for BitfieldField {
    fn parse(input: ParseStream) -> Result<Self> {
        let vis = input.parse()?;
        let ident = if let Ok(ident) = input.parse::<Ident>() {
            Some(ident)
        } else {
            input.parse::<Token![_]>()?;
            None
        };
        let _colon_token = input.parse()?;
        let used_type = input.parse()?;
        let _range_sep_token = input.parse()?;
        let range = input.parse()?;
        Ok(BitfieldField {
            vis,
            ident,
            _colon_token,
            used_type,
            _range_sep_token,
            range,
        })
    }
}

impl Parse for BitfieldFieldsNamed {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        Ok(BitfieldFieldsNamed {
            _brace_token: braced!(content in input),
            named: content.parse_terminated(BitfieldField::parse)?,
        })
    }
}

impl Parse for BitfieldStruct {
    fn parse(input: ParseStream) -> Result<Self> {
        let vis = input.parse()?;
        let _struct_token = input.parse()?;
        let ident = input.parse()?;
        let _colon_token = input.parse()?;
        let base_type: Type = input.parse()?;
        let base_type_str = base_type.to_token_stream().to_string();
        if base_type_str.as_bytes()[0] != 'u' as u8 {
            return Err(syn::Error::new(
                base_type.span(),
                "Bitfield base type must be an unsigned integral type",
            ));
        }
        let base_type_size = match &base_type_str[1..].parse::<u8>() {
            Ok(i) => *i,
            Err(_) => {
                return Err(syn::Error::new(
                    base_type.span(),
                    "Bitfield base type must be an unsigned integral type",
                ))
            }
        };
        if base_type_size as usize > MAX_BITS {
            return Err(syn::Error::new(
                base_type.span(),
                format!(
                    "Bitfield base type size can only be up to {} bits",
                    MAX_BITS
                ),
            ));
        }
        let fields = input.parse()?;
        let _semi_token = input.parse()?;
        Ok(BitfieldStruct {
            vis,
            _struct_token,
            ident,
            _colon_token,
            base_type,
            base_type_size,
            fields,
            _semi_token,
        })
    }
}
