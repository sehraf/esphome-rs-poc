#[allow(unused_imports)]
use log::*;
use std::net::TcpStream;
use std::sync::Arc;

use esp_idf_hal::prelude::*;
use esp_idf_hal::{
    i2c,
    ledc::{config::TimerConfig, Channel, Timer},
};

use protobuf::Message;

use crate::api::*;

// crate is broken
// pub mod bme280;

// never worked
// pub mod ccs811;

pub mod led;
pub mod light;

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

    Log(Box<SubscribeLogsResponse>)
}

/// Ownder and manager of all (hardware) components
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
        let i2c_bus = i2c::Master::new(p.i2c0, pins, config).expect("failed to aquire i2c");

        // #######################################
        // # BME280
        // #######################################

        // // initialize the BME280 using the primary I2C address 0x76
        // let mut bme280 = ::bme280::BME280::new_primary(i2c_bus.clone(), esp_idf_hal::delay::Ets);
        // match bme280.init() {
        //     Ok(()) => {
        //         info!("BME280 initialized");

        //         match bme280.measure() {
        //             Ok(mes) => {
        //                 info!("measured {:.1}°C", mes.temperature);
        //                 info!("measured {:.0}hPa", mes.pressure);
        //                 info!("measured {:.2}%", mes.humidity);

        //                 let bme280 = bme280::Bme280::new(bme280);
        //                 let bme280 = Box::new(bme280);
        //                 components.push(bme280);
        //             }
        //             Err(err) => {
        //                 info!("failed to measure: {:?}", err);
        //                 match err {
        //                     ::bme280::Error::CompensationFailed
        //                     | ::bme280::Error::InvalidData
        //                     | ::bme280::Error::NoCalibrationData
        //                     | ::bme280::Error::UnsupportedChip => {}
        //                     ::bme280::Error::I2c(err) => info!("I2c error: {}", err),
        //                 }
        //             }
        //         }
        //     }
        //     Err(err) => {
        //         info!("BME280 failed to initialized: {:?}", err);
        //         match err {
        //             ::bme280::Error::CompensationFailed
        //             | ::bme280::Error::InvalidData
        //             | ::bme280::Error::NoCalibrationData
        //             | ::bme280::Error::UnsupportedChip => {}
        //             ::bme280::Error::I2c(err) => info!("I2c error: {}", err),
        //         }
        //     }
        // }

        // #######################################
        // # CCS811
        // #######################################
        // let ccs811 = Ccs811Awake::new(i2c_bus, SlaveAddr::Default);
        // let mut ccs811 = ccs811.start_application().ok().unwrap();
        // ccs811.set_mode(MeasurementMode::ConstantPower1s).unwrap();
        // unsafe {
        //     ets_delay_us(1000);
        // }

        // match ccs811.data() {
        //     Ok(data) => {
        //         info!("CCS811 initialized");
        //         info!("eCO2: {}, eTVOC: {}", data.eco2, data.etvoc);

        //         let ccs811 = ccs811::CompCcs811::new(ccs811);
        //         let ccs811 = Box::new(ccs811);
        //         components.push(ccs811);
        //     }
        //     Err(err) => {
        //         info!("CCS811 failed to initialized: {:?}", err);
        //     }
        // }

        // #######################################
        // # LED - GPIO9
        // #######################################
        let pin = gpio_out!(p, gpio9);
        let led = led::Led::new(pin);
        let led = Box::new(led);
        components.push(led);

        // #######################################
        // # RGB - GPIO3 + GPIO4 + GPIO5
        // #######################################
        let pin_r = gpio_out!(p, gpio3);
        let pin_g = gpio_out!(p, gpio4);
        let pin_b = gpio_out!(p, gpio5);

        let channel_r = Channel::new(p.ledc.channel0, timer.clone(), pin_r)
            .expect("failed to setup chhannel r");
        let channel_g = Channel::new(p.ledc.channel1, timer.clone(), pin_g)
            .expect("failed to setup chhannel g");
        let channel_b = Channel::new(p.ledc.channel2, timer.clone(), pin_b)
            .expect("failed to setup chhannel b");

        let light = light::Light::new((channel_r, channel_g, channel_b));
        let light = Box::new(light);
        components.push(light);

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
