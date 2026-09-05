#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssIdentifierSerialization(Vec<u16>);

impl CssIdentifierSerialization {
    pub fn as_utf16(&self) -> &[u16] {
        &self.0
    }
}

pub fn serialize_css_identifier(identifier: &[u16]) -> CssIdentifierSerialization {
    let mut output = Vec::with_capacity(identifier.len());
    for (index, &unit) in identifier.iter().enumerate() {
        if unit == 0 {
            output.push(0xfffd);
        } else if is_control(unit)
            || index == 0 && is_ascii_digit(unit)
            || index == 1 && is_ascii_digit(unit) && identifier.first() == Some(&u16::from(b'-'))
        {
            escape_as_code_point(&mut output, unit);
        } else if index == 0 && unit == u16::from(b'-') && identifier.len() == 1 {
            output.extend([u16::from(b'\\'), unit]);
        } else if is_identifier_code_unit(unit) {
            output.push(unit);
        } else {
            output.extend([u16::from(b'\\'), unit]);
        }
    }
    CssIdentifierSerialization(output)
}

const fn is_control(unit: u16) -> bool {
    matches!(unit, 0x0001..=0x001f | 0x007f)
}

const fn is_ascii_digit(unit: u16) -> bool {
    matches!(unit, 0x0030..=0x0039)
}

const fn is_identifier_code_unit(unit: u16) -> bool {
    unit >= 0x0080
        || matches!(
            unit,
            0x002d | 0x005f | 0x0030..=0x0039 | 0x0041..=0x005a | 0x0061..=0x007a
        )
}

fn escape_as_code_point(output: &mut Vec<u16>, unit: u16) {
    output.push(u16::from(b'\\'));
    output.extend(format!("{unit:x}").encode_utf16());
    output.push(u16::from(b' '));
}

#[cfg(test)]
mod tests {
    use super::serialize_css_identifier;

    fn serialize(input: &str) -> String {
        String::from_utf16(
            serialize_css_identifier(&input.encode_utf16().collect::<Vec<_>>()).as_utf16(),
        )
        .expect("scalar input produces scalar output")
    }

    #[test]
    fn serializes_cssom_identifier_edges() {
        assert_eq!(serialize("0a"), "\\30 a");
        assert_eq!(serialize("-0a"), "-\\30 a");
        assert_eq!(serialize("-"), "\\-");
        assert_eq!(serialize("\0"), "\u{fffd}");
        assert_eq!(serialize("\u{7f}"), "\\7f ");
        assert_eq!(serialize("hello\\world"), "hello\\\\world");
        assert_eq!(serialize("--a"), "--a");
    }

    #[test]
    fn preserves_lone_utf16_surrogates() {
        let input = [0xd834, u16::from(b'a'), 0xdf06];
        assert_eq!(serialize_css_identifier(&input).as_utf16(), input);
    }
}
