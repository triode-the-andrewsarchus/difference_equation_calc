use egui::{Color32, Image, Response, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Undo,
    Redo,
    ArrowRight,
    Search,
    Alert,
    Save,
    Folder,
    Reset,
    Plus,
    Close,
    Pin,
    Waves,
}

impl Icon {
    #[inline]
    pub const fn uri(&self) -> &'static str {
        match self {
            Self::Undo => "bytes://icons/undo.svg",
            Self::Redo => "bytes://icons/redo.svg",
            Self::ArrowRight => "bytes://icons/arrow_right.svg",
            Self::Search => "bytes://icons/search.svg",
            Self::Alert => "bytes://icons/alert.svg",
            Self::Save => "bytes://icons/save.svg",
            Self::Folder => "bytes://icons/folder.svg",
            Self::Reset => "bytes://icons/rotate_ccw.svg",
            Self::Plus => "bytes://icons/plus.svg",
            Self::Close => "bytes://icons/x.svg",
            Self::Pin => "bytes://icons/pin.svg",
            Self::Waves => "bytes://icons/waves.svg",
        }
    }

    #[inline]
    pub const fn svg_bytes(&self) -> &'static [u8] {
        match self {
            Self::Undo => include_bytes!("../../assets/icons/undo.svg"),
            Self::Redo => include_bytes!("../../assets/icons/redo.svg"),
            Self::ArrowRight => include_bytes!("../../assets/icons/arrow_right.svg"),
            Self::Search => include_bytes!("../../assets/icons/search.svg"),
            Self::Alert => include_bytes!("../../assets/icons/alert.svg"),
            Self::Save => include_bytes!("../../assets/icons/save.svg"),
            Self::Folder => include_bytes!("../../assets/icons/folder.svg"),
            Self::Reset => include_bytes!("../../assets/icons/rotate_ccw.svg"),
            Self::Plus => include_bytes!("../../assets/icons/plus.svg"),
            Self::Close => include_bytes!("../../assets/icons/x.svg"),
            Self::Pin => include_bytes!("../../assets/icons/pin.svg"),
            Self::Waves => include_bytes!("../../assets/icons/waves.svg"),
        }
    }

    pub fn image(&self, size: f32) -> Image<'static> {
        Image::from_bytes(self.uri(), self.svg_bytes()).fit_to_exact_size(Vec2::splat(size))
    }

    pub fn image_tinted(&self, size: f32, tint: Color32) -> Image<'static> {
        self.image(size).tint(tint)
    }

    pub fn ui(&self, ui: &mut Ui, size: f32, tint: Color32) -> Response {
        ui.add(self.image_tinted(size, tint))
    }
}
