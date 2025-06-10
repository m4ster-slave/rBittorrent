use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
}
impl Error for ParseError {}
impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ParseError: {}", self.message)
    }
}
impl ParseError {
    pub fn new(message: &str) -> Self {
        ParseError {
            message: message.to_string(),
        }
    }
}

pub fn parse_value(input: &str) -> Result<String, ParseError> {
    // if the value is a digit is is a bencoded string
    if input.chars().next().unwrap().is_ascii_digit() {
        let colon_pos = input
            .find(':')
            .ok_or_else(|| ParseError::new("Missing colon in string encoding"))?;

        let enc_str_len = &input[..colon_pos]
            .parse::<usize>()
            .map_err(|_| ParseError::new("Invalid string length"))?;

        if colon_pos + 1 + enc_str_len > input.len() {
            return Err(ParseError::new("String length exceeds input"));
        }

        let output = &input[colon_pos + 1..colon_pos + 1 + enc_str_len];
        // consider using serde for this but will try to serialize manually for now
        Ok(format!("\"{}\"", output))
    } else if input.starts_with('i') {
        let end_pos = input
            .find('e')
            .ok_or_else(|| ParseError::new("Missing 'e' in integer encoding"))?;

        let output = &input[1..end_pos];

        // This is what the official spec says so we are gonna return
        // them as errors:
        // i-0e is invalid. All encodings with a leading zero, such as
        // i03e, are invalid, other than i0e, which of course corresponds
        // to 0.
        if output.starts_with("-0") {
            return Err(ParseError::new("Invalid encoding: i-0e"));
        } else if output.starts_with("0") && output != "0" {
            return Err(ParseError::new(
                "Invalid encoding, leading zeros not allowed",
            ));
        }

        Ok(output.to_string())
    } else if input.starts_with('l') {
        unimplemented!();
    } else {
        unimplemented!();
    }
}

#[cfg(test)]
mod test {
    use crate::parser::parse_value;

    #[test]
    fn test_parse_string() {
        assert_eq!(parse_value("5:hello").unwrap(), "\"hello\"");
        assert_eq!(parse_value("9:iloverust").unwrap(), "\"iloverust\"");
        // chars afer the length specified
        assert_eq!(
            parse_value("9:iloverustandneovim").unwrap(),
            "\"iloverust\""
        );
    }

    #[test]
    fn test_parse_integer() {
        // postive number
        assert_eq!(parse_value("i69e").unwrap(), "69");
        // negative number
        assert_eq!(parse_value("i-69e").unwrap(), "-69");
        // chars after value
        assert_eq!(parse_value("i69esdlkfj").unwrap(), "69");
        assert_eq!(parse_value("i69eeee").unwrap(), "69");

        // -0 is invalid
        assert!(parse_value("i-0e").is_err());
        // leading 0 is invalid
        assert!(parse_value("i05e").is_err());
        // both is invalid too
        assert!(parse_value("i-05e").is_err());

        // just a zero is just a zero
        assert_eq!(parse_value("i0e").unwrap(), "0");
    }
}
