use rand::RngCore;
use std::ffi::CString;
use std::iter::zip;

pub struct PoorlyObfuscated {
    pub key: Vec<u8>,
    pub data: Vec<u8>,
}
impl PoorlyObfuscated {
    pub fn from_string(input: &String) -> PoorlyObfuscated {
        let mut key = vec![0; 8];
        rand::rng().fill_bytes(&mut key);
        Self::from_string_with_key(input, &key)
    }

    pub fn from_string_with_key(input: &String, random_key: &[u8]) -> PoorlyObfuscated {
        // todo: complete this function to actually obfuscate the string
        let data = CString::new(input.clone().into_bytes()).unwrap().to_bytes().to_vec();
        PoorlyObfuscated { key: vec![], data }
    }

    pub fn to_string(&self) -> CString {
        //todo: complete this function to deobfuscate your string
        CString::new(self.data.clone()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::iter::zip;

    #[test]
    fn test_string_conversion() {
        let input = String::from("hello");
        assert_eq!(
            PoorlyObfuscated::from_string(&input).to_string(),
            c"hello".into()
        );
    }
    #[test]
    fn test_long_string_conversion() {
        let input = String::from("hello but in a longer message");
        assert_eq!(
            PoorlyObfuscated::from_string(&input).to_string(),
            c"hello but in a longer message".into()
        );
    }
    #[test]
    fn test_obfuscation() {
        let input = String::from("some kinda long string");
        let in_data = input.as_bytes().to_vec();
        let obf = PoorlyObfuscated::from_string(&input);
        let out_data = obf.data;
        for pair in zip(&in_data, &out_data) {
            assert_ne!(pair.0, pair.1);
        }
    }
}
