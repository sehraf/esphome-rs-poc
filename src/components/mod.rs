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

pub mod light;
pub mod logger;

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
    ($peripherals:expr, $gpio:ident) => {
        $peripherals
            .pins
            .$gpio
            .into_output()
            .expect("failed to acquire pin")
    };
}

/// Small helper for getting a GPIO as input_output
#[allow(unused_macros)]
macro_rules! gpio_in_out {
    ($peripherals:expr, $gpio:ident) => {
        $peripherals
            .pins
            .$gpio
            .into_input_output()
            .expect("failed to acquire pin")
    };
}

#[allow(unused_macros)]
macro_rules! make_light_binbary {
    ($name: expr, $peripherals:expr, $gpio:ident, $components:expr) => {
        // get pin, boxed
        let pin = Box::new(gpio_out!($peripherals, $gpio));
        // create light, boxed
        let light = Box::new(light::Light::new_binary($name, pin));
        // add to components
        $components.push(light);
    };
}

macro_rules! ledc_channel {
    ($peripherals:expr, $gpio:ident, $channel:ident, $timer: expr) => {
        Channel::new(
            $peripherals.ledc.$channel,
            $timer.clone(),
            gpio_out!($peripherals, $gpio),
        )
        .expect("failed to setup chhannel")
    };
}

#[allow(unused_macros)]
macro_rules! make_light_monochromatic {
    ($name: expr, $peripherals:expr, $gpio:ident, $channel:ident, $timer: expr, $components:expr) => {
        let channel = Box::new(ledc_channel!($peripherals, $gpio, $channel, $timer));
        // create light, boxed
        let light = Box::new(light::Light::new_monochromatic($name, channel));
        // add to components
        $components.push(light);
    };
}

#[allow(unused_macros)]
macro_rules! make_light_rgb {
    ($name: expr, $peripherals:expr, $gpio_r:ident, $gpio_g:ident, $gpio_b:ident, $channel_r:ident, $channel_g:ident, $channel_b:ident, $timer: expr, $components:expr) => {
        // get channels
        let channel_r = Box::new(ledc_channel!($peripherals, $gpio_r, $channel_r, $timer));
        let channel_g = Box::new(ledc_channel!($peripherals, $gpio_g, $channel_g, $timer));
        let channel_b = Box::new(ledc_channel!($peripherals, $gpio_b, $channel_b, $timer));

        // create light, boxed
        let light = Box::new(light::Light::new_rgb(
            $name,
            (channel_r, channel_g, channel_b),
        ));
        // add to components
        $components.push(light);
    };
}

impl ComponentManager {
    pub fn new() -> ComponentManager {
        // Preripherals live here
        let peripherals =
            esp_idf_hal::peripherals::Peripherals::take().expect("Failed to obtain Peripherals");
        // time for whoever needs it
        let timer = Arc::new(
            Timer::new(
                peripherals.ledc.timer0,
                &TimerConfig::default().frequency(25.kHz().into()),
            )
            .expect("failed to setup timer"),
        );

        let mut components: Vec<Box<dyn Component>> = vec![];

        // #######################################
        // # I2C - GPIO0 + GPIO2
        // #######################################
        let pins = i2c::MasterPins {
            scl: gpio_in_out!(peripherals, gpio0),
            sda: gpio_in_out!(peripherals, gpio2),
        };
        let config = i2c::config::MasterConfig::default();
        #[allow(unused_variables)]
        // cannot be shared at the moment
        let i2c_bus =
            i2c::Master::new(peripherals.i2c0, pins, config).expect("failed to aquire i2c");

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
        // # LEDs - GPIO9, GPIO18, GPIO19
        // #######################################
        {
            const NAME: &str = "Rusty old LED";

            // GPIO LED (blue)
            make_light_monochromatic!(
                NAME.to_owned() + " " + "blue",
                peripherals,
                gpio9,
                channel3,
                timer,
                components
            );
            // LED Warm (yellow)
            make_light_binbary!(
                NAME.to_owned() + " " + "yellow",
                peripherals,
                gpio18,
                components
            );
            // LED Cold (white)
            make_light_binbary!(
                NAME.to_owned() + " " + "white",
                peripherals,
                gpio19,
                components
            );
        }

        // #######################################
        // # RGB - GPIO3 + GPIO4 + GPIO5
        // #######################################
        {
            const NAME: &str = "Rusty old RGB Light";

            // build in RGB LED
            make_light_rgb!(
                NAME.to_owned() + " " + "onboard",
                peripherals,
                gpio3,
                gpio4,
                gpio5,
                channel0,
                channel1,
                channel2,
                timer,
                components
            );
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
