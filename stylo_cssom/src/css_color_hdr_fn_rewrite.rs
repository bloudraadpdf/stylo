use std::{borrow::Cow, fmt::Write as _};

use stylo_cssom_model::color_matrix::multiply_3x3_vector as matmul3;

use crate::css_scan::{
    current_property_name, find_matching_close_paren, is_ident_continue, parse_component_as_f32,
    utf8_char_width,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssColorHdrFn {
    Ictcp,

    Jzazbz,

    Jzczhz,
}

impl CssColorHdrFn {
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Ictcp => "ictcp",
            Self::Jzazbz => "jzazbz",
            Self::Jzczhz => "jzczhz",
        }
    }

    pub fn from_marker(marker: &str) -> Option<Self> {
        match marker.trim() {
            "ictcp" => Some(Self::Ictcp),
            "jzazbz" => Some(Self::Jzazbz),
            "jzczhz" => Some(Self::Jzczhz),
            _ => None,
        }
    }
}

pub const CSSCOLOR_FN_MARKER_PREFIX: &str = "--__moegoe_csscolor_fn_";

pub fn marker_property_name(longhand: &str) -> String {
    let mut out = String::with_capacity(CSSCOLOR_FN_MARKER_PREFIX.len() + longhand.len());
    out.push_str(CSSCOLOR_FN_MARKER_PREFIX);
    out.push_str(longhand);
    out
}

const ICTCP_OPEN: &[u8] = b"ictcp(";
const JZAZBZ_OPEN: &[u8] = b"jzazbz(";
const JZCZHZ_OPEN: &[u8] = b"jzczhz(";

pub fn rewrite_csscolor_hdr_fn_calls(css: &str) -> Cow<'_, str> {
    if !contains_csscolor_hdr_fn(css) {
        return Cow::Borrowed(css);
    }
    Cow::Owned(rewrite_owned(css))
}

fn contains_csscolor_hdr_fn(css: &str) -> bool {
    crate::css_scan::contains_any_ascii_ci(css, &[ICTCP_OPEN, JZAZBZ_OPEN, JZCZHZ_OPEN])
}

fn rewrite_owned(css: &str) -> String {
    let mut out = String::with_capacity(css.len() + 64);
    let bytes = css.as_bytes();
    let mut i = 0;

    let mut pending_markers: Vec<String> = Vec::new();

    while i < bytes.len() {
        if bytes[i] == b';' || bytes[i] == b'}' {
            for marker in pending_markers.drain(..) {
                out.push_str(&marker);
            }
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }

        if let Some((fn_kind, fn_token_len)) = match_csscolor_fn_token(bytes, i) {
            let at_token_boundary = i == 0 || !is_ident_continue(bytes[i - 1]);
            if at_token_boundary {
                if let Some(parsed) = parse_csscolor_fn_call(bytes, i, fn_token_len, fn_kind) {
                    let longhand = current_property_name(&out);
                    out.push_str("color(xyz-d65 ");

                    let _ = write!(
                        out,
                        "{:.6} {:.6} {:.6}",
                        parsed.xyz_d65[0], parsed.xyz_d65[1], parsed.xyz_d65[2]
                    );
                    if let Some(alpha) = parsed.alpha_str {
                        out.push_str(" / ");
                        out.push_str(&alpha);
                    }
                    out.push(')');
                    if let Some(name) = longhand {
                        let marker =
                            format!("; {}: {}", marker_property_name(&name), fn_kind.marker());
                        pending_markers.push(marker);
                    }
                    i = parsed.end;
                } else {
                    out.push(bytes[i] as char);
                    i += 1;
                }
                continue;
            }
        }

        let char_len = utf8_char_width(bytes[i]);
        out.push_str(&css[i..i + char_len]);
        i += char_len;
    }

    for marker in pending_markers.drain(..) {
        out.push_str(&marker);
    }

    out
}

fn match_csscolor_fn_token(bytes: &[u8], i: usize) -> Option<(CssColorHdrFn, usize)> {
    crate::css_scan::match_any_ascii_ci(bytes, i, &[ICTCP_OPEN, JZAZBZ_OPEN, JZCZHZ_OPEN]).map(
        |(index, length)| {
            (
                [
                    CssColorHdrFn::Ictcp,
                    CssColorHdrFn::Jzazbz,
                    CssColorHdrFn::Jzczhz,
                ][index],
                length,
            )
        },
    )
}

struct ParsedCssColorFn {
    xyz_d65: [f32; 3],

    alpha_str: Option<String>,
    end: usize,
}

fn parse_csscolor_fn_call(
    bytes: &[u8],
    start: usize,
    fn_token_len: usize,
    kind: CssColorHdrFn,
) -> Option<ParsedCssColorFn> {
    let after_open = start + fn_token_len;
    let close = find_matching_close_paren(bytes, after_open)?;
    let body = std::str::from_utf8(&bytes[after_open..close]).ok()?;

    let (head, alpha_str) = match body.split_once('/') {
        Some((h, a)) => (h.trim(), Some(a.trim().to_string())),
        None => (body.trim(), None),
    };

    let mut tokens = head.split(|c: char| c.is_ascii_whitespace() || c == ',');
    let c0 = next_non_empty(&mut tokens).and_then(parse_component_as_f32)?;
    let c1 = next_non_empty(&mut tokens).and_then(parse_component_as_f32)?;
    let c2 = next_non_empty(&mut tokens).and_then(parse_component_as_f32)?;
    if next_non_empty(&mut tokens).is_some() {
        return None;
    }

    let xyz_d65 = match kind {
        CssColorHdrFn::Ictcp => ictcp_to_xyz_d65([c0, c1, c2]),
        CssColorHdrFn::Jzazbz => jzazbz_to_xyz_d65([c0, c1, c2]),
        CssColorHdrFn::Jzczhz => {
            let (jz, az, bz) = jzczhz_to_jzazbz(c0, c1, c2);
            jzazbz_to_xyz_d65([jz, az, bz])
        },
    };

    Some(ParsedCssColorFn {
        xyz_d65,
        alpha_str,
        end: close + 1,
    })
}

fn next_non_empty<'a, I: Iterator<Item = &'a str>>(iter: &mut I) -> Option<&'a str> {
    for token in iter {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

#[allow(clippy::items_after_statements)]
fn ictcp_to_xyz_d65(ictcp: [f32; 3]) -> [f32; 3] {
    const ICTCP_TO_LMS_PRIME: [[f32; 3]; 3] = [
        [1.0, 0.008_609_037, 0.111_029_625],
        [1.0, -0.008_609_037, -0.111_029_625],
        [1.0, 0.560_031_3, -0.320_627_05],
    ];
    let lms_prime = matmul3(ICTCP_TO_LMS_PRIME, ictcp);

    let lms = [
        pq_eotf(lms_prime[0]),
        pq_eotf(lms_prime[1]),
        pq_eotf(lms_prime[2]),
    ];

    const LMS_TO_XYZ_D65: [[f32; 3]; 3] = [
        [2.070_658, -1.326_456_4, 0.206_900_8],
        [0.364_990_2, 0.680_469_4, -0.045_459_5],
        [-0.049_595_4, -0.049_421_4, 1.188_065],
    ];
    let xyz = matmul3(LMS_TO_XYZ_D65, lms);

    const PQ_PEAK_NITS: f32 = 10_000.0;
    const SDR_DIFFUSE_NITS: f32 = 203.0;
    let scale = SDR_DIFFUSE_NITS / PQ_PEAK_NITS * 100.0;
    [xyz[0] * scale, xyz[1] * scale, xyz[2] * scale]
}

#[allow(clippy::items_after_statements)]
fn jzazbz_to_xyz_d65(jzazbz: [f32; 3]) -> [f32; 3] {
    const D: f32 = -0.56;
    const D0: f32 = 1.629_549_5e-12;
    const N: f32 = 2_610.0 / 16_384.0;
    const P: f32 = 1.7 * 2_523.0 / 32.0;
    const C1: f32 = 3_424.0 / 4_096.0;
    const C2: f32 = 2_413.0 / 128.0;
    const C3: f32 = 2_392.0 / 128.0;
    const B: f32 = 1.15;
    const G: f32 = 0.66;

    let jz = jzazbz[0];
    let iz = (jz + D0) / (1.0 + D - D * (jz + D0));

    const M2_INV: [[f32; 3]; 3] = [
        [1.0, 0.138_605_04, 0.058_047_316],
        [1.0, -0.138_605_04, -0.058_047_316],
        [1.0, -0.096_019_242, -0.811_892],
    ];
    let lms_prime = matmul3(M2_INV, [iz, jzazbz[1], jzazbz[2]]);

    let lms = [
        jzazbz_pq_inverse(lms_prime[0], N, P, C1, C2, C3),
        jzazbz_pq_inverse(lms_prime[1], N, P, C1, C2, C3),
        jzazbz_pq_inverse(lms_prime[2], N, P, C1, C2, C3),
    ];

    const M1_INV: [[f32; 3]; 3] = [
        [1.924_226_4, -1.004_792_3, 0.037_651_405],
        [0.350_316_55, 0.726_481_9, -0.065_384_17],
        [-0.090_982_11, -0.312_728_08, 1.522_766_6],
    ];
    let xyz_prime = matmul3(M1_INV, lms);

    let x = (xyz_prime[0] + (B - 1.0) * xyz_prime[2]) / B;
    let y = (xyz_prime[1] + (G - 1.0) * x) / G;
    [x, y, xyz_prime[2]]
}

fn jzazbz_pq_inverse(v: f32, n: f32, p: f32, c1: f32, c2: f32, c3: f32) -> f32 {
    let v = v.max(0.0);
    let num = v.powf(1.0 / p) - c1;
    let denom = c2 - c3 * v.powf(1.0 / p);
    if denom.abs() < 1e-12 {
        return 0.0;
    }
    let frac = (num / denom).max(0.0);
    10_000.0 * frac.powf(1.0 / n) / 10_000.0
}

fn jzczhz_to_jzazbz(jz: f32, cz: f32, hz: f32) -> (f32, f32, f32) {
    let radians = hz.to_radians();
    let az = cz * radians.cos();
    let bz = cz * radians.sin();
    (jz, az, bz)
}

fn pq_eotf(v: f32) -> f32 {
    const C1: f32 = 0.835_937_5;
    const C2: f32 = 18.851_563;
    const C3: f32 = 18.687_5;
    const M1: f32 = 0.159_301_76;
    const M2: f32 = 78.843_75;
    let v = v.max(0.0);
    let v_m2 = v.powf(1.0 / M2);
    let num = (v_m2 - C1).max(0.0);
    let denom = C2 - C3 * v_m2;
    if denom.abs() < 1e-12 {
        return 0.0;
    }
    (num / denom).powf(1.0 / M1)
}
