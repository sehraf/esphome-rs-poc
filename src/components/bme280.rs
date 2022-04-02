use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use log::*;
use protobuf::Message;

use embedded_hal::blocking::{
    delay::DelayMs,
    i2c::{Read, Write, WriteRead},
};

use bme280::BME280;

use crate::{
    api::{ListEntitiesSensorResponse, SensorStateResponse},
    components::{Component, ComponentUpdate},
    consts::MessageTypes,
    utils::*,
};

const UPDATE_TICK: Duration = Duration::from_secs(60);
const NAME: &str = "Rusty old BME280";

pub struct Bme280<I2C, D> {
    key: u32,

    bme: BME280<I2C, D>,

    last_update: Instant,
}

impl<E, I2C, D> Bme280<I2C, D>
where
    I2C: Read<Error = E> + Write<Error = E> + WriteRead<Error = E>,
    D: DelayMs<u8>,
{
    pub fn new(bme: BME280<I2C, D>) -> Bme280<I2C, D> {
        Bme280 {
            key: name_to_hash(NAME),
            bme,

            last_update: Instant::now(),
        }
    }

    fn get_key(&self) -> u32 {
        self.key
    }

    fn gen_resp(&mut self) -> Vec<ComponentUpdate> {
        match self.bme.measure() {
            Ok(mes) => {
                trace!("measured {:.1}°C", mes.temperature);
                trace!("measured {:.0}hPa", mes.pressure / 100.);
                trace!("measured {:.2}%", mes.humidity);

                let mut resp = vec![];

                let mut temp = SensorStateResponse::new();
                temp.set_key(self.get_key());
                temp.set_state(mes.temperature);
                resp.push(ComponentUpdate::Response((
                    MessageTypes::SensorStateResponse,
                    Arc::new(Box::new(temp)),
                )));

                let mut humi = SensorStateResponse::new();
                humi.set_key(self.get_key() + 1);
                humi.set_state(mes.humidity);
                resp.push(ComponentUpdate::Response((
                    MessageTypes::SensorStateResponse,
                    Arc::new(Box::new(humi)),
                )));

                let mut pres = SensorStateResponse::new();
                pres.set_key(self.get_key() + 2);
                pres.set_state(mes.pressure / 100.); // mes.pressure is in Pa
                resp.push(ComponentUpdate::Response((
                    MessageTypes::SensorStateResponse,
                    Arc::new(Box::new(pres)),
                )));

                return resp;
            }
            _ => vec![],
        }
    }
}

impl<E, I2C, D> Component for Bme280<I2C, D>
where
    I2C: Read<Error = E> + Write<Error = E> + WriteRead<Error = E>,
    D: DelayMs<u8>,
{
    fn get_description(&self) -> Vec<(MessageTypes, Arc<Box<dyn Message>>)> {
        let mut resps: Vec<(MessageTypes, Arc<Box<dyn Message>>)> = vec![];

        // Temperatur
        let name = String::from(NAME) + " Temperatur";
        let mut resp = ListEntitiesSensorResponse::new();
        resp.set_disabled_by_default(false);
        resp.set_key(self.get_key());
        resp.set_name(name.to_owned());
        resp.set_object_id(name_to_object(&name));
        resp.set_unique_id(name_to_unique(&name, "bme280"));
        resp.set_unit_of_measurement(String::from("°C"));
        resp.set_accuracy_decimals(1);

        resps.push((
            MessageTypes::ListEntitiesSensorResponse,
            Arc::new(Box::new(resp)),
        ));

        // Humidity
        let name = String::from(NAME) + " Humidity";
        let mut resp = ListEntitiesSensorResponse::new();
        resp.set_disabled_by_default(false);
        resp.set_key(self.get_key() + 1);
        resp.set_name(name.to_owned());
        resp.set_object_id(name_to_object(&name));
        resp.set_unique_id(name_to_unique(&name, "bme280"));
        resp.set_unit_of_measurement(String::from("%"));
        resp.set_accuracy_decimals(1);

        resps.push((
            MessageTypes::ListEntitiesSensorResponse,
            Arc::new(Box::new(resp)),
        ));

        // Preasure
        let name = String::from(NAME) + " Preasure";
        let mut resp = ListEntitiesSensorResponse::new();
        resp.set_disabled_by_default(false);
        resp.set_key(self.get_key() + 2);
        resp.set_name(name.to_owned());
        resp.set_object_id(name_to_object(&name));
        resp.set_unique_id(name_to_unique(&name, "bme280"));
        resp.set_unit_of_measurement(String::from("hPa"));
        resp.set_accuracy_decimals(1);

        resps.push((
            MessageTypes::ListEntitiesSensorResponse,
            Arc::new(Box::new(resp)),
        ));

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
