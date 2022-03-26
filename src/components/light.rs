use embedded_hal::PwmPin;

use crate::{
    api::{ColorMode, LightStateResponse, ListEntitiesLightResponse},
    components::{BaseComponent, Component, ComponentUpdate},
    consts::LIST_ENTITIES_LIGHT_RESPONSE,
    utils::{rgbw::Rgbw, *},
};

pub struct Light<PINR, PING, PINB> {
    base: BaseComponent,

    state: bool,
    rgbw: Rgbw,
    brightness: f32,

    pin_rgb: Option<(PINR, PING, PINB)>,
}

impl<PINR, PING, PINB> Light<PINR, PING, PINB>
where
    PINR: PwmPin,
    PING: PwmPin,
    PINB: PwmPin,
{
    pub fn new(name: String, pins: (PINR, PING, PINB)) -> Light<PINR, PING, PINB> {
        Light {
            base: BaseComponent::new(name),
            state: false,

            rgbw: (1., 1., 1.).into(),
            brightness: 1.,

            pin_rgb: Some(pins),
        }
    }

    fn get_key(&self) -> u32 {
        self.base.get_object_id_hash()
    }

    fn as_response(&self) -> Box<LightStateResponse> {
        let mut resp = LightStateResponse::new();
        resp.set_key(self.get_key());
        resp.set_state(self.state);
        if self.pin_rgb.is_some() {
            resp.set_red(self.rgbw.red());
            resp.set_green(self.rgbw.green());
            resp.set_blue(self.rgbw.blue());
        }
        resp.set_brightness(self.brightness);
        Box::new(resp)
    }
}

fn set_pwm(pin: &mut impl PwmPin<Duty = u32>, brightness: f32) {
    let max_duty = pin.get_max_duty();
    let duty: u32 = (max_duty as f32 * brightness) as u32;
    pin.set_duty(duty);
}

impl<PINR, PING, PINB> Component for Light<PINR, PING, PINB>
where
    PINR: PwmPin<Duty = u32>,
    PING: PwmPin<Duty = u32>,
    PINB: PwmPin<Duty = u32>,
{
    fn get_description(&self) -> Vec<(u32, Box<dyn protobuf::Message>)> {
        let modes = if self.pin_rgb.is_some() {
            vec![ColorMode::COLOR_MODE_BRIGHTNESS, ColorMode::COLOR_MODE_RGB]
        } else {
            unreachable!("no pins configured, crashing!");
        };

        let mut resp = ListEntitiesLightResponse::new();
        resp.set_disabled_by_default(false);
        resp.set_key(self.get_key());
        resp.set_name(self.base.get_name());
        resp.set_object_id(self.base.get_object_id());
        resp.set_unique_id(name_to_unique(&self.base.name, "light"));
        resp.set_supported_color_modes(modes);

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
                // update colors
                if req.get_has_rgb() && self.pin_rgb.is_some() {
                    self.rgbw.set_red(req.get_red());
                    self.rgbw.set_green(req.get_green());
                    self.rgbw.set_blue(req.get_blue());
                }
                // brightness
                if req.get_has_brightness() {
                    self.brightness = req.get_brightness();
                }

                // set new values
                if self.state {
                    // scale
                    let rgbw = self.rgbw.scale(self.brightness);
                    if let Some(pins) = &mut self.pin_rgb {
                        set_pwm(&mut pins.0, rgbw.red());
                        set_pwm(&mut pins.1, rgbw.green());
                        set_pwm(&mut pins.2, rgbw.blue());
                    }
                } else {
                    // turn off
                    if let Some(pins) = &mut self.pin_rgb {
                        set_pwm(&mut pins.0, 0.);
                        set_pwm(&mut pins.1, 0.);
                        set_pwm(&mut pins.2, 0.);
                    }
                }

                return vec![ComponentUpdate::LightResponse(self.as_response())];
            }
            _ => {}
        }
        vec![]
    }
}
