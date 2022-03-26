use async_channel::Sender;
use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::api::*;

#[allow(unused_imports)]
use crate::{
    api::{ColorMode, LightStateResponse, ListEntitiesLightResponse},
    components::{BaseComponent, Component, ComponentUpdate},
    consts::LIST_ENTITIES_LIGHT_RESPONSE,
    utils::{light_color::LightColor, *},
};

static mut LOGGER: EspHomeLogger = EspHomeLogger { send: None };

pub struct EspHomeLogger {
    send: Option<Sender<ComponentUpdate>>,
}

unsafe impl Send for EspHomeLogger {}
unsafe impl Sync for EspHomeLogger {}

impl EspHomeLogger {
    pub fn initialize_default() {
        unsafe {
            ::log::set_logger(&LOGGER)
                .map(|()| LOGGER.initialize())
                .unwrap();
        }
    }

    pub fn initialize(&self) {
        ::log::set_max_level(self.get_max_level());
    }

    pub fn get_max_level(&self) -> LevelFilter {
        LevelFilter::Debug
    }

    pub fn set_send(send: Sender<ComponentUpdate>) {
        unsafe {
            LOGGER.send = Some(send);
        }
    }

    #[allow(dead_code)]
    fn get_marker(level: Level) -> &'static str {
        // static const char *const LOG_LEVEL_LETTERS[] = {
        //     "",    // NONE
        //     "E",   // ERROR
        //     "W",   // WARNING
        //     "I",   // INFO
        //     "C",   // CONFIG
        //     "D",   // DEBUG
        //     "V",   // VERBOSE
        //     "VV",  // VERY_VERBOSE
        // };
        match level {
            Level::Error => "E",
            Level::Warn => "W",
            Level::Info => "I",
            Level::Debug => "D",
            Level::Trace => "V",
        }
    }

    #[allow(dead_code)]
    fn get_color(level: Level) -> u8 {
        // #define ESPHOME_LOG_COLOR_BLACK "30"
        // #define ESPHOME_LOG_COLOR_RED "31"     // ERROR
        // #define ESPHOME_LOG_COLOR_GREEN "32"   // INFO
        // #define ESPHOME_LOG_COLOR_YELLOW "33"  // WARNING
        // #define ESPHOME_LOG_COLOR_BLUE "34"
        // #define ESPHOME_LOG_COLOR_MAGENTA "35"  // CONFIG
        // #define ESPHOME_LOG_COLOR_WHITE "38"

        match level {
            // #define ESPHOME_LOG_COLOR_CYAN "36"     // DEBUG
            Level::Debug => 36,
            // #define ESPHOME_LOG_COLOR_RED "31"     // ERROR
            Level::Error => 31, // LOG_COLOR_RED
            // #define ESPHOME_LOG_COLOR_GREEN "32"   // INFO
            Level::Info => 32, // LOG_COLOR_GREEN,
            // #define ESPHOME_LOG_COLOR_GRAY "37"     // VERBOSE
            Level::Trace => 37,
            // #define ESPHOME_LOG_COLOR_YELLOW "33"  // WARNING
            Level::Warn => 33, // LOG_COLOR_BROWN
        }
    }
}

impl Log for EspHomeLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.get_max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // server running?
            if let Some(send) = &self.send {
                // let color = match record.level() {
                //     Level::Debug => "36",
                //     Level::Error => "31", // LOG_COLOR_RED
                //     Level::Info => "32",  // LOG_COLOR_GREEN,
                //     Level::Trace => "37",
                //     Level::Warn => "33", // LOG_COLOR_BROWN
                // };
                // let marker = match record.level() {
                //     Level::Error => "E",
                //     Level::Warn => "W",
                //     Level::Info => "I",
                //     Level::Debug => "D",
                //     Level::Trace => "V",
                // };

                // this results in a stack overflow ...
                // let output = format!(
                //     "\x1b[0;{}m[{}] {}: {}\x1b[0m",
                //     Self::get_color(record.level()),
                //     Self::get_marker(record.level()),
                //     record.target(),
                //     record.args()
                // );
                // let output = format!(
                //     "\x1b[0;{}m[{}] {}\x1b[0m",
                //     Self::get_color(record.level()),
                //     Self::get_marker(record.level()),
                //     record.args()
                // );

                let output = format!(
                    "[{}] {}",
                    Self::get_marker(record.level()),
                    record.args()
                );

                let mut resp = SubscribeLogsResponse::new();
                resp.set_level(record.level().into());
                resp.set_message(output);

                smol::block_on(async {
                    send.send(ComponentUpdate::Log(Box::new(resp)))
                        .await
                        .expect("failed to send");
                })
            }

            // forward to ESP-IDF
            esp_idf_svc::log::EspLogger.log(record);
        }
    }

    fn flush(&self) {}
}

impl From<LogLevel> for LevelFilter {
    fn from(ll: LogLevel) -> Self {
        match ll {
            LogLevel::LOG_LEVEL_NONE => LevelFilter::Off,
            LogLevel::LOG_LEVEL_ERROR => LevelFilter::Error,
            LogLevel::LOG_LEVEL_WARN => LevelFilter::Warn,
            LogLevel::LOG_LEVEL_INFO => LevelFilter::Info,
            LogLevel::LOG_LEVEL_CONFIG => LevelFilter::Off,
            LogLevel::LOG_LEVEL_DEBUG => LevelFilter::Debug,
            LogLevel::LOG_LEVEL_VERBOSE => LevelFilter::Trace,
            LogLevel::LOG_LEVEL_VERY_VERBOSE => LevelFilter::Trace,
        }
    }
}

impl From<LevelFilter> for LogLevel {
    fn from(lf: LevelFilter) -> Self {
        match lf {
            LevelFilter::Off => LogLevel::LOG_LEVEL_NONE,
            LevelFilter::Error => LogLevel::LOG_LEVEL_ERROR,
            LevelFilter::Warn => LogLevel::LOG_LEVEL_WARN,
            LevelFilter::Info => LogLevel::LOG_LEVEL_INFO,
            LevelFilter::Debug => LogLevel::LOG_LEVEL_DEBUG,
            LevelFilter::Trace => LogLevel::LOG_LEVEL_VERY_VERBOSE,
        }
    }
}

impl From<Level> for LogLevel {
    fn from(lf: Level) -> Self {
        match lf {
            Level::Error => LogLevel::LOG_LEVEL_ERROR,
            Level::Warn => LogLevel::LOG_LEVEL_WARN,
            Level::Info => LogLevel::LOG_LEVEL_INFO,
            Level::Debug => LogLevel::LOG_LEVEL_DEBUG,
            Level::Trace => LogLevel::LOG_LEVEL_VERY_VERBOSE,
        }
    }
}
