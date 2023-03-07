//! # serde_canonical_json
//! 
//! This crate provides a [Canonical JSON](https://wiki.laptop.org/go/Canonical_JSON) formatter for serde_json.
//! 
//! ## Usage
//! 
//! ```rust
//! use serde::Serialize;
//! use serde_json::Serializer;
//! use serde_canonical_json::CanonicalFormatter;
//! 
//! // CanonicalFormatter will ensure our keys are in lexical order
//! #[derive(Serialize)]
//! struct Data
//! {
//!     c: isize,
//!     b: bool,
//!     a: String,
//! }
//! 
//! let data = Data { c: 120, b: false, a: "Hello!".to_owned() };
//! 
//! let mut ser = Serializer::with_formatter(Vec::new(), CanonicalFormatter::new());
//! 
//! data.serialize(&mut ser).unwrap();
//! 
//! let json = String::from_utf8(ser.into_inner()).unwrap();
//! 
//! assert_eq!(json, r#"{"a":"Hello!","b":false,"c":120}"#);

use std::{io::{self, ErrorKind, Error}, collections::VecDeque};
use serde_json::ser::Formatter;
use lazy_static::lazy_static;
use regex::Regex;


#[derive(Default)]
pub struct CanonicalFormatter
{
    object_stack: VecDeque<ObjectStackFrame>,
}


impl CanonicalFormatter
{
    pub fn new() -> Self
    {
        Self { object_stack: VecDeque::new() }
    }
}


struct ObjectStackFrame
{
    members: Vec<ObjectMemberBuffer>,
}


impl ObjectStackFrame
{
    fn new() -> Self { Self { members: Vec::new() } }


    fn push_member(&mut self)
    {
        self.members.push(ObjectMemberBuffer::new())
    }


    fn current_member(&mut self) -> Option<&mut ObjectMemberBuffer>
    {
        self.members.last_mut()
    }


    fn string(&mut self) -> String
    {
        let mut output = "{".to_owned();

        self.members.sort_by(|a, b| a.key.cmp(&b.key));

        for (index, member) in self.members.iter_mut().enumerate()
        {
            output.push_str(&member.string(index == 0));
        }

        output.push('}');

        output
    }
}


struct ObjectMemberBuffer
{
    key: String,
    value: String,
    key_finished: bool,
}


impl ObjectMemberBuffer
{
    fn new() -> Self
    {
        Self { key: String::new(), value: String::new(), key_finished: false }
    }


    fn push(&mut self, ch: char)
    {
        if self.key_finished
        {
            self.value.push(ch);
        }
        else
        {
            self.key.push(ch);
        }
    }


    fn push_str(&mut self, str: &str)
    {
        if self.key_finished
        {
            self.value.push_str(str);
        }
        else
        {
            self.key.push_str(str);
        }
    }


    fn finish_key(&mut self)
    {
        self.key_finished = true
    }


    fn string(&self, first: bool) -> String
    {
        let prefix = if first
        {
            ""
        }
        else
        {
            ","
        };

        format!("{}{}:{}", prefix, &self.key, &self.value)
    }
}


impl CanonicalFormatter
{
    fn push_object(&mut self)
    {
        self.object_stack.push_front(ObjectStackFrame::new())
    }


    fn current_object(&mut self) -> Option<&mut ObjectStackFrame>
    {
        self.object_stack.front_mut()
    }


    fn pop_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()>
    {
        let Some(mut object) = self.object_stack.pop_front() else
        {
            return Err(Error::new(ErrorKind::InvalidData, "Object requested when object is not active."))
        };

        let string = object.string();

        // Check to see if this was the top of the stack
        if let Some(parent) = self.current_object()
        {
            let Some(member) = parent.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(&string);

            Ok(())
        }
        else
        {
            writer.write_all(string.as_bytes())
        }
    }
}


impl Formatter for CanonicalFormatter
{
    fn write_f32<W>(&mut self, _writer: &mut W, _value: f32) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        Err(Error::new(ErrorKind::InvalidData, "Floating point numbers are forbidden."))
    }


    fn write_f64<W>(&mut self, _writer: &mut W, _value: f64) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        Err(Error::new(ErrorKind::InvalidData, "Floating point numbers are forbidden."))
    }


    fn write_number_str<W>(&mut self, writer: &mut W, value: &str) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        // Numbers are allowed to be of the form:
        // digit
        // digit1-9 digits
        // - digit1-9
        // - digit1-9 digits

        lazy_static!
        {
            static ref RE: Regex = Regex::new(r"^\d$|^-[1-9]$|^-?[1-9]\d+$").unwrap();
        }

        if RE.is_match(value)
        {
            if let Some(object) = self.current_object()
            {
                let Some(member) = object.current_member() else
                {
                    return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
                };
    
                member.push_str(value);
                Ok(())
            }
            else
            {
                writer.write_all(value.as_bytes())
            }
        }
        else
        {
            Err(Error::new(ErrorKind::InvalidData, "Number string in invalid format."))
        }
    }


    fn write_char_escape<W>(&mut self, writer: &mut W, char_escape: serde_json::ser::CharEscape) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        use serde_json::ser::CharEscape::*;

        // Only permitted escape values are for " and \
        // Everything else passed through verbatim

        let s = match char_escape {
            Quote => "\\\"",
            ReverseSolidus => "\\\\",
            Solidus => "/",
            Backspace => "\x08",
            FormFeed => "\x0C",
            LineFeed => "\n",
            CarriageReturn => "\r",
            Tab => "\t",
            AsciiControl(byte) =>
            {
                if let Some(object) = self.current_object()
                {
                    let Some(member) = object.current_member() else
                    {
                        return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
                    };
        
                    member.push(byte as char);
                    return Ok(())
                }
                else
                {
                    return writer.write_all(&[byte])
                }
            }
        };

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }


    fn begin_object<W>(&mut self, _writer: &mut W) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        self.push_object();

        Ok(())
    }


    fn begin_object_key<W>(&mut self, _writer: &mut W, _first: bool) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        let Some(object) = self.current_object() else
        {
            return Err(Error::new(ErrorKind::InvalidData, "Object key requested when object is not active."))
        };

        object.push_member();
        Ok(())
    }


    fn begin_string<W>(&mut self, writer: &mut W) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str("\"");
            Ok(())
        }
        else
        {
            writer.write_all(b"\"")
        }
    }


    fn write_string_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(fragment);
            Ok(())
        }
        else
        {
            writer.write_all(fragment.as_bytes())
        }
    }


    fn end_string<W>(&mut self, writer: &mut W) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str("\"");
            Ok(())
        }
        else
        {
            writer.write_all(b"\"")
        }
    }


    fn end_object_key<W>(&mut self, _writer: &mut W) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        let Some(object) = self.current_object() else
        {
            return Err(Error::new(ErrorKind::InvalidData, "Object key requested when object is not active."))
        };
        let Some(member) = object.current_member() else
        {
            return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
        };

        member.finish_key();
        Ok(())
    }


    fn begin_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        Ok(())
    }


    fn end_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        Ok(())
    }


    fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
        where
            W: ?Sized + io::Write,
    {
        self.pop_object(writer)
    }
    
    /// Writes a `null` value to the specified writer.
    #[inline]
    fn write_null<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str("null");
            Ok(())
        }
        else
        {
            writer.write_all(b"null")
        }
    }

    /// Writes a `true` or `false` value to the specified writer.
    #[inline]
    fn write_bool<W>(&mut self, writer: &mut W, value: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let s = if value {
            "true"
        } else {
            "false"
        };

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i8<W>(&mut self, writer: &mut W, value: i8) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i16<W>(&mut self, writer: &mut W, value: i16) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i32<W>(&mut self, writer: &mut W, value: i32) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i64<W>(&mut self, writer: &mut W, value: i64) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }

    /// Writes an integer value like `-123` to the specified writer.
    #[inline]
    fn write_i128<W>(&mut self, writer: &mut W, value: i128) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u8<W>(&mut self, writer: &mut W, value: u8) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u16<W>(&mut self, writer: &mut W, value: u16) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u32<W>(&mut self, writer: &mut W, value: u32) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u64<W>(&mut self, writer: &mut W, value: u64) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }

    /// Writes an integer value like `123` to the specified writer.
    #[inline]
    fn write_u128<W>(&mut self, writer: &mut W, value: u128) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let mut buffer = itoa::Buffer::new();
        let s = buffer.format(value);

        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(s);
            Ok(())
        }
        else
        {
            writer.write_all(s.as_bytes())
        }
    }

    /// Called before every array.  Writes a `[` to the specified
    /// writer.
    #[inline]
    fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str("[");
            Ok(())
        }
        else
        {
            writer.write_all(b"[")
        }
    }

    /// Called after every array.  Writes a `]` to the specified
    /// writer.
    #[inline]
    fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str("]");
            Ok(())
        }
        else
        {
            writer.write_all(b"]")
        }
    }

    /// Called before every array value.  Writes a `,` if needed to
    /// the specified writer.
    #[inline]
    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if first
        {
            Ok(())
        }
        else if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(",");
            Ok(())
        }
        else
        {
            writer.write_all(b",")
        }
    }

    /// Called after every array value.
    #[inline]
    fn end_array_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        Ok(())
    }

    /// Writes a raw JSON fragment that doesn't need any escaping to the
    /// specified writer.
    #[inline]
    fn write_raw_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if let Some(object) = self.current_object()
        {
            let Some(member) = object.current_member() else
            {
                return Err(Error::new(ErrorKind::InvalidData, "Object member requested when member is not active."))
            };

            member.push_str(fragment);
            Ok(())
        }
        else
        {
            writer.write_all(fragment.as_bytes())
        }
    }
}


#[cfg(test)]
mod tests;