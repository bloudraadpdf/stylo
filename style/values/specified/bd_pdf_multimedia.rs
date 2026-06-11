/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-multimedia-*` PDF multimedia annotation properties
//! (V1-21 — Sound / Movie / Screen, ISO 32000-2 §12.5.6.16–18).
//!
//! Native moegoe fork-extension surface that controls whether a
//! multimedia HTML element (`<audio>`, `<video>`, `<embed>`) emits
//! the matching PDF multimedia annotation. By default the convert
//! layer emits one annotation per element; `none` suppresses
//! emission while keeping the element's layout box for fallback
//! rendering. These properties apply only to elements that already
//! produce a multimedia annotation; they are not inherited.
//!
//! Companion properties:
//!
//! * `-bd-pdf-multimedia-format` — sound encoding hint (`auto` |
//!   `pcm` | `mulaw` | `alaw`). The default `auto` lets the convert
//!   layer sniff the audio container (WAV → PCM) and emit a typed
//!   warning when the encoding cannot be determined.
//!
//! The cluster is intentionally small: rich multimedia parameters
//! (sample rate, channels, bit depth) come from the audio container
//! header itself; only properties that override HTML semantics need
//! a CSS cascade surface.

use crate::derives::*;

/// Specified value of `-bd-pdf-multimedia`.
///
/// Controls whether a multimedia HTML element emits the
/// corresponding PDF multimedia annotation. `auto` (initial) keeps
/// HTML semantics — `<audio>` produces a Sound annotation,
/// `<video>` a Screen annotation, `<embed type="video/...">` a
/// Movie annotation. `none` suppresses annotation emission so the
/// element behaves as a regular replaced box without an interactive
/// overlay.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdPdfMultimedia {
    /// Default — derive annotation emission from the HTML element
    /// type.
    #[default]
    Auto,
    /// Suppress the PDF multimedia annotation for this element.
    None,
}

impl BdPdfMultimedia {
    /// Whether the keyword is `auto` (initial — emit per HTML
    /// semantics).
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }

    /// Whether the keyword is `none` (suppress emission).
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Specified value of `-bd-pdf-multimedia-format`.
///
/// Sound-annotation encoding hint per ISO 32000-2 §12.5.6.16
/// Table 185. `auto` (initial) lets the convert layer detect the
/// encoding from the audio container header; the explicit keywords
/// force the corresponding PDF `/E` entry. moegoe only embeds
/// uncompressed PCM audio in v1; `mulaw` / `alaw` are reserved for
/// authors who supply pre-encoded payloads via a future
/// `-bd-pdf-multimedia-bytes` longhand.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdPdfMultimediaFormat {
    /// Default — sniff the encoding from the audio container.
    #[default]
    Auto,
    /// `/E /Raw` — linear PCM samples, unsigned 8-bit / signed 16+.
    Pcm,
    /// `/E /muLaw` — 8-bit µ-law compressed samples.
    Mulaw,
    /// `/E /ALaw` — 8-bit A-law compressed samples.
    Alaw,
}

impl BdPdfMultimediaFormat {
    /// Whether the keyword is `auto` (initial — detect from
    /// container).
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}
