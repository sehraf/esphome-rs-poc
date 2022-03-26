#[derive(Debug, Clone, Copy)]
pub enum LightColor {
    Monochromatic(f32),
    Rgb(f32, f32, f32),
    Rgbw(f32, f32, f32, f32),
    Rgbww(f32, f32, f32, f32, f32),
}

#[allow(dead_code)]
impl LightColor {
    pub fn scale(&self, factor: f32) -> Self {
        // create a copy!
        match self {
            LightColor::Monochromatic(w) => (w * factor).into(),
            LightColor::Rgb(r, g, b) => (r * factor, g * factor, b * factor).into(),
            LightColor::Rgbw(r, g, b, w) => (r * factor, g * factor, b * factor, w * factor).into(),
            LightColor::Rgbww(r, g, b, w, ww) => {
                (r * factor, g * factor, b * factor, w * factor, ww * factor).into()
            }
        }
    }

    // getters
    pub fn get_red(&self) -> f32 {
        match self {
            LightColor::Monochromatic(_) => unreachable!("light has no \"red\""),
            LightColor::Rgb(r, _, _)
            | LightColor::Rgbw(r, _, _, _)
            | LightColor::Rgbww(r, _, _, _, _) => *r,
        }
    }

    pub fn get_green(&self) -> f32 {
        match self {
            LightColor::Monochromatic(_) => unreachable!("light has no \"green\""),
            LightColor::Rgb(_, g, _)
            | LightColor::Rgbw(_, g, _, _)
            | LightColor::Rgbww(_, g, _, _, _) => *g,
        }
    }

    pub fn get_blue(&self) -> f32 {
        match self {
            LightColor::Monochromatic(_) => unreachable!("light has no \"blue\""),
            LightColor::Rgb(_, _, b)
            | LightColor::Rgbw(_, _, b, _)
            | LightColor::Rgbww(_, _, b, _, _) => *b,
        }
    }

    pub fn get_white(&self) -> f32 {
        match self {
            LightColor::Rgb(_, _, _) => {
                unreachable!("light has no \"white\"")
            }
            LightColor::Monochromatic(w)
            | LightColor::Rgbw(_, _, _, w)
            | LightColor::Rgbww(_, _, _, w, _) => *w,
        }
    }

    pub fn get_warm_white(&self) -> f32 {
        match self {
            LightColor::Monochromatic(_)
            | LightColor::Rgb(_, _, _)
            | LightColor::Rgbw(_, _, _, _) => unreachable!("light has no \"warm white\""),
            LightColor::Rgbww(_, _, _, _, ww) => *ww,
        }
    }

    // setters
    pub fn set_red(&mut self, r_new: f32) {
        match self {
            LightColor::Monochromatic(_) => unreachable!("light has no \"red\""),
            LightColor::Rgb(r, _, _)
            | LightColor::Rgbw(r, _, _, _)
            | LightColor::Rgbww(r, _, _, _, _) => *r = r_new,
        }
    }

    pub fn set_green(&mut self, g_new: f32) {
        match self {
            LightColor::Monochromatic(_) => unreachable!("light has no \"green\""),
            LightColor::Rgb(_, g, _)
            | LightColor::Rgbw(_, g, _, _)
            | LightColor::Rgbww(_, g, _, _, _) => *g = g_new,
        }
    }

    pub fn set_blue(&mut self, b_new: f32) {
        match self {
            LightColor::Monochromatic(_) => unreachable!("light has no \"blue\""),
            LightColor::Rgb(_, _, b)
            | LightColor::Rgbw(_, _, b, _)
            | LightColor::Rgbww(_, _, b, _, _) => *b = b_new,
        }
    }

    pub fn set_white(&mut self, w_new: f32) {
        match self {
            LightColor::Rgb(_, _, _) => {
                unreachable!("light has no \"white\"")
            }
            LightColor::Monochromatic(w)
            | LightColor::Rgbw(_, _, _, w)
            | LightColor::Rgbww(_, _, _, w, _) => *w = w_new,
        }
    }

    pub fn set_warm_white(&mut self, ww_new: f32) {
        match self {
            LightColor::Monochromatic(_)
            | LightColor::Rgb(_, _, _)
            | LightColor::Rgbw(_, _, _, _) => unreachable!("light has no \"warm white\""),
            LightColor::Rgbww(_, _, _, _, ww) => *ww = ww_new,
        }
    }
}

impl From<(f32, f32, f32)> for LightColor {
    fn from(rgb: (f32, f32, f32)) -> Self {
        LightColor::Rgb(rgb.0, rgb.1, rgb.2)
    }
}

impl From<(f32, f32, f32, f32)> for LightColor {
    fn from(rgbw: (f32, f32, f32, f32)) -> Self {
        LightColor::Rgbw(rgbw.0, rgbw.1, rgbw.2, rgbw.3)
    }
}

impl From<(f32, f32, f32, f32, f32)> for LightColor {
    fn from(rgbww: (f32, f32, f32, f32, f32)) -> Self {
        LightColor::Rgbww(rgbww.0, rgbww.1, rgbww.2, rgbww.3, rgbww.4)
    }
}

impl From<f32> for LightColor {
    fn from(w: f32) -> Self {
        LightColor::Monochromatic(w)
    }
}
