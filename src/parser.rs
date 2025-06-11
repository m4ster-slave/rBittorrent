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
    match input.chars().next().unwrap() {
        '0'..='9' => {
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
        }
        'i' => {
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
        }
        'l' => {
            let mut pos = 1;
            let mut elements = Vec::new();

            while pos < input.len() {
                // end of list
                if input.chars().nth(pos).unwrap() == 'e' {
                    let result = format!("[{}]", elements.join(","));
                    return Ok(result);
                }

                // find the end of next element to parse it
                let element_end = find_element_end(&input[pos..])?;
                let element_input = &input[pos..pos + element_end];

                // recursively parse the element
                let parsed_element = parse_value(element_input)?;
                elements.push(parsed_element);
                pos += element_end;
            }

            Err(ParseError::new("Missing 'e' at end of list"))
        }
        'd' => {
            let mut pos = 1;
            let mut elements = Vec::new();

            while pos < input.len() {
                // end of this dict
                if input.chars().nth(pos).unwrap() == 'e' {
                    let result = format!(
                        "{{{}}}",
                        elements
                            .iter()
                            .map(|(left, right)| format!("{left}:{right}"))
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                    return Ok(result);
                }

                // parse the left and right element
                let element_end = find_element_end(&input[pos..])?;
                let element_input = &input[pos..pos + element_end];
                let left_element = parse_value(element_input)?;
                pos += element_end;

                let element_end = find_element_end(&input[pos..])?;
                let element_input = &input[pos..pos + element_end];
                let right_element = parse_value(element_input)?;
                pos += element_end;

                elements.push((left_element, right_element));
            }

            Err(ParseError::new("Missing 'e' at end of this dict"))
        }
        _ => Err(ParseError::new("Not a valid prefix")),
    }
}

fn find_element_end(input: &str) -> Result<usize, ParseError> {
    match input.chars().next().unwrap() {
        '0'..='9' => {
            // String: find colon, read length, skip that many chars
            let colon_pos = input
                .find(':')
                .ok_or_else(|| ParseError::new("Missing colon"))?;
            let length = input[..colon_pos]
                .parse::<usize>()
                .map_err(|_| ParseError::new("Invalid length"))?;
            Ok(colon_pos + 1 + length)
        }
        'i' => {
            // Integer: find the 'e'
            let end_pos = input
                .find('e')
                .ok_or_else(|| ParseError::new("Missing 'e'"))?;
            Ok(end_pos + 1)
        }
        'l' => {
            // List: count nested brackets
            let mut pos = 1;
            let mut depth = 1;
            while pos < input.len() && depth > 0 {
                match input.chars().nth(pos).unwrap() {
                    'l' => depth += 1,
                    'e' => depth -= 1,
                    '0'..='9' => {
                        // Skip over string content
                        let colon_pos = input[pos..].find(':').unwrap() + pos;
                        let length = input[pos..colon_pos].parse::<usize>().unwrap();
                        pos = colon_pos + 1 + length;
                        continue;
                    }
                    'i' => {
                        // Skip to end of integer
                        pos = input[pos..].find('e').unwrap() + pos + 1;
                        continue;
                    }
                    _ => {}
                }
                pos += 1;
            }
            Ok(pos)
        }
        'd' => {
            // List: count nested brackets
            let mut pos = 1;
            let mut depth = 1;
            while pos < input.len() && depth > 0 {
                match input.chars().nth(pos).unwrap() {
                    'l' => depth += 1,
                    'd' => depth += 1,
                    'e' => depth -= 1,
                    '0'..='9' => {
                        // Skip over string content
                        let colon_pos = input[pos..].find(':').unwrap() + pos;
                        let length = input[pos..colon_pos].parse::<usize>().unwrap();
                        pos = colon_pos + 1 + length;
                        continue;
                    }
                    'i' => {
                        // Skip to end of integer
                        pos = input[pos..].find('e').unwrap() + pos + 1;
                        continue;
                    }

                    _ => {}
                }
                pos += 1;
            }
            Ok(pos)
        }
        _ => Err(ParseError::new("Invalid element type")),
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

    #[test]
    fn test_parse_list() {
        assert_eq!(
            parse_value("l4:spam4:eggse").unwrap(),
            "[\"spam\",\"eggs\"]"
        );

        assert_eq!(
            parse_value("l9:iloverusti-69ee").unwrap(),
            "[\"iloverust\",-69]"
        );
        // empty list
        assert_eq!(parse_value("le").unwrap(), "[]");
        // no e at end should throw error
        assert!(parse_value("l4:spam4:eggs").is_err());
        // nested lists
        assert_eq!(
            parse_value("l9:iloverustl4:spam4:eggsee").unwrap(),
            "[\"iloverust\",[\"spam\",\"eggs\"]]"
        );
    }

    #[test]
    fn test_parse_this_dict() {
        assert_eq!(
            parse_value("d3:foo3:bar5:helloi52ee").unwrap(),
            r#"{"foo":"bar","hello":52}"#
        );
    }
}
