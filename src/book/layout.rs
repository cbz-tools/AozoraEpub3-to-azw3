#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WritingMode {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageProgression {
    #[default]
    Default,
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    Default,
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenditionOrientation {
    Auto,
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenditionSpread {
    Auto,
    None,
    Landscape,
    Portrait,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenditionFlow {
    Auto,
    Paginated,
    ScrolledContinuous,
    ScrolledDoc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenditionAlign {
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSpread {
    Left,
    Right,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenditionSemantics {
    pub orientation: Option<RenditionOrientation>,
    pub spread: Option<RenditionSpread>,
    pub flow: Option<RenditionFlow>,
    pub align_x: Option<RenditionAlign>,
    pub page_spread: Option<PageSpread>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Layout {
    pub writing_mode: WritingMode,
    pub page_progression: PageProgression,
    pub direction: Direction,
}
