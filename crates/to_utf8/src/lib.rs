use std::borrow::Cow;
use std::ptr;
mod tables_gen;

#[derive(Clone, Copy)]
pub enum FromType {
    Windows1250,
    Windows1251,
    Windows1252,
    Cp437,
}

#[derive(Clone, Copy)]
pub enum BufferAllocationPolicy {
    FitToRequiredSize,
    UseGrowFactor,
}

const ENTRY_SIZE: usize = 6;

pub struct StatelessUtf8Encoder {
    translation_array: &'static [i8],
    from_type: FromType,
}

impl StatelessUtf8Encoder {
    pub fn new(source_encoding: FromType) -> Self {
        let translation_array = match source_encoding {
            FromType::Windows1250 => &tables_gen::WINDOWS_1250[..],
            FromType::Windows1251 => &tables_gen::WINDOWS_1251[..],
            FromType::Windows1252 => &tables_gen::WINDOWS_1252[..],
            FromType::Cp437 => &tables_gen::CP437[..],
        };
        StatelessUtf8Encoder {
            translation_array,
            from_type: source_encoding,
        }
    }
    pub fn from_type(&self) -> FromType {
        self.from_type
    }

    pub fn get_utf8<'a>(
        &self,
        input: &'a [u8],
        _buffer_allocation_policy: BufferAllocationPolicy,
        buffer: &'a mut Vec<u8>,
    ) -> Cow<'a, str> {
        if input.is_ascii() {
            // Ascii is safe to convert straight to utf8
            return Cow::Borrowed(unsafe { std::str::from_utf8_unchecked(input) });
        }
        let mut required_size = 0;
        for &byte in input {
            let offset = (byte as usize) * ENTRY_SIZE;
            let len = self.translation_array[offset] as usize;
            required_size += len;
        }

        if buffer.len() < required_size {
            buffer.resize(required_size, 0);
        }

        let mut current_pos = 0;
        for &byte in input {
            let offset = (byte as usize) * ENTRY_SIZE;
            let len = self.translation_array[offset] as usize;

            unsafe {
                ptr::copy_nonoverlapping(
                    self.translation_array.as_ptr().add(offset + 1) as *const u8,
                    buffer.as_mut_ptr().add(current_pos),
                    len,
                );
            }
            current_pos += len;
        }

        buffer.truncate(current_pos);
        Cow::Borrowed(unsafe { std::str::from_utf8_unchecked(buffer) })
    }

    pub fn get_legacy_enc<'a>(
        &self,
        input: &'a str,
        _buffer_allocation_policy: BufferAllocationPolicy,
        buffer: &'a mut Vec<u8>,
    ) -> Cow<'a, [u8]> {
        if input.is_ascii() {
            return Cow::Borrowed(input.as_bytes());
        }

        buffer.clear();
        let input_bytes = input.as_bytes();
        let mut i = 0;
        while i < input_bytes.len() {
            let mut found = false;
            let mut char_len = 1;

            if input_bytes[i] & 0b10000000 != 0 {
                if input_bytes[i] & 0b11100000 == 0b11000000 {
                    char_len = 2;
                } else if input_bytes[i] & 0b11110000 == 0b11100000 {
                    char_len = 3;
                } else if input_bytes[i] & 0b11111000 == 0b11110000 {
                    char_len = 4;
                }
            }

            let current_char_bytes = &input_bytes[i..i + char_len];

            for j in 0..256 {
                let offset = j * ENTRY_SIZE;
                let len = self.translation_array[offset] as usize;

                if len == char_len {
                    let mut is_match = true;
                    for k in 0..len {
                        if self.translation_array[offset + 1 + k] != current_char_bytes[k] as i8 {
                            is_match = false;
                            break;
                        }
                    }
                    if is_match {
                        buffer.push(j as u8);
                        found = true;
                        break;
                    }
                }
            }

            if !found {
                buffer.push(b' ');
            }

            i += char_len;
        }
        Cow::Borrowed(buffer)
    }
}

pub struct Utf8Encoder {
    stateless_encoder: StatelessUtf8Encoder,
    buffer: Vec<u8>,
}

impl Utf8Encoder {
    pub fn new(source_encoding: FromType) -> Self {
        Utf8Encoder {
            buffer: Vec::new(),
            stateless_encoder: StatelessUtf8Encoder::new(source_encoding),
        }
    }

    pub fn get_utf8<'a>(&'a mut self, input: &'a [u8]) -> &'a str {
        if input.is_ascii() {
            return unsafe { std::str::from_utf8_unchecked(input) };
        }

        self.stateless_encoder.get_utf8(
            input,
            BufferAllocationPolicy::UseGrowFactor,
            &mut self.buffer,
        );

        unsafe { std::str::from_utf8_unchecked(&self.buffer) }
    }
    pub fn get_legacy_enc<'a>(&'a mut self, input: &'a str) -> Cow<'a, [u8]> {
        self.stateless_encoder.get_legacy_enc(
            input,
            BufferAllocationPolicy::UseGrowFactor,
            &mut self.buffer,
        )
    }
    pub fn get_stateless_encoder(&self) -> &StatelessUtf8Encoder {
        &self.stateless_encoder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_1252_to_utf8() {
        let mut encoder = Utf8Encoder::new(FromType::Windows1252);

        // Test case 1: ASCII-only byte slice should be unchanged
        let input_ascii = b"Hello, world!";
        let utf8_output = encoder.get_utf8(input_ascii);
        assert_eq!(utf8_output, "Hello, world!");

        // Test case 2: Byte slice with a non-ASCII character
        // The WINDOWS-1252 byte for the euro sign (€) is 0x80.
        let input_euro_sign = b"Euro sign: \x80";
        let utf8_output = encoder.get_utf8(input_euro_sign);
        assert_eq!(utf8_output, "Euro sign: €");

        // Test case 3: Byte slice with multiple non-ASCII characters
        let input_multi = b"A \x80 is worth \x80.";
        let utf8_output = encoder.get_utf8(input_multi);
        assert_eq!(utf8_output, "A € is worth €.");
    }
    #[test]
    fn test_utf8_to_windows_1252() {
        let mut encoder = Utf8Encoder::new(FromType::Windows1252);

        let input_utf8 = "Euro sign: €";
        let legacy_output_bytes = encoder.get_legacy_enc(input_utf8);
        assert_eq!(legacy_output_bytes, Cow::Borrowed(b"Euro sign: \x80"));

        let input_ascii = "Hello, world!";
        let legacy_output_bytes = encoder.get_legacy_enc(input_ascii);
        assert_eq!(legacy_output_bytes, Cow::Borrowed(b"Hello, world!"));
    }
}
