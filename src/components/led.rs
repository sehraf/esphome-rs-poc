use protobuf::Message;

use embedded_hal::digital::v2::OutputPin;

use crate::{
    api::{ColorMode, LightStateResponse, ListEntitiesLightResponse},
    components::{BaseComponent, Component, ComponentUpdate},
    consts::LIST_ENTITIES_LIGHT_RESPONSE,
    utils::*,
};

pub struct Led<PIN> {
    base: BaseComponent,

    state: bool,
    pin: PIN,
}

impl<E, PIN> Led<PIN>
where
    PIN: OutputPin<Error = E>,
    E: std::fmt::Debug,
{
    pub fn new(name: String, pin: PIN) -> Led<PIN> {
        Led {
            base: BaseComponent::new(name),
            state: false,
            pin,
        }
    }

    fn get_key(&self) -> u32 {
        self.base.get_object_id_hash()
    }

    pub fn as_response(&self) -> Box<LightStateResponse> {
        let mut resp = LightStateResponse::new();
        resp.set_key(self.get_key());
        resp.set_state(self.state);
        Box::new(resp)
    }
}

impl<E, PIN> Component for Led<PIN>
where
    PIN: OutputPin<Error = E>,
    E: std::fmt::Debug,
{
    fn get_description(&self) -> Vec<(u32, Box<dyn Message>)> {
        let mut resp = ListEntitiesLightResponse::new();
        resp.set_disabled_by_default(false);
        resp.set_key(self.get_key());
        resp.set_name(self.base.get_name().to_owned());
        resp.set_object_id(self.base.get_object_id().to_owned());
        resp.set_unique_id(name_to_unique(&self.base.name, "led"));
        resp.set_supported_color_modes(vec![ColorMode::COLOR_MODE_ON_OFF]);

        vec![(
            LIST_ENTITIES_LIGHT_RESPONSE,
            Box::new(resp) as Box<dyn Message>,
        )]
    }

    fn handle_update(&mut self, msg: &ComponentUpdate) -> Vec<ComponentUpdate> {
        match msg {
            ComponentUpdate::Request(key) if key.is_none() || key == &Some(self.get_key()) => {
                return vec![ComponentUpdate::LightResponse(self.as_response())];
            }
            ComponentUpdate::LightRequest(req) if req.get_key() == self.get_key() => {
                if req.get_has_state() {
                    self.state = req.get_state();
                    if self.state {
                        self.pin.set_high().unwrap();
                    } else {
                        self.pin.set_low().unwrap();
                    }

                    return vec![ComponentUpdate::LightResponse(self.as_response())];
                }
            }
            _ => {}
        }
        vec![]
    }
}
