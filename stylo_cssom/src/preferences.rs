use std::sync::Once;

struct RequiredServoStylePref {
    name: &'static str,
    value: bool,
    rationale: &'static str,
}

const REQUIRED_SERVO_STYLE_PREFS: &[RequiredServoStylePref] = &[
    RequiredServoStylePref {
        name: "layout.grid.enabled",
        value: true,
        rationale: "CSS Grid layout parsing and computed values",
    },
    RequiredServoStylePref {
        name: "layout.columns.enabled",
        value: true,
        rationale: "CSS Multi-column layout parsing and computed values",
    },
    RequiredServoStylePref {
        name: "layout.container-queries.enabled",
        value: true,
        rationale: "container-type/container-name plus size queries and cq units",
    },
    RequiredServoStylePref {
        name: "layout.css.style-queries.enabled",
        value: true,
        rationale: "the style-query half of @container support",
    },
    RequiredServoStylePref {
        name: "layout.css.basic-shape-shape.enabled",
        value: true,
        rationale: "clip-path: shape(...) parsing",
    },
    RequiredServoStylePref {
        name: "layout.css.motion-path-url.enabled",
        value: true,
        rationale: "offset-path URL references to SVG geometry",
    },
    RequiredServoStylePref {
        name: "layout.css.attr.enabled",
        value: true,
        rationale: "attr() in generated content and property values",
    },
    RequiredServoStylePref {
        name: "layout.css.content.alt-text.enabled",
        value: true,
        rationale: "CSS Content 3 `<main> / <alt>` alt-text syntax in generated content",
    },
    RequiredServoStylePref {
        name: "layout.css.fit-content-function.enabled",
        value: true,
        rationale: "fit-content(<length-percentage>) sizing syntax",
    },
    RequiredServoStylePref {
        name: "layout.css.system-ui.enabled",
        value: true,
        rationale: "system-ui generic font-family handling",
    },
    RequiredServoStylePref {
        name: "layout.css.properties-and-values.enabled",
        value: true,
        rationale: "@property custom property registration",
    },
    RequiredServoStylePref {
        name: "layout.css.at-scope.enabled",
        value: true,
        rationale: "@scope cascade filtering handled inside Stylo",
    },
    RequiredServoStylePref {
        name: "layout.css.starting-style-at-rules.enabled",
        value: true,
        rationale: "@starting-style parsing and transition before-change styles",
    },
    RequiredServoStylePref {
        name: "layout.css.custom-media.enabled",
        value: true,
        rationale: "@custom-media expansion inside media queries",
    },
    RequiredServoStylePref {
        name: "layout.css.anchor-positioning.enabled",
        value: true,
        rationale: "@position-try rules plus anchor()/anchor-size() parsing",
    },
    RequiredServoStylePref {
        name: "layout.css.scroll-driven-animations.enabled",
        value: true,
        rationale: "named progress timelines, automatic durations, and animation ranges",
    },
    RequiredServoStylePref {
        name: "layout.css.grid-template-masonry-value.enabled",
        value: true,
        rationale: "masonry keyword parsing for grid-template diagnostics",
    },
    RequiredServoStylePref {
        name: "layout.variable_fonts.enabled",
        value: true,
        rationale: "CSS Fonts 4 font-variation-settings parsing and computed values",
    },
    RequiredServoStylePref {
        name: "layout.css.font-tech.enabled",
        value: true,
        rationale: "CSS Conditional 4 §4 @supports font-tech() / font-format() \
                    predicate parsing; evaluator verdicts come from the moegoe \
                    Stylo fork (`supports_rule.rs::eval_font_tech` / \
                    `::eval_font_format`)",
    },
    RequiredServoStylePref {
        name: "layout.unimplemented",
        value: true,
        rationale: "Servo-gated counter and other already-consumed properties",
    },
    RequiredServoStylePref {
        name: "layout.writing-mode.enabled",
        value: true,
        rationale: "writing-mode and direction-sensitive layout",
    },
];

pub fn initialise_required_servo_style_prefs() {
    static INITIALISE: Once = Once::new();
    INITIALISE.call_once(|| {
        for pref in REQUIRED_SERVO_STYLE_PREFS {
            debug_assert!(
                !pref.rationale.is_empty(),
                "required Servo pref {} must carry a rationale",
                pref.name
            );
            stylo_config::set_bool(pref.name, pref.value);
        }
    });
}
