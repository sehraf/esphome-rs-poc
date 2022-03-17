#[derive(Debug, Clone, Copy)]
pub struct Rgbw {
    red: f32,
    green: f32,
    blue: f32,
    #[allow(dead_code)]
    has_rgb: bool,

    white: f32,
    #[allow(dead_code)]
    has_white: bool,
}

#[allow(dead_code)]
impl Rgbw {
    pub fn scale(&self, factor: f32) -> Self {
        (
            self.red() * factor,
            self.green() * factor,
            self.blue() * factor,
            self.white() * factor,
        )
            .into()
    }

    pub fn set_red(&mut self, red: f32) {
        self.red = red;
    }
    pub fn set_green(&mut self, green: f32) {
        self.green = green;
    }
    pub fn set_blue(&mut self, blue: f32) {
        self.blue = blue;
    }
    pub fn set_white(&mut self, white: f32) {
        self.white = white;
    }

    pub fn red(&self) -> f32 {
        self.red
    }
    pub fn green(&self) -> f32 {
        self.green
    }
    pub fn blue(&self) -> f32 {
        self.blue
    }
    pub fn white(&self) -> f32 {
        self.white
    }
}

impl From<(f32, f32, f32)> for Rgbw {
    fn from(rgb: (f32, f32, f32)) -> Self {
        Rgbw {
            red: rgb.0,
            green: rgb.1,
            blue: rgb.2,
            has_rgb: true,
            white: 0f32,
            has_white: false,
        }
    }
}

impl From<(f32, f32, f32, f32)> for Rgbw {
    fn from(rgbw: (f32, f32, f32, f32)) -> Self {
        Rgbw {
            red: rgbw.0,
            green: rgbw.1,
            blue: rgbw.2,
            has_rgb: true,
            white: rgbw.3,
            has_white: true,
        }
    }
}

impl From<f32> for Rgbw {
    fn from(w: f32) -> Self {
        Rgbw {
            red: 0f32,
            green: 0f32,
            blue: 0f32,
            has_rgb: false,
            white: w,
            has_white: true,
        }
    }
}
