use obfuscated_strings::PoorlyObfuscated;
use quote::__private::TokenStream;
use quote::quote;
use rand::RngCore;
use syn::parse_macro_input;
#[proc_macro]
pub fn obfuscate_str(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::LitStr);
    let str_value = input.value();
    let mut key = [0u8; 8];
    rand::rng().fill_bytes(&mut key);
    proc_macro::TokenStream::from(impl_replace_macro(str_value, &key))
}
fn impl_replace_macro(input: String, random_key: &[u8]) -> TokenStream {
    let obj = PoorlyObfuscated::from_string_with_key(&input, random_key);
    let random_key = obj.key;
    let data = obj.data;
    let out = quote! {
        PoorlyObfuscated{key: vec![#(#random_key,)*], data: vec![#(#data,)*]}.to_string()
    };
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codegen() {
        let key = [0u8; 8];
        let out = impl_replace_macro("hello".parse().unwrap(), &key);
        let dummy = "PoorlyObfuscated { key : vec ! [0u8 , 0u8 , 0u8 , 0u8 , 0u8 , 0u8 , 0u8 , 0u8 ,] , data : vec ! [104u8 , 101u8 , 108u8 , 108u8 , 111u8 ,] } . to_string ()";
        assert_eq!(out.to_string(), dummy.to_string());
        println!("{}", out);
    }
}
