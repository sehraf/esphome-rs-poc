#[allow(unused_imports)]
use log::*;
use std::{net::TcpStream, sync::Arc};

use esp_idf_hal::prelude::*;
#[allow(unused_imports)]
use esp_idf_hal::{
    i2c,
    ledc::{config::TimerConfig, Channel, Timer},
};

#[cfg(feature = "ccs811")]
use embedded_ccs811::{
    nb::block, Ccs811AppMode, Ccs811Awake, Ccs811BootMode, MeasurementMode, SlaveAddr,
};

use protobuf::Message;

use crate::{api::*, utils::*};

// crate is broken
#[cfg(feature = "has_bme280")]
pub mod bme280;

// never worked
#[cfg(feature = "has_ccs811")]
pub mod ccs811;

// pub mod led;
pub mod light;

pub struct BaseComponent {
    name: String,
    key: u32,
    object_id: String,
    // unique_id: String,
}

#[allow(dead_code)]
impl BaseComponent {
    pub fn new(name: String) -> Self {
        let key = name_to_hash(&name);
        let object_id = name_to_object(&name);
        BaseComponent {
            key,
            name,
            object_id,
            // unique_id: (),
        }
    }

    // https://github.com/esphome/esphome/blob/3c0414c42027d8cc3cab8e59c878116f62d8fac7/esphome/core/entity_base.h#L21
    pub fn get_name(&self) -> String {
        self.name.to_owned()
    }

    // https://github.com/esphome/esphome/blob/3c0414c42027d8cc3cab8e59c878116f62d8fac7/esphome/core/entity_base.h#L25
    pub fn get_object_id(&self) -> String {
        self.object_id.to_owned()
    }

    // https://github.com/esphome/esphome/blob/3c0414c42027d8cc3cab8e59c878116f62d8fac7/esphome/core/entity_base.h#L28
    pub fn get_object_id_hash(&self) -> u32 {
        self.key
    }
}

pub trait Component {
    fn handle_update(&mut self, msg: &ComponentUpdate) -> Vec<ComponentUpdate>;

    fn get_description(&self) -> Vec<(u32, Box<dyn Message>)>;
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ComponentUpdate {
    /// Request an update of a specific or all components
    Request(Option<u32>),
    /// Send a tick to all modules that can then decide whether to generate an update or not
    Update,

    /// Client is connecting, `Arc` is required for `Clone`, thoguh is should not be used
    Connection(Arc<smol::Async<TcpStream>>),
    /// Client is closing the connection
    Closing,

    /// Component related values
    LightRequest(Box<LightCommandRequest>),
    LightResponse(Box<LightStateResponse>),

    SensorResponse(Box<SensorStateResponse>),

    Log(Box<SubscribeLogsResponse>),
}

/// Owner and manager of all (hardware) components
///
/// Takes care of forwaring incoming requests as well as regularly ticks all components.
pub struct ComponentManager {
    components: Vec<Box<dyn Component>>,
}

/// Small helper for getting a GPIO as output
macro_rules! gpio_out {
    ($peripherals:ident, $gpio:ident) => {
        $peripherals
            .pins
            .$gpio
            .into_output()
            .expect("failed to aquire pin")
    };
}

/// Small helper for getting a GPIO as input_output
#[allow(unused_macros)]
macro_rules! gpio_in_out {
    ($peripherals:ident, $gpio:ident) => {
        $peripherals
            .pins
            .$gpio
            .into_input_output()
            .expect("failed to aquire pin")
    };
}

impl ComponentManager {
    pub fn new() -> ComponentManager {
        // Preripherals live here
        let p =
            esp_idf_hal::peripherals::Peripherals::take().expect("Failed to obtain Peripherals");
        // time for whoever needs it
        let timer = Arc::new(
            Timer::new(
                p.ledc.timer0,
                &TimerConfig::default().frequency(25.kHz().into()),
            )
            .expect("failed to setup timer"),
        );

        let mut components: Vec<Box<dyn Component>> = vec![];

        // #######################################
        // # I2C - GPIO0 + GPIO2
        // #######################################
        let pins = i2c::MasterPins {
            scl: gpio_in_out!(p, gpio0),
            sda: gpio_in_out!(p, gpio2),
        };
        let config = i2c::config::MasterConfig::default();
        #[allow(unused_variables)]
        // cannot be shared at the moment
        let i2c_bus = i2c::Master::new(p.i2c0, pins, config).expect("failed to aquire i2c");

        // #######################################
        // # BME280
        // #######################################
        #[cfg(feature = "has_bme280")]
        {
            // initialize the BME280 using the primary I2C address 0x76
            let mut bme280 =
                ::bme280::BME280::new_primary(i2c_bus.clone(), esp_idf_hal::delay::Ets);
            match bme280.init() {
                Ok(()) => {
                    info!("BME280 initialized");

                    match bme280.measure() {
                        Ok(mes) => {
                            info!("measured {:.1}°C", mes.temperature);
                            info!("measured {:.0}hPa", mes.pressure);
                            info!("measured {:.2}%", mes.humidity);

                            let bme280 = bme280::Bme280::new(bme280);
                            let bme280 = Box::new(bme280);
                            components.push(bme280);
                        }
                        Err(err) => {
                            info!("failed to measure: {:?}", err);
                            match err {
                                ::bme280::Error::CompensationFailed
                                | ::bme280::Error::InvalidData
                                | ::bme280::Error::NoCalibrationData
                                | ::bme280::Error::UnsupportedChip => {}
                                ::bme280::Error::I2c(err) => info!("I2c error: {}", err),
                            }
                        }
                    }
                }
                Err(err) => {
                    info!("BME280 failed to initialized: {:?}", err);
                    match err {
                        ::bme280::Error::CompensationFailed
                        | ::bme280::Error::InvalidData
                        | ::bme280::Error::NoCalibrationData
                        | ::bme280::Error::UnsupportedChip => {}
                        ::bme280::Error::I2c(err) => info!("I2c error: {}", err),
                    }
                }
            }
        }

        // #######################################
        // # CCS811
        // #######################################
        #[cfg(feature = "has_ccs811")]
        {
            let ccs811 = Ccs811Awake::new(i2c_bus, SlaveAddr::Default);
            let mut ccs811 = ccs811.start_application().ok().unwrap();
            ccs811.set_mode(MeasurementMode::ConstantPower1s).unwrap();

            // std::thread::sleep(Duration::from_millis(100));

            let res = block!(ccs811.data());
            match res {
                Ok(data) => {
                    info!("CCS811 initialized");
                    info!("eCO2: {}, eTVOC: {}", data.eco2, data.etvoc);

                    let ccs811 = ccs811::CompCcs811::new(ccs811);
                    let ccs811 = Box::new(ccs811);
                    components.push(ccs811);
                }
                Err(err) => {
                    info!("CCS811 failed to initialized: {:?}", err);
                }
            }
        }

        // #######################################
        // # LED - GPIO9
        // #######################################
        {
            const NAME: &str = "Rusty old LED";

            let pin_blue = Box::new(gpio_out!(p, gpio9)); // GPIO LED (blue)
            components.push(Box::new(light::Light::new_binary(
                NAME.to_owned() + " " + "blue",
                pin_blue,
            )));
            let pin_w = Box::new(gpio_out!(p, gpio18)); // LED Warm (yellow)
            components.push(Box::new(light::Light::new_binary(
                NAME.to_owned() + " " + "yellow",
                pin_w,
            )));
            let pin_c = Box::new(gpio_out!(p, gpio19)); // LED Cold (white)
            components.push(Box::new(light::Light::new_binary(
                NAME.to_owned() + " " + "white",
                pin_c,
            )));
        }

        // #######################################
        // # RGB - GPIO3 + GPIO4 + GPIO5
        // #######################################
        {
            const NAME: &str = "Rusty old RGB Light";

            let pin_r = gpio_out!(p, gpio3);
            let pin_g = gpio_out!(p, gpio4);
            let pin_b = gpio_out!(p, gpio5);

            let channel_r = Box::new(
                Channel::new(p.ledc.channel0, timer.clone(), pin_r)
                    .expect("failed to setup chhannel r"),
            );
            let channel_g = Box::new(
                Channel::new(p.ledc.channel1, timer.clone(), pin_g)
                    .expect("failed to setup chhannel g"),
            );
            let channel_b = Box::new(
                Channel::new(p.ledc.channel2, timer.clone(), pin_b)
                    .expect("failed to setup chhannel b"),
            );

            let light = light::Light::new_rgb(NAME.to_owned(), (channel_r, channel_g, channel_b));
            let light = Box::new(light);
            components.push(light);
        }

        ComponentManager { components }
    }

    pub fn hanlde(&mut self, msg: &ComponentUpdate) -> Vec<ComponentUpdate> {
        let mut resp = vec![];

        for comp in &mut self.components {
            for comp_resp in comp.handle_update(&msg) {
                resp.push(comp_resp);
            }
        }

        resp
    }

    pub fn get_descriptions(&mut self) -> Vec<(u32, Box<dyn Message>)> {
        let mut ret = vec![];

        for comp in &self.components {
            ret.append(&mut comp.get_description());
        }

        ret
    }
}
