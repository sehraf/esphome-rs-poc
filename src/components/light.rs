use embedded_hal::PwmPin;

use crate::{
    api::{ColorMode, LightStateResponse, ListEntitiesLightResponse},
    components::{Component, ComponentUpdate},
    consts::LIST_ENTITIES_LIGHT_RESPONSE,
    utils::{rgbw::Rgbw, *},
};

const NAME: &str = "Rusty old RGB Light";

pub struct Light<PINR, PING, PINB> {
    key: u32,

    state: bool,
    rgbw: Rgbw,
    brightness: f32,

    pin_rgb: Option<(PINR, PING, PINB)>,
}

// impl<C: HwChannel, H: HwTimer, T: Borrow<Timer<H>>, P: OutputPin> embedded_hal_0_2::PwmPin
//     for Channel<C, H, T, P>

// impl<E, H, T, PINR, PING, PINB> Light<PINR, PING, PINB>
impl<PINR, PING, PINB> Light<PINR, PING, PINB>
where
    // PR: OutputPin<Error = E>,
    // PG: OutputPin<Error = E>,
    // PB: OutputPin<Error = E>,
    // PINR: Channel<CR, H, T, PR>,
    // PING: Channel<CG, H, T, PG>,
    // PINB: Channel<CB, H, T, PB>,
    // E: std::fmt::Debug,
    // H: HwTimer,
    // T: Borrow<Timer<H>>,
    PINR: PwmPin,
    PING: PwmPin,
    PINB: PwmPin,
{
    pub fn new(pins: (PINR, PING, PINB)) -> Light<PINR, PING, PINB> {
        //! Create a 25 kHz PWM signal with 75 % duty cycle on GPIO 1
        //! ```
        //!
        //! let max_duty = channel.get_max_duty()?;
        //! channel.set_duty(max_duty * 3 / 4);

        Light {
            key: name_to_hash(NAME),
            state: false,

            rgbw: (1f32, 1f32, 1f32).into(),
            brightness: 1f32,

            pin_rgb: Some(pins),
            // pin_w: None,
        }
    }

    fn get_key(&self) -> u32 {
        self.key
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
    // PINR: OutputPin<Error = E>,
    // PING: OutputPin<Error = E>,
    // PINB: OutputPin<Error = E>,
    // // PINW: OutputPin<Error = EspError>,
    // E: std::fmt::Debug,
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
        resp.set_key(self.key);
        resp.set_name(String::from(NAME));
        resp.set_object_id(name_to_object(NAME));
        resp.set_unique_id(name_to_unique(NAME, "light"));
        resp.set_supported_color_modes(modes);

        vec![(LIST_ENTITIES_LIGHT_RESPONSE, Box::new(resp))]
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
                    let rgbw = self.rgbw.scale(self.brightness);
                    if let Some(pins) = &mut self.pin_rgb {
                        // XXX no PWM for now
                        // pins.0
                        //     .set_state((rgbw.red() > 0.5).into())
                        //     .expect("failed to set pin");
                        // pins.1
                        //     .set_state((rgbw.green() > 0.5).into())
                        //     .expect("failed to set pin");
                        // pins.2
                        //     .set_state((rgbw.blue() > 0.5).into())
                        //     .expect("failed to set pin");
                        set_pwm(&mut pins.0, rgbw.red());
                        set_pwm(&mut pins.1, rgbw.green());
                        set_pwm(&mut pins.2, rgbw.blue());
                    }
                } else {
                    if let Some(pins) = &mut self.pin_rgb {
                        // pins.0.set_low().expect("failed to set pin");
                        // pins.1.set_low().expect("failed to set pin");
                        // pins.2.set_low().expect("failed to set pin");
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
