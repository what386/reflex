const COMPACT_BREAKPOINT: f32 = 840.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LayoutMode {
    Wide,
    Compact,
}

impl LayoutMode {
    pub(super) fn for_width(width: f32) -> Self {
        if width < COMPACT_BREAKPOINT {
            Self::Compact
        } else {
            Self::Wide
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LayoutMode;

    #[test]
    fn selects_compact_layout_below_the_breakpoint() {
        assert_eq!(LayoutMode::for_width(480.0), LayoutMode::Compact);
        assert_eq!(LayoutMode::for_width(839.0), LayoutMode::Compact);
        assert_eq!(LayoutMode::for_width(840.0), LayoutMode::Wide);
        assert_eq!(LayoutMode::for_width(1_000.0), LayoutMode::Wide);
    }
}
