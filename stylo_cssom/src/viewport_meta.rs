//! HTML viewport metadata, CSS Device Adaptation sections 6, 7 and 9.

#[derive(Clone, Copy, Debug, PartialEq)]
enum Length {
    Pixels(f32),
    DeviceWidth,
    DeviceHeight,
}

impl Length {
    fn resolve(self, width: f32, height: f32) -> f32 {
        match self {
            Self::Pixels(value) => value,
            Self::DeviceWidth => width,
            Self::DeviceHeight => height,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Dimension {
    Auto,
    Extend,
    Length(Length),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewportMeta {
    width: Option<Dimension>,
    height: Option<Dimension>,
    zoom: Option<f32>,
    min_zoom: Option<f32>,
    max_zoom: Option<f32>,
}

impl ViewportMeta {
    pub fn cascade(&mut self, content: &str) {
        let mut rule = Self::default();
        let mut input = content;
        while !input.is_empty() {
            input = input.trim_start_matches(|c| whitespace(c) || separator(c) || c == '=');
            let end = input
                .find(|c| whitespace(c) || separator(c) || c == '=')
                .unwrap_or(input.len());
            let name = &input[..end];
            input = &input[end..];
            let end = input
                .find(|c| separator(c) || c == '=')
                .unwrap_or(input.len());
            input = &input[end..];
            if !input.starts_with('=') {
                continue;
            }
            input = input.trim_start_matches(|c| whitespace(c) || c == '=');
            let end = input
                .find(|c| whitespace(c) || separator(c) || c == '=')
                .unwrap_or(input.len());
            let value = &input[..end];
            input = &input[end..];
            if value.is_empty() {
                continue;
            }
            let number = number_prefix(value);
            if number.is_some_and(|number| number < 0.0 || number.is_nan()) {
                continue;
            }
            match name.to_ascii_lowercase().as_str() {
                "width" | "height" => {
                    let length = match number {
                        Some(number) => Length::Pixels(number.clamp(1.0, 10000.0)),
                        None if value.eq_ignore_ascii_case("device-width") => Length::DeviceWidth,
                        None if value.eq_ignore_ascii_case("device-height") => Length::DeviceHeight,
                        None => Length::Pixels(1.0),
                    };
                    if name.eq_ignore_ascii_case("width") {
                        rule.width = Some(Dimension::Length(length));
                    } else {
                        rule.height = Some(Dimension::Length(length));
                    }
                },
                "initial-scale" | "minimum-scale" | "maximum-scale" => {
                    let zoom = match number {
                        Some(number) => number.clamp(0.1, 10.0),
                        None if value.eq_ignore_ascii_case("yes") => 1.0,
                        None if value.eq_ignore_ascii_case("device-width")
                            || value.eq_ignore_ascii_case("device-height") =>
                        {
                            10.0
                        },
                        None => 0.1,
                    };
                    match name.to_ascii_lowercase().as_str() {
                        "initial-scale" => rule.zoom = Some(zoom),
                        "minimum-scale" => rule.min_zoom = Some(zoom),
                        _ => rule.max_zoom = Some(zoom),
                    }
                },
                _ => {},
            }
        }
        if rule.zoom.is_some() && rule.width.is_none() {
            rule.width = Some(if rule.height.is_some() {
                Dimension::Auto
            } else {
                Dimension::Extend
            });
        }
        self.width = rule.width.or(self.width);
        self.height = rule.height.or(self.height);
        self.zoom = rule.zoom.or(self.zoom);
        self.min_zoom = rule.min_zoom.or(self.min_zoom);
        self.max_zoom = rule.max_zoom.or(self.max_zoom);
    }

    pub fn resolve(self, initial_width: f32, initial_height: f32) -> (f32, f32) {
        let max_zoom = self
            .max_zoom
            .map(|max| self.min_zoom.map_or(max, |min| max.max(min)));
        let zoom = self
            .zoom
            .map(|zoom| self.min_zoom.map_or(zoom, |min| zoom.max(min)))
            .map(|zoom| max_zoom.map_or(zoom, |max| zoom.min(max)));
        let extend_zoom = match (zoom, max_zoom) {
            (Some(zoom), Some(max)) => Some(zoom.min(max)),
            (zoom, max) => zoom.or(max),
        };
        let resolve_dimension = |dimension: Option<Dimension>, initial: f32| {
            let extent = extend_zoom.map(|zoom| initial / zoom);
            match dimension {
                None | Some(Dimension::Auto) => None,
                Some(Dimension::Extend) => extent,
                Some(Dimension::Length(length)) => {
                    let length = length.resolve(initial_width, initial_height);
                    Some(extent.map_or(length, |extent| extent.max(length)))
                },
            }
        };
        let width = resolve_dimension(self.width, initial_width);
        let height = resolve_dimension(self.height, initial_height);
        let width = width.unwrap_or_else(|| {
            height.map_or(initial_width, |height| {
                if initial_height == 0.0 {
                    initial_width
                } else {
                    height * initial_width / initial_height
                }
            })
        });
        let height = height.unwrap_or_else(|| {
            if initial_width == 0.0 {
                initial_height
            } else {
                width * initial_height / initial_width
            }
        });
        (width, height)
    }
}

fn whitespace(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | ' ')
}
fn separator(c: char) -> bool {
    matches!(c, ',' | ';')
}

fn number_prefix(value: &str) -> Option<f32> {
    let mut input = cssparser::ParserInput::new(value);
    let mut parser = cssparser::Parser::new(&mut input);
    match parser.next().ok()? {
        cssparser::Token::Number { value, .. } | cssparser::Token::Dimension { value, .. } => {
            Some(*value)
        },
        cssparser::Token::Percentage { unit_value, .. } => Some(*unit_value * 100.0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn viewport(content: &str, initial: (f32, f32)) -> (f32, f32) {
        let mut meta = ViewportMeta::default();
        meta.cascade(content);
        meta.resolve(initial.0, initial.1)
    }
    #[test]
    fn dimensions_preserve_the_initial_aspect_ratio() {
        assert_eq!(viewport("width=300", (800.0, 600.0)), (300.0, 225.0));
        assert_eq!(viewport("height=300", (800.0, 600.0)), (400.0, 300.0));
        assert_eq!(
            viewport("width=device-height,height=device-width", (800.0, 600.0)),
            (600.0, 800.0)
        );
    }
    #[test]
    fn metadata_scanner_and_numeric_prefixes() {
        assert_eq!(
            viewport("WIDTH = 300garbage; height==400, ignored=x", (800.0, 600.0)),
            (300.0, 400.0)
        );
        assert_eq!(viewport("width=-1,width=0", (800.0, 600.0)), (1.0, 0.75));
        assert_eq!(viewport("width=1e8", (800.0, 600.0)), (10000.0, 7500.0));
    }
    #[test]
    fn zoom_extends_dimensions_before_preserving_the_aspect_ratio() {
        assert_eq!(
            viewport("width=400, initial-scale=1", (320.0, 480.0)),
            (400.0, 600.0)
        );
        assert_eq!(
            viewport("width=400, initial-scale=1", (640.0, 480.0)),
            (640.0, 480.0)
        );
        assert_eq!(viewport("initial-scale=2", (800.0, 600.0)), (400.0, 300.0));
        assert_eq!(
            viewport("width=10, maximum-scale=5", (320.0, 480.0)),
            (64.0, 96.0)
        );
    }
    #[test]
    fn later_metadata_cascades_individual_descriptors() {
        let mut meta = ViewportMeta::default();
        meta.cascade("width=300,height=400");
        meta.cascade("width=-100");
        assert_eq!(meta.resolve(800.0, 600.0), (300.0, 400.0));
        meta.cascade("initial-scale=2");
        assert_eq!(meta.resolve(800.0, 600.0), (400.0, 400.0));
    }
}
