use std::time::{Duration, Instant};

// use log::*;
use protobuf::Message;

use embedded_hal::blocking::i2c::{Read, Write, WriteRead};

use embedded_ccs811::{mode::App, prelude::*, Ccs811Awake};

use crate::{
    api::{ListEntitiesSensorResponse, SensorStateResponse},
    components::{Component, ComponentUpdate},
    consts::MessageTypes,
    utils::*,
};

const UPDATE_TICK: Duration = Duration::from_secs(60);
const NAME: &str = "Rusty old CCS811";

pub struct CompCcs811<I2C> {
    key: u32,

    ccs811: Ccs811Awake<I2C, App>,

    last_update: Instant,
}

impl<E, I2C> CompCcs811<I2C>
where
    I2C: Read<Error = E> + Write<Error = E> + WriteRead<Error = E>,
{
    pub fn new(ccs811: Ccs811Awake<I2C, App>) -> CompCcs811<I2C> {
        // let ccs811 = ccs811.
        CompCcs811 {
            key: name_to_hash(NAME),
            ccs811,

            last_update: Instant::now(),
        }
    }

    fn get_key(&self) -> u32 {
        self.key
    }

    fn gen_resp(&mut self) -> Vec<ComponentUpdate> {
        match self.ccs811.data() {
            Ok(data) => {
                let mut resp = vec![];

                let mut eco2 = SensorStateResponse::new();
                eco2.set_key(self.get_key());
                eco2.set_state(data.eco2 as f32);
                resp.push(ComponentUpdate::SensorResponse(Box::new(eco2)));

                let mut etvoc = SensorStateResponse::new();
                etvoc.set_key(self.get_key() + 1);
                etvoc.set_state(data.etvoc as f32);
                resp.push(ComponentUpdate::SensorResponse(Box::new(etvoc)));

                return resp;
            }
            _ => {}
        }
        vec![]
    }
}

impl<E, I2C> Component for CompCcs811<I2C>
where
    I2C: Read<Error = E> + Write<Error = E> + WriteRead<Error = E>,
{
    fn get_description(&self) -> Vec<(MessageTypes, Box<dyn Message>)> {
        let mut resps: Vec<(MessageTypes, Box<dyn Message>)> = vec![];

        // eCO2
        let name = String::from(NAME) + " eCO2";
        let mut resp = ListEntitiesSensorResponse::new();
        resp.set_disabled_by_default(false);
        resp.set_key(self.get_key());
        resp.set_name(name.to_owned());
        resp.set_object_id(name_to_object(&name));
        resp.set_unique_id(name_to_unique(&name, "ccs811"));
        resp.set_unit_of_measurement(String::from("ppm"));

        resps.push((MessageTypes::ListEntitiesSensorResponse, Box::new(resp)));

        // etvoc
        let name = String::from(NAME) + " Total Volatile Organic Compound";
        let mut resp = ListEntitiesSensorResponse::new();
        resp.set_disabled_by_default(false);
        resp.set_key(self.get_key() + 1);
        resp.set_name(name.to_owned());
        resp.set_object_id(name_to_object(&name));
        resp.set_unique_id(name_to_unique(&name, "ccs811"));
        resp.set_unit_of_measurement(String::from("ppb"));

        resps.push((MessageTypes::ListEntitiesSensorResponse, Box::new(resp)));

        resps
    }

    fn handle_update(&mut self, msg: &ComponentUpdate) -> Vec<ComponentUpdate> {
        match msg {
            ComponentUpdate::Request(key) if key.is_none() || key == &Some(self.key) => {
                return self.gen_resp();
            }
            ComponentUpdate::Update => {
                let now = Instant::now();
                if now.duration_since(self.last_update) >= UPDATE_TICK {
                    self.last_update = now;
                    return self.gen_resp();
                }
            }
            _ => {}
        }
        vec![]
    }
}

// let dev = hal::I2cdev::new("/dev/i2c-1").unwrap();
// let nwake = hal::Pin::new(17);
// let delay = hal::Delay {};
// let address = SlaveAddr::default();
// let sensor = Ccs811::new(dev, address, nwake, delay);
// let mut sensor = sensor.start_application().ok().unwrap();
// sensor.set_mode(MeasurementMode::ConstantPower1s).unwrap();
// loop {
//     let data = block!(sensor.data()).unwrap();
//     println!("eCO2: {}, eTVOC: {}", data.eco2, data.etvoc);
// }
