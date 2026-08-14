servo-style
===========

Style system for Servo, using [rust-cssparser](https://github.com/servo/rust-cssparser) for parsing.

 * [Documentation](https://book.servo.org/architecture/style.html).

Authored stylesheets select cssparser's CSS 2 malformed-URL recovery mode.
This retains the component-value topology of invalid legacy `url()` syntax so
that unterminated strings and matched pairs delimit declarations and rules as
required by CSS 2.1. Other parser entry points keep cssparser's CSS Syntax
Level 3 default unless their owning standard requires the legacy recovery.
