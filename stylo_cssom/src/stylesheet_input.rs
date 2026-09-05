use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE};
use stylo_cssom_model::StylesheetEnvironmentEncoding;

pub fn decode_stylesheet_bytes(
    bytes: &[u8],
    transport_encoding: Option<&'static Encoding>,
    environment: StylesheetEnvironmentEncoding,
) -> (String, StylesheetEnvironmentEncoding) {
    let bom = Encoding::for_bom(bytes);
    let (encoding, bom_length) = match bom {
        Some((encoding, length)) => (encoding, length),
        None => (
            transport_encoding
                .or_else(|| stylesheet_byte_prefix_encoding(bytes))
                .unwrap_or(environment.encoding()),
            0,
        ),
    };
    let (text, _) = encoding.decode_without_bom_handling(&bytes[bom_length..]);
    (
        text.into_owned(),
        StylesheetEnvironmentEncoding::new(encoding),
    )
}

fn stylesheet_byte_prefix_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    const PREFIX: &[u8] = b"@charset \"";
    let candidate = bytes.get(..bytes.len().min(1024))?;
    let rest = candidate.strip_prefix(PREFIX)?;
    let label_end = rest.iter().position(|byte| *byte == b'"')?;
    let (label, suffix) = rest.split_at(label_end);
    if suffix.get(1) != Some(&b';')
        || !label
            .iter()
            .all(|byte| (0x00..=0x21).contains(byte) || (0x23..=0x7f).contains(byte))
    {
        return None;
    }
    let encoding = Encoding::for_label(label)?;
    Some(if encoding == UTF_16BE || encoding == UTF_16LE {
        UTF_8
    } else {
        encoding
    })
}
