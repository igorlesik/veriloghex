//! This is a library for reading and parsing Verilog hex files
//! obtained with `objcopy -O verilog`.
//!
//! Inspired by https://github.com/martinmroz/ihex/blob/master/src/reader.rs
//!
//! # Iterating over bytes example:
//!
//! ```ignore
//! static TEXT_STR: &str = r#"
//! @81000000
//! 09 A0 F3 22 20 34 63 84 02 00 6F 00 E0 57 81 40
//! 01 41 81 41 01 42 81 42 01 43 81 43 01 44 81 44"#;
//!
//! let reader = crate::Reader::new(TEXT_STR);
//! for data in reader {
//!     std::println!("{}", data.unwrap());
//! }
//! ```
//!
//! Output:
//! ```ignore
//! new address: 0x81000000
//! 0x81000000: 09
//! 0x81000001: A0
//! 0x81000002: F3
//! 0x81000003: 22
//! ```
//!
//! # Grouping bytes example:
//!
//! ```ignore
//! static TEXT_STR: &str = r#"
//! @81000000
//! 09 A0 F3 22 20 34 63 84 02 00 6F 00 E0 57 81 40
//! 01 41 81 41 01 42 81 42 01 43 81 43 01 44 81 44"#;
//!
//! let reader = crate::Reader::new_with_options(TEXT_STR, crate::ReaderOptions { group: true });
//! for data in reader {
//!     std::println!("{}", data.unwrap());
//! }
//! ```
//!
//! Output:
//! ```ignore
//! new address: 0x81000000
//! 0x81000000: 8463342022F3A009
//! 0x81000008: 408157E0006F0002
//! 0x81000010: 4281420141814101
//! ```

#![no_std]

#[cfg(feature = "std")]
extern crate std;

use core::error::Error;
use core::fmt;
use core::str;

type Addr = u64;

/// Bytes in a line are grouped into N groups of M bytes each.
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DataType {
    U8(u8),
    U16(u16),
    U24(u32),
    U32(u32),
    U40(u64),
    U48(u64),
    U56(u64),
    U64(u64),
}

/// Syntax token type.
#[derive(Debug, PartialEq)]
pub enum Record {
    Data {
        addr: Addr,
        value: DataType,
    },
    EndOfFile,
    Comment,

    /// Example: @81000000
    NewAddress(Addr),
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Record::EndOfFile => write!(f, "EOF"),
            Record::Comment => write!(f, "comment"),
            Record::NewAddress(addr) => write!(f, "new address: {:#010X}", addr),
            Record::Data { addr, value } => {
                write!(
                    f,
                    "{:#010X}: {:02X}",
                    addr,
                    match value {
                        DataType::U8(value) => u64::from(*value),
                        DataType::U16(value) => u64::from(*value),
                        DataType::U24(value) => u64::from(*value),
                        DataType::U32(value) => u64::from(*value),
                        DataType::U40(value) => *value,
                        DataType::U48(value) => *value,
                        DataType::U56(value) => *value,
                        DataType::U64(value) => *value,
                    }
                )
            }
        }
    }
}

/// Custom simple error type.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ReaderError {
    /// Failed to parse tokens.
    InvalidSyntax,
    /// Can't convert string to number.
    BadNumberConversion,
}

impl fmt::Display for ReaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ReaderError::InvalidSyntax => write!(f, "invalid format"),
            ReaderError::BadNumberConversion => write!(f, "cant convert string to number"),
        }
    }
}

impl Error for ReaderError {}

impl Record {
    /// Constructs a new [`Record`] by parsing `string`.
    pub fn from_string(string: &str, current_addr: Addr) -> Result<Self, ReaderError> {
        if string.is_empty() {
            return Err(ReaderError::InvalidSyntax);
        }

        if string.starts_with("//") {
            return Ok(Record::Comment);
        }

        if let Some(stripped_string) = string.strip_prefix('@') {
            if let Ok(value) = u64::from_str_radix(stripped_string, 16) {
                return Ok(Record::NewAddress(value));
            } else {
                return Err(ReaderError::BadNumberConversion);
            }
        }

        if let Ok(value) = u8::from_str_radix(string, 16) {
            Ok(Record::Data {
                addr: current_addr,
                value: DataType::U8(value),
            })
        } else {
            Err(ReaderError::BadNumberConversion)
        }
    }
}

/// Configuration options for the reader.
#[derive(Default)]
pub struct ReaderOptions {
    /// Group bytes into 2..8 bytes.
    pub group: bool,
}

/* Can be derived so far
impl Default for ReaderOptions {
    fn default() -> Self {
        ReaderOptions { group: false }
    }
}*/

/// A reader for Verilog hex files.
///
/// Example:
///
/// ```ignore
/// let reader = crate::Reader::new(TEXT_STR);
/// for data in reader {
///     std::println!("{}", data.unwrap());
/// }
/// let mut reader = crate::Reader::new(TEXT_STR);
/// assert_eq!(
///     reader.nth(1),
///     Some(Ok(Record::Data {
///         addr: 0x81000000,
///         value: DataType::U8(0x09u8)
///     }))
/// );
/// ```
pub struct Reader<'a> {
    /// Iterator over tokens.
    token_iterator: core::iter::Peekable<str::SplitAsciiWhitespace<'a>>,
    /// Reading may complete earlier.
    finished: bool,
    /// Configuration options.
    #[allow(dead_code)]
    options: ReaderOptions,
    /// Current address.
    current_addr: Addr,
}

impl<'a> Reader<'a> {
    /// Create a new reader with the specified options.
    pub fn new_with_options(string: &'a str, options: ReaderOptions) -> Self {
        Reader {
            token_iterator: string.split_ascii_whitespace().peekable(), // whitespaces + newlines
            finished: false,
            options,
            current_addr: 0,
        }
    }

    /// Create a new reader with default options.
    pub fn new(string: &'a str) -> Self {
        Reader::new_with_options(string, Default::default())
    }

    /// Private helper method for obtaining the next record string.
    /// Does not respect the 'finished' flag.
    /// It will return either the next record string to be read, or None if nothing is left to process.
    fn next_record(&mut self) -> Option<&'a str> {
        self.token_iterator
            .by_ref()
            .find(|&token| !token.is_empty())
    }
}

impl<'a> Iterator for Reader<'a> {
    type Item = Result<Record, ReaderError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        match self.next_record() {
            None => {
                self.finished = true;
                None
            }

            Some(token) => {
                let mut parse_result = Record::from_string(token, self.current_addr);

                if parse_result.is_err() {
                    self.finished = true;
                }

                if let Ok(Record::EndOfFile) = parse_result {
                    self.finished = true;
                }

                if let Ok(Record::NewAddress(new_addr)) = parse_result {
                    self.current_addr = new_addr;
                } else if let Ok(Record::Data { addr: _, value: _ }) = parse_result {
                    self.current_addr += 1;
                }

                if self.options.group && !self.finished {
                    while let Ok(Record::Data { addr, value }) = parse_result {
                        if matches!(value, DataType::U64(_)) {
                            break;
                        }
                        let start_addr = addr;
                        if let Some(next_token) = self.token_iterator.peek() {
                            let next_result = Record::from_string(next_token, self.current_addr);
                            if let Ok(Record::Data {
                                addr: _next_addr,
                                value: next_value,
                            }) = next_result
                                && let DataType::U8(next_value_u8) = next_value
                            {
                                parse_result = Ok(Record::Data {
                                    addr: start_addr,
                                    value: group_new_data(value, next_value_u8),
                                });
                                self.current_addr += 1;
                                self.token_iterator.next();
                                continue;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }

                Some(parse_result)
            }
        }
    }
}

fn group_new_data(value: DataType, next_value_u8: u8) -> DataType {
    match value {
        DataType::U8(value_u8) => {
            DataType::U16(u16::from(value_u8) | (u16::from(next_value_u8) << 8))
        }
        DataType::U16(value_u16) => {
            DataType::U24(u32::from(value_u16) | (u32::from(next_value_u8) << 16))
        }
        DataType::U24(value_u24) => DataType::U32(value_u24 | (u32::from(next_value_u8) << 24)),
        DataType::U32(value_u32) => {
            DataType::U40(u64::from(value_u32) | (u64::from(next_value_u8) << 32))
        }
        DataType::U40(value_u40) => DataType::U48(value_u40 | (u64::from(next_value_u8) << 40)),
        DataType::U48(value_u48) => DataType::U56(value_u48 | (u64::from(next_value_u8) << 48)),
        DataType::U56(value_u56) => DataType::U64(value_u56 | (u64::from(next_value_u8) << 56)),
        DataType::U64(value_u64) => DataType::U64(value_u64),
    }
}

//impl<'a> FusedIterator for Reader<'a> {}

#[cfg(feature = "std")]
pub fn read_file(filepath: &str) -> Option<std::string::String> {
    use std::io::Read;
    if let Ok(mut file) = std::fs::File::open(filepath) {
        let mut contents = std::string::String::new();
        if file.read_to_string(&mut contents).is_ok() {
            Some(contents)
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(test)]
static TEXT_STR: &str = r#"
@81000000
09 A0 F3 22 20 34 63 84 02 00 6F 00 E0 57 81 40
01 41 81 41 01 42 81 42 01 43 81 43 01 44 81 44
01 45 81 45 01 46 81 46 01 47 81 47 01 48 81 48
01 49 81 49 01 4A 81 4A 01 4B 81 4B 01 4C 81 4C
01 4D 81 4D 01 4E 81 4E 01 4F 81 4F 97 11 08 00
93 81 C1 85 97 02 0F 00 93 82 C2 FA 16 81 97 12
00 00 93 82 22 8E 73 90 52 30 97 00 00 00 E7 80
20 4B 97 00 00 00 E7 80 20 4C 01 A0 82 80 00 00
@81000080
79 71 22 F4 00 18 AA 87 2E 87 23 2E F4 FC BA 87
23 2C F4 FC 83 27 84 FD FD 8B 23 26 F4 FE 03 27
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read() {
        let reader = crate::Reader::new(TEXT_STR);
        for _data in reader {
            #[cfg(feature = "std")]
            std::println!("{}", _data.unwrap());
        }
        let mut reader = crate::Reader::new(TEXT_STR);
        assert_eq!(
            reader.nth(1),
            Some(Ok(Record::Data {
                addr: 0x81000000,
                value: DataType::U8(0x09u8)
            }))
        );
        assert_eq!(
            reader.nth(1), // took 2 before, skip 1, this is 3rd
            Some(Ok(Record::Data {
                addr: 0x81000002,
                value: DataType::U8(0xF3u8)
            }))
        );
    }

    #[test]
    fn test_read_group() {
        let reader =
            crate::Reader::new_with_options(TEXT_STR, crate::ReaderOptions { group: true });
        for _data in reader {
            #[cfg(feature = "std")]
            std::println!("{}", _data.unwrap());
        }
    }
}
