pub type CmykQuad = [f32; 4];

pub const NAMED_COLOUR_CMYK_TABLE_LEN: usize = 139;

pub const NAMED_COLOUR_CMYK_TABLE: &[(&str, (u8, u8, u8), CmykQuad)] = &[
    ("black", (0, 0, 0), [0.0, 0.0, 0.0, 1.0]),
    ("silver", (192, 192, 192), naive_cmyk(192, 192, 192)),
    ("gray", (128, 128, 128), naive_cmyk(128, 128, 128)),
    ("white", (255, 255, 255), [0.0, 0.0, 0.0, 0.0]),
    ("maroon", (128, 0, 0), naive_cmyk(128, 0, 0)),
    ("red", (255, 0, 0), [0.0, 1.0, 1.0, 0.0]),
    ("purple", (128, 0, 128), naive_cmyk(128, 0, 128)),
    ("fuchsia", (255, 0, 255), [0.0, 1.0, 0.0, 0.0]),
    ("green", (0, 128, 0), naive_cmyk(0, 128, 0)),
    ("lime", (0, 255, 0), [1.0, 0.0, 1.0, 0.0]),
    ("olive", (128, 128, 0), naive_cmyk(128, 128, 0)),
    ("yellow", (255, 255, 0), [0.0, 0.0, 1.0, 0.0]),
    ("navy", (0, 0, 128), naive_cmyk(0, 0, 128)),
    ("blue", (0, 0, 255), [1.0, 1.0, 0.0, 0.0]),
    ("teal", (0, 128, 128), naive_cmyk(0, 128, 128)),
    ("aqua", (0, 255, 255), [1.0, 0.0, 0.0, 0.0]),
    ("aliceblue", (240, 248, 255), naive_cmyk(240, 248, 255)),
    ("antiquewhite", (250, 235, 215), naive_cmyk(250, 235, 215)),
    ("aquamarine", (127, 255, 212), naive_cmyk(127, 255, 212)),
    ("azure", (240, 255, 255), naive_cmyk(240, 255, 255)),
    ("beige", (245, 245, 220), naive_cmyk(245, 245, 220)),
    ("bisque", (255, 228, 196), naive_cmyk(255, 228, 196)),
    ("blanchedalmond", (255, 235, 205), naive_cmyk(255, 235, 205)),
    ("blueviolet", (138, 43, 226), naive_cmyk(138, 43, 226)),
    ("brown", (165, 42, 42), naive_cmyk(165, 42, 42)),
    ("burlywood", (222, 184, 135), naive_cmyk(222, 184, 135)),
    ("cadetblue", (95, 158, 160), naive_cmyk(95, 158, 160)),
    ("chartreuse", (127, 255, 0), naive_cmyk(127, 255, 0)),
    ("chocolate", (210, 105, 30), naive_cmyk(210, 105, 30)),
    ("coral", (255, 127, 80), naive_cmyk(255, 127, 80)),
    ("cornflowerblue", (100, 149, 237), naive_cmyk(100, 149, 237)),
    ("cornsilk", (255, 248, 220), naive_cmyk(255, 248, 220)),
    ("crimson", (220, 20, 60), naive_cmyk(220, 20, 60)),
    ("darkblue", (0, 0, 139), naive_cmyk(0, 0, 139)),
    ("darkcyan", (0, 139, 139), naive_cmyk(0, 139, 139)),
    ("darkgoldenrod", (184, 134, 11), naive_cmyk(184, 134, 11)),
    ("darkgray", (169, 169, 169), naive_cmyk(169, 169, 169)),
    ("darkgreen", (0, 100, 0), naive_cmyk(0, 100, 0)),
    ("darkkhaki", (189, 183, 107), naive_cmyk(189, 183, 107)),
    ("darkmagenta", (139, 0, 139), naive_cmyk(139, 0, 139)),
    ("darkolivegreen", (85, 107, 47), naive_cmyk(85, 107, 47)),
    ("darkorange", (255, 140, 0), naive_cmyk(255, 140, 0)),
    ("darkorchid", (153, 50, 204), naive_cmyk(153, 50, 204)),
    ("darkred", (139, 0, 0), naive_cmyk(139, 0, 0)),
    ("darksalmon", (233, 150, 122), naive_cmyk(233, 150, 122)),
    ("darkseagreen", (143, 188, 143), naive_cmyk(143, 188, 143)),
    ("darkslateblue", (72, 61, 139), naive_cmyk(72, 61, 139)),
    ("darkslategray", (47, 79, 79), naive_cmyk(47, 79, 79)),
    ("darkturquoise", (0, 206, 209), naive_cmyk(0, 206, 209)),
    ("darkviolet", (148, 0, 211), naive_cmyk(148, 0, 211)),
    ("deeppink", (255, 20, 147), naive_cmyk(255, 20, 147)),
    ("deepskyblue", (0, 191, 255), naive_cmyk(0, 191, 255)),
    ("dimgray", (105, 105, 105), naive_cmyk(105, 105, 105)),
    ("dodgerblue", (30, 144, 255), naive_cmyk(30, 144, 255)),
    ("firebrick", (178, 34, 34), naive_cmyk(178, 34, 34)),
    ("floralwhite", (255, 250, 240), naive_cmyk(255, 250, 240)),
    ("forestgreen", (34, 139, 34), naive_cmyk(34, 139, 34)),
    ("gainsboro", (220, 220, 220), naive_cmyk(220, 220, 220)),
    ("ghostwhite", (248, 248, 255), naive_cmyk(248, 248, 255)),
    ("gold", (255, 215, 0), naive_cmyk(255, 215, 0)),
    ("goldenrod", (218, 165, 32), naive_cmyk(218, 165, 32)),
    ("greenyellow", (173, 255, 47), naive_cmyk(173, 255, 47)),
    ("honeydew", (240, 255, 240), naive_cmyk(240, 255, 240)),
    ("hotpink", (255, 105, 180), naive_cmyk(255, 105, 180)),
    ("indianred", (205, 92, 92), naive_cmyk(205, 92, 92)),
    ("indigo", (75, 0, 130), naive_cmyk(75, 0, 130)),
    ("ivory", (255, 255, 240), naive_cmyk(255, 255, 240)),
    ("khaki", (240, 230, 140), naive_cmyk(240, 230, 140)),
    ("lavender", (230, 230, 250), naive_cmyk(230, 230, 250)),
    ("lavenderblush", (255, 240, 245), naive_cmyk(255, 240, 245)),
    ("lawngreen", (124, 252, 0), naive_cmyk(124, 252, 0)),
    ("lemonchiffon", (255, 250, 205), naive_cmyk(255, 250, 205)),
    ("lightblue", (173, 216, 230), naive_cmyk(173, 216, 230)),
    ("lightcoral", (240, 128, 128), naive_cmyk(240, 128, 128)),
    ("lightcyan", (224, 255, 255), naive_cmyk(224, 255, 255)),
    (
        "lightgoldenrodyellow",
        (250, 250, 210),
        naive_cmyk(250, 250, 210),
    ),
    ("lightgray", (211, 211, 211), naive_cmyk(211, 211, 211)),
    ("lightgreen", (144, 238, 144), naive_cmyk(144, 238, 144)),
    ("lightpink", (255, 182, 193), naive_cmyk(255, 182, 193)),
    ("lightsalmon", (255, 160, 122), naive_cmyk(255, 160, 122)),
    ("lightseagreen", (32, 178, 170), naive_cmyk(32, 178, 170)),
    ("lightskyblue", (135, 206, 250), naive_cmyk(135, 206, 250)),
    ("lightslategray", (119, 136, 153), naive_cmyk(119, 136, 153)),
    ("lightsteelblue", (176, 196, 222), naive_cmyk(176, 196, 222)),
    ("lightyellow", (255, 255, 224), naive_cmyk(255, 255, 224)),
    ("limegreen", (50, 205, 50), naive_cmyk(50, 205, 50)),
    ("linen", (250, 240, 230), naive_cmyk(250, 240, 230)),
    (
        "mediumaquamarine",
        (102, 205, 170),
        naive_cmyk(102, 205, 170),
    ),
    ("mediumblue", (0, 0, 205), naive_cmyk(0, 0, 205)),
    ("mediumorchid", (186, 85, 211), naive_cmyk(186, 85, 211)),
    ("mediumpurple", (147, 112, 219), naive_cmyk(147, 112, 219)),
    ("mediumseagreen", (60, 179, 113), naive_cmyk(60, 179, 113)),
    (
        "mediumslateblue",
        (123, 104, 238),
        naive_cmyk(123, 104, 238),
    ),
    ("mediumspringgreen", (0, 250, 154), naive_cmyk(0, 250, 154)),
    ("mediumturquoise", (72, 209, 204), naive_cmyk(72, 209, 204)),
    ("mediumvioletred", (199, 21, 133), naive_cmyk(199, 21, 133)),
    ("midnightblue", (25, 25, 112), naive_cmyk(25, 25, 112)),
    ("mintcream", (245, 255, 250), naive_cmyk(245, 255, 250)),
    ("mistyrose", (255, 228, 225), naive_cmyk(255, 228, 225)),
    ("moccasin", (255, 228, 181), naive_cmyk(255, 228, 181)),
    ("navajowhite", (255, 222, 173), naive_cmyk(255, 222, 173)),
    ("oldlace", (253, 245, 230), naive_cmyk(253, 245, 230)),
    ("olivedrab", (107, 142, 35), naive_cmyk(107, 142, 35)),
    ("orange", (255, 165, 0), naive_cmyk(255, 165, 0)),
    ("orangered", (255, 69, 0), naive_cmyk(255, 69, 0)),
    ("orchid", (218, 112, 214), naive_cmyk(218, 112, 214)),
    ("palegoldenrod", (238, 232, 170), naive_cmyk(238, 232, 170)),
    ("palegreen", (152, 251, 152), naive_cmyk(152, 251, 152)),
    ("paleturquoise", (175, 238, 238), naive_cmyk(175, 238, 238)),
    ("palevioletred", (219, 112, 147), naive_cmyk(219, 112, 147)),
    ("papayawhip", (255, 239, 213), naive_cmyk(255, 239, 213)),
    ("peachpuff", (255, 218, 185), naive_cmyk(255, 218, 185)),
    ("peru", (205, 133, 63), naive_cmyk(205, 133, 63)),
    ("pink", (255, 192, 203), naive_cmyk(255, 192, 203)),
    ("plum", (221, 160, 221), naive_cmyk(221, 160, 221)),
    ("powderblue", (176, 224, 230), naive_cmyk(176, 224, 230)),
    ("rebeccapurple", (102, 51, 153), naive_cmyk(102, 51, 153)),
    ("rosybrown", (188, 143, 143), naive_cmyk(188, 143, 143)),
    ("royalblue", (65, 105, 225), naive_cmyk(65, 105, 225)),
    ("saddlebrown", (139, 69, 19), naive_cmyk(139, 69, 19)),
    ("salmon", (250, 128, 114), naive_cmyk(250, 128, 114)),
    ("sandybrown", (244, 164, 96), naive_cmyk(244, 164, 96)),
    ("seagreen", (46, 139, 87), naive_cmyk(46, 139, 87)),
    ("seashell", (255, 245, 238), naive_cmyk(255, 245, 238)),
    ("sienna", (160, 82, 45), naive_cmyk(160, 82, 45)),
    ("skyblue", (135, 206, 235), naive_cmyk(135, 206, 235)),
    ("slateblue", (106, 90, 205), naive_cmyk(106, 90, 205)),
    ("slategray", (112, 128, 144), naive_cmyk(112, 128, 144)),
    ("snow", (255, 250, 250), naive_cmyk(255, 250, 250)),
    ("springgreen", (0, 255, 127), naive_cmyk(0, 255, 127)),
    ("steelblue", (70, 130, 180), naive_cmyk(70, 130, 180)),
    ("tan", (210, 180, 140), naive_cmyk(210, 180, 140)),
    ("thistle", (216, 191, 216), naive_cmyk(216, 191, 216)),
    ("tomato", (255, 99, 71), naive_cmyk(255, 99, 71)),
    ("turquoise", (64, 224, 208), naive_cmyk(64, 224, 208)),
    ("violet", (238, 130, 238), naive_cmyk(238, 130, 238)),
    ("wheat", (245, 222, 179), naive_cmyk(245, 222, 179)),
    ("whitesmoke", (245, 245, 245), naive_cmyk(245, 245, 245)),
    ("yellowgreen", (154, 205, 50), naive_cmyk(154, 205, 50)),
];

#[doc(hidden)]
#[allow(clippy::many_single_char_names)]
pub const fn naive_cmyk(r: u8, g: u8, b: u8) -> CmykQuad {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;
    let max_rgb = const_max3(rf, gf, bf);
    let k = 1.0 - max_rgb;
    if k >= 1.0 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let denom = 1.0 - k;
    let c = (1.0 - rf - k) / denom;
    let m = (1.0 - gf - k) / denom;
    let y = (1.0 - bf - k) / denom;
    [c, m, y, k]
}

#[doc(hidden)]
const fn const_max3(a: f32, b: f32, c: f32) -> f32 {
    let ab = if a >= b { a } else { b };
    if ab >= c { ab } else { c }
}

#[inline]
pub fn named_colour_to_cmyk(r: u8, g: u8, b: u8) -> Option<CmykQuad> {
    NAMED_COLOUR_CMYK_TABLE
        .iter()
        .find(|(_, rgb, _)| *rgb == (r, g, b))
        .map(|(_, _, cmyk)| *cmyk)
}
