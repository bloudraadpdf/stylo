#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollMarkerGroupPosition {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollMarkerGroupMode {
    Links,
    Tabs,
}
