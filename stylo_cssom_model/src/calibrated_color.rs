#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalRgbParams {
    pub white_point: [f32; 3],

    pub black_point: Option<[f32; 3]>,

    pub gamma: Option<[f32; 3]>,

    pub matrix: Option<[f32; 9]>,
}

impl Eq for CalRgbParams {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalGrayParams {
    pub white_point: [f32; 3],

    pub black_point: Option<[f32; 3]>,

    pub gamma: Option<f32>,
}

impl Eq for CalGrayParams {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabParams {
    pub white_point: [f32; 3],

    pub black_point: Option<[f32; 3]>,

    pub range: Option<[f32; 4]>,
}

impl Eq for LabParams {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibratedColour {
    CalRgb(CalRgbParams),

    CalGray(CalGrayParams),

    Lab(LabParams),
}
