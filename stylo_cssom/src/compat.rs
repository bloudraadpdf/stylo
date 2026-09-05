pub mod metadata;
pub mod translate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompatMode {
    #[default]
    None,

    PdfReactor,

    Prince,
}
