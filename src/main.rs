#![allow(clippy::single_component_path_imports)]

use std::sync::Arc;
use std::time::Duration;

use anyhow::*;
use async_net::Ipv4Addr;
use consts::MessageTypes;
// use esp_idf_svc::log::EspLogger;
use log::*;

use protobuf::Message;

use smol;

// use embedded_svc::anyerror::*;
use embedded_svc::ping::Ping;
use embedded_svc::wifi::*;

use esp_idf_svc::netif::*;
use esp_idf_svc::nvs::*;
use esp_idf_svc::ping::*;
use esp_idf_svc::sysloop::*;
use esp_idf_svc::wifi::*;

const MAC: &str = "<MAC>"; // ESP32-C3-13
// const SSID: &str = env!("RUST_ESP32_STD_HELLO_WIFI_SSID");
// const PASS: &str = env!("RUST_ESP32_STD_HELLO_WIFI_PASS");
const SSID: &str = "<SSID>";
const PASS: &str = "<PASS>";

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const NAME: &'static str = env!("CARGO_PKG_NAME");
const MODEL: &str = "ESP32 DevKit";
const PROJECT_SUFFIX: &str = "Example";

const CLIENT_PW: &str = "test1234"; // empty for none

const PORT: u16 = 6053;

mod api;
mod client;
mod components;
mod consts;

mod server;
mod utils;

use components::ComponentManager;

pub struct Device {
    pub mac: String,

    pub model: String,
    pub name: String,
    pub project_name: String,
    pub project_version: String,
    pub server_name: String,

    pub password: String,

    pub component_description: Vec<(MessageTypes, Box<dyn Message>)>,
}

fn main() -> Result<()> {
    // temporary hack
    esp_idf_sys::link_patches();

    // Get backtraces from anyhow; only works for Xtensa arch currently
    #[cfg(target_arch = "xtensa")]
    env::set_var("RUST_BACKTRACE", "1");

    // setup vfs and event fd
    esp_idf_sys::esp!(unsafe {
        #[allow(clippy::needless_update)]
        esp_idf_sys::esp_vfs_eventfd_register(&esp_idf_sys::esp_vfs_eventfd_config_t {
            max_fds: 5,
            ..Default::default()
        })
    })?;

    // // Bind the log crate to the ESP Logging facilities
    // EspLogger::initialize_default();
    components::logger::EspHomeLogger::initialize_default();

    let netif_stack = Arc::new(EspNetifStack::new()?);
    let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    let default_nvs = Arc::new(EspDefaultNvs::new()?);

    let wifi = wifi(
        netif_stack.clone(),
        sys_loop_stack.clone(),
        default_nvs.clone(),
    )?;

    // WiFi should be up by now, get IP addr
    let ip = {
        let status = wifi.get_status();

        match status {
            Status(
                ClientStatus::Started(ClientConnectionStatus::Connected(ClientIpStatus::Done(
                    ip_settings,
                ))),
                ApStatus::Stopped,
            ) => ip_settings.ip,
            _ => unreachable!("WiFi was fine a few seconds ago!"),
        }
    };

    run_esphome(&ip);

    drop(wifi);
    info!("Wifi stopped");

    Ok(())
}

fn run_esphome(ip: &Ipv4Addr) {
    // initialise components
    let mut comp_mngr = Box::new(ComponentManager::new());

    // create high level device
    let device = Arc::new(Device {
        mac: String::from(MAC), // TODO

        model: String::from(MODEL),
        name: String::from(NAME),
        project_name: String::from(NAME) + "." + PROJECT_SUFFIX, // the '.' is required!
        project_version: String::from(VERSION),
        server_name: String::from(NAME) + " on " + MODEL,

        password: String::from(CLIENT_PW),

        component_description: comp_mngr.get_descriptions(),
    });

    // setup mDNS
    #[cfg(feature = "mdns")]
    if let Err(err) = setup_mdns(&device, ip) {
        warn!("failed to setup mDNS: {}", err);
    }

    // setup server
    smol::block_on(async {
        let server = server::EspHomeApiServer::new(device, comp_mngr);
        let _server = Box::new(server).run_asyn().await;
    });
}

fn wifi(
    netif_stack: Arc<EspNetifStack>,
    sys_loop_stack: Arc<EspSysLoopStack>,
    default_nvs: Arc<EspDefaultNvs>,
) -> Result<Box<EspWifi>> {
    let mut wifi = Box::new(EspWifi::new(netif_stack, sys_loop_stack, default_nvs)?);

    info!("Wifi created, about to scan");

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == SSID);

    let channel = if let Some(ours) = ours {
        info!(
            "Found configured access point {} on channel {}",
            SSID, ours.channel
        );
        Some(ours.channel)
    } else {
        info!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            SSID
        );
        None
    };

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.into(),
        password: PASS.into(),
        channel,
        ..Default::default()
    }))?;

    info!("Wifi configuration set, about to get status");

    wifi.wait_status_with_timeout(Duration::from_secs(20), |status| !status.is_transitional())
        .map_err(|e| anyhow::anyhow!("Unexpected Wifi status: {:?}", e))?;

    let status = wifi.get_status();

    if let Status(
        ClientStatus::Started(ClientConnectionStatus::Connected(ClientIpStatus::Done(ip_settings))),
        ApStatus::Stopped,
    ) = status
    {
        info!("Wifi connected, about to do some pings");

        let ping_summary = EspPing::default().ping(
            ip_settings.subnet.gateway,
            &embedded_svc::ping::Configuration {
                count: 3,
                ..Default::default()
            },
        )?;
        if ping_summary.transmitted != ping_summary.received {
            bail!(
                "Pinging gateway {} resulted in timeouts",
                ip_settings.subnet.gateway
            );
        }

        info!("Pinging done");
    } else {
        bail!("Unexpected Wifi status: {:?}", status);
    }

    Ok(wifi)
}

#[cfg(feature = "mdns")]
fn setup_mdns(dev: &Device, _ip: &Ipv4Addr) -> Result<()> {
    use esp_idf_sys::esp;
    use std::{ffi, ptr};

    let hostname = ffi::CString::new(dev.name.to_owned())?;
    let instance_name = ffi::CString::new(dev.name.to_owned())?;
    let service = ffi::CString::new("_esphomelib")?;
    let ty = ffi::CString::new("_tcp")?;

    trace!("about to enter unsafe land");
    unsafe {
        esp!(esp_idf_sys::mdns_init())?;

        esp!(esp_idf_sys::mdns_hostname_set(hostname.as_ptr()))?;
        esp!(esp_idf_sys::mdns_instance_name_set(instance_name.as_ptr()))?;

        esp!(esp_idf_sys::mdns_service_add(
            ptr::null_mut(),
            service.as_ptr(),
            ty.as_ptr(),
            PORT,
            ptr::null_mut(),
            0
        ))?;
    };
    trace!("mDNS should be setup");

    Ok(())
}
