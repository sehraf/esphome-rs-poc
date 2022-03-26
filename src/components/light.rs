use embedded_hal::{digital::v2::OutputPin, PwmPin};
use esp_idf_sys::EspError;

use crate::{
    api::{ColorMode, LightStateResponse, ListEntitiesLightResponse},
    components::{BaseComponent, Component, ComponentUpdate},
    consts::LIST_ENTITIES_LIGHT_RESPONSE,
    utils::{light_color::LightColor, *},
};

use log::info;

enum LightPlatform {
    Binary {
        pin: Box<dyn OutputPin<Error = EspError>>,
    },
    Monochromatic {
        pin: Box<dyn PwmPin<Duty = u32>>,
        brightness: f32,
    },
    RGB {
        pin_r: Box<dyn PwmPin<Duty = u32>>,
        pin_g: Box<dyn PwmPin<Duty = u32>>,
        pin_b: Box<dyn PwmPin<Duty = u32>>,
        brightness: f32,
        color: LightColor,
    },
}

pub struct Light {
    base: BaseComponent,
    state: bool,
    platform: LightPlatform,
}

impl Light {
    #[allow(dead_code)]
    pub fn new_binary(name: String, pin: Box<dyn OutputPin<Error = EspError>>) -> Light {
        Light {
            base: BaseComponent::new(name),
            state: false,
            platform: LightPlatform::Binary { pin },
        }
    }

    #[allow(dead_code)]
    pub fn new_monochromatic(name: String, pin: Box<dyn PwmPin<Duty = u32>>) -> Light {
        Light {
            base: BaseComponent::new(name),
            state: false,
            platform: LightPlatform::Monochromatic {
                pin,
                brightness: 1.,
            },
        }
    }

    pub fn new_rgb(
        name: String,
        pins: (
            Box<dyn PwmPin<Duty = u32>>,
            Box<dyn PwmPin<Duty = u32>>,
            Box<dyn PwmPin<Duty = u32>>,
        ),
    ) -> Light {
        Light {
            base: BaseComponent::new(name),
            state: false,
            platform: LightPlatform::RGB {
                pin_r: pins.0,
                pin_g: pins.1,
                pin_b: pins.2,
                brightness: 1.,
                color: (1., 1., 1.).into(),
            },
        }
    }

    fn get_key(&self) -> u32 {
        self.base.get_object_id_hash()
    }

    fn as_response(&self) -> Box<LightStateResponse> {
        let mut resp = LightStateResponse::new();
        resp.set_key(self.get_key());
        resp.set_state(self.state);
        // if self.pin_rgb.is_some() {
        //     resp.set_red(self.rgbw.red());
        //     resp.set_green(self.rgbw.green());
        //     resp.set_blue(self.rgbw.blue());
        // }
        // resp.set_brightness(self.brightness);

        match self.platform {
            LightPlatform::Binary { .. } => (),
            LightPlatform::Monochromatic { brightness, .. }
            | LightPlatform::RGB { brightness, .. } => resp.set_brightness(brightness),
        }
        match self.platform {
            LightPlatform::Binary { .. } | LightPlatform::Monochromatic { .. } => (),
            LightPlatform::RGB { color, .. } => {
                resp.set_red(color.get_red());
                resp.set_green(color.get_green());
                resp.set_blue(color.get_blue());
            }
        }
        Box::new(resp)
    }
}

fn set_pwm(pin: &mut Box<dyn PwmPin<Duty = u32>>, brightness: f32) {
    let max_duty = pin.get_max_duty();
    let duty: u32 = (max_duty as f32 * brightness) as u32;
    pin.set_duty(duty);
}

impl Component for Light {
    fn get_description(&self) -> Vec<(u32, Box<dyn protobuf::Message>)> {
        // let modes = if self.pin_rgb.is_some() {
        //     vec![ColorMode::COLOR_MODE_BRIGHTNESS, ColorMode::COLOR_MODE_RGB]
        // } else {
        //     unreachable!("no pins configured, crashing!");
        // };

        let mut resp = ListEntitiesLightResponse::new();
        resp.set_disabled_by_default(false);
        resp.set_key(self.get_key());
        resp.set_name(self.base.get_name());
        resp.set_object_id(self.base.get_object_id());
        resp.set_unique_id(name_to_unique(&self.base.name, "light"));
        // resp.set_supported_color_modes(modes);
        match self.platform {
            LightPlatform::Binary { .. } => {
                resp.set_supported_color_modes(vec![ColorMode::COLOR_MODE_ON_OFF])
            }
            LightPlatform::Monochromatic { .. } => {
                resp.set_supported_color_modes(vec![ColorMode::COLOR_MODE_BRIGHTNESS])
            }
            LightPlatform::RGB { .. } => resp.set_supported_color_modes(vec![
                ColorMode::COLOR_MODE_BRIGHTNESS,
                ColorMode::COLOR_MODE_RGB,
            ]),
        }

        vec![(
            LIST_ENTITIES_LIGHT_RESPONSE,
            Box::new(resp) as Box<dyn protobuf::Message>,
        )]
    }

    fn handle_update(&mut self, msg: &ComponentUpdate) -> Vec<ComponentUpdate> {
        match msg {
            ComponentUpdate::Request(key) if key.is_none() || key == &Some(self.get_key()) => {
                return vec![ComponentUpdate::LightResponse(self.as_response())];
            }
            ComponentUpdate::LightRequest(req) if req.get_key() == self.get_key() => {
                // update state
                if req.get_has_state() {
                    self.state = req.get_state();
                }

                // brightness
                // if req.get_has_brightness() {
                //     self.brightness = req.get_brightness();
                // }
                if req.get_has_brightness() {
                    match &mut self.platform {
                        LightPlatform::Binary { .. } => unreachable!("light has no brightness"),
                        LightPlatform::Monochromatic { brightness, .. }
                        | LightPlatform::RGB { brightness, .. } => {
                            *brightness = req.get_brightness()
                        }
                    }
                }

                // update colors
                // if req.get_has_rgb() && self.pin_rgb.is_some() {
                //     self.rgbw.set_red(req.get_red());
                //     self.rgbw.set_green(req.get_green());
                //     self.rgbw.set_blue(req.get_blue());
                // }
                if req.get_has_rgb() {
                    match &mut self.platform {
                        LightPlatform::Binary { .. } | LightPlatform::Monochromatic { .. } => {
                            unreachable!("light has no color")
                        }
                        LightPlatform::RGB { color, .. } => {
                            color.set_red(req.get_red());
                            color.set_green(req.get_green());
                            color.set_blue(req.get_blue());
                        }
                    }
                }

                // set new values
                // if self.state {
                //     // scale
                //     let rgbw = self.rgbw.scale(self.brightness);
                //     if let Some(pins) = &mut self.pin_rgb {
                //         set_pwm(&mut pins.0, rgbw.red());
                //         set_pwm(&mut pins.1, rgbw.green());
                //         set_pwm(&mut pins.2, rgbw.blue());
                //     }
                // } else {
                //     // turn off
                //     if let Some(pins) = &mut self.pin_rgb {
                //         set_pwm(&mut pins.0, 0.);
                //         set_pwm(&mut pins.1, 0.);
                //         set_pwm(&mut pins.2, 0.);
                //     }
                // }
                match &mut self.platform {
                    LightPlatform::Binary { pin } => {
                        if self.state {
                            pin.set_high().unwrap();
                        } else {
                            pin.set_low().unwrap();
                        }
                    }
                    LightPlatform::Monochromatic { pin, brightness } => {
                        if self.state {
                            set_pwm(pin, *brightness);
                        } else {
                            set_pwm(pin, 0.);
                        }
                    }
                    LightPlatform::RGB {
                        pin_r,
                        pin_g,
                        pin_b,
                        brightness,
                        color,
                    } => {
                        let color = if self.state {
                            color.scale(*brightness)
                        } else {
                            (0., 0., 0.).into()
                        };

                        set_pwm(pin_r, color.get_red());
                        set_pwm(pin_g, color.get_green());
                        set_pwm(pin_b, color.get_blue());
                    }
                }

                return vec![ComponentUpdate::LightResponse(self.as_response())];
            }
            _ => {}
        }
        vec![]
    }
}
