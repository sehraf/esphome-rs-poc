use std::{sync::Arc, time::Duration};

use anyhow::{bail, Result};
use log::*;

use smol::io::AsyncWriteExt;

use crate::{api::*, components::ComponentUpdate, consts::*, Device};
use protobuf::{Message, ProtobufEnum};

// from ESPHome
const API_MAX: u32 = 1;
const API_MIN: u32 = 6;

macro_rules! expect_empty {
    ($msg:ident, $opt:literal) => {
        if !$msg.is_empty() {
            warn!("{}: expected empty message!", $opt);
        }
    };
}

#[derive(Debug)]
enum ConnectionState {
    Initalized,
    Helloed, // that name is weird
    Connected,
}

impl ConnectionState {
    pub fn is_call_legal(&self, ty: MessageTypes) -> bool {
        use ConnectionState::*;

        match self {
            Initalized => {
                // only exptec HelloRequest (1)
                return ty == MessageTypes::HelloRequest;
            }
            Helloed => {
                // only expect ConnectRequest (3) and DeviceInfoRequest (9)
                return ty == MessageTypes::ConnectRequest || ty == MessageTypes::DeviceInfoRequest;
            }
            Connected => {
                // expect everything else
                return ty != MessageTypes::HelloRequest && ty != MessageTypes::ConnectRequest;
            }
        }
    }
}

/// This client implements the communication with the ESPHome API client.
///
/// When an api command is received it gets sent to the [[ComponentHandler]].
/// When a responde from the [[ComponentHandler]] is gets sent to the api client.
pub struct EspHomeApiClient {
    state: ConnectionState,
    stream: smol::Async<std::net::TcpStream>,

    device: Arc<Device>,

    recv: async_channel::Receiver<ComponentUpdate>,
    send: async_channel::Sender<ComponentUpdate>,

    // Component specific options
    log: LogLevel,
}

impl EspHomeApiClient {
    pub fn new(
        stream: smol::Async<std::net::TcpStream>,
        device: Arc<Device>,
        receiver: async_channel::Receiver<ComponentUpdate>,
        sender: async_channel::Sender<ComponentUpdate>,
    ) -> EspHomeApiClient {
        EspHomeApiClient {
            state: ConnectionState::Initalized,
            stream,
            device,
            recv: receiver,
            send: sender,

            log: LogLevel::LOG_LEVEL_NONE,
        }
    }

    pub async fn handle(&mut self) -> Result<()> {
        // TODO
        // We should wait for both async, internal messages and external data.
        // There is `select!` but futures-lite does not support this.

        // check for pending packages to send
        while let Ok(msg) = self.recv.try_recv() {
            // DO NOT LOG ANYTHING IN HERE
            // It'll create a recursion

            // we expect responses here
            match msg {
                ComponentUpdate::Request(..)
                | ComponentUpdate::Update
                | ComponentUpdate::LightRequest(..)
                | ComponentUpdate::Closing
                | ComponentUpdate::Connection(..) => {
                    warn!("received unexpected message! This is likely a code bug!");
                }

                ComponentUpdate::LightResponse(msg) => {
                    send_packet(
                        &mut self.stream,
                        MessageTypes::LightStateResponse,
                        msg.as_ref(),
                    )
                    .await?;
                }
                ComponentUpdate::SensorResponse(msg) => {
                    send_packet(
                        &mut self.stream,
                        MessageTypes::SensorStateResponse,
                        msg.as_ref(),
                    )
                    .await?;
                }
                ComponentUpdate::Log(msg) => {
                    // DO NOT LOG ANYTHING IN HERE
                    // It'll create a recursion

                    // only send when requested
                    if msg.level.value() <= self.log.value() {
                        send_packet(
                            &mut self.stream,
                            MessageTypes::SubscribeLogsResponse,
                            msg.as_ref(),
                        )
                        .await?;
                    }
                }
            }
        }

        // read available packets
        let ret = read_packet(&mut self.stream).await;
        let (ty, msg) = match ret {
            Ok((0, v)) if v.is_empty() => return Ok(()),
            Ok((ty, msg)) => (ty.into(), msg),
            Err(err) => {
                if let Some(err) = err.downcast_ref::<std::io::Error>() {
                    // warn!("1 {}", err);
                    match err.kind() {
                        std::io::ErrorKind::ConnectionReset => return Ok(()),
                        std::io::ErrorKind::UnexpectedEof => return Ok(()),
                        _ => {
                            warn!("read_packet recieved io error: {}", err);
                            bail!("read_packet recieved io error: {}", err);
                        }
                    }
                }

                warn!("2 {}", err);
                bail!("unhandled error {}", err);
                // return Ok(());
            }
        };
        info!("received type {}", ty);

        // handle special cases independend
        match ty {
            MessageTypes::DisconnectRequest => {
                // DisconnectRequest
                info!("DisconnectRequest");
                expect_empty!(msg, "DisconnectRequest");

                let resp = DisconnectResponse::new();
                send_packet(&mut self.stream, MessageTypes::DisconnectResponse, &resp).await?;
                return Ok(());
            }
            MessageTypes::DisconnectResponse => {
                // DisconnectResponse
                info!("DisconnectResponse");
                expect_empty!(msg, "DisconnectResponse");

                bail!("disconnected");
            }
            MessageTypes::PingRequest => {
                // PingRequest
                info!("PingRequest");
                expect_empty!(msg, "PingRequest");

                let resp = PingResponse::new();
                send_packet(&mut self.stream, MessageTypes::PingResponse, &resp).await?;
                return Ok(());
            }
            MessageTypes::PingResponse => {}
            _ => {}
        }

        // check if type is allowed
        if !self.state.is_call_legal(ty) {
            warn!(
                "received illegal call! type: {}, state {:?}",
                ty, self.state
            );
            return Ok(());
        }

        match ty {
            MessageTypes::HelloRequest => {
                // HelloRequest
                let req = HelloRequest::parse_from_bytes(&msg)?;
                info!("HelloRequest");
                info!(
                    " -> incoming connection from client {}",
                    req.get_client_info()
                );

                let mut resp = HelloResponse::new();
                resp.set_server_info(self.device.project_name.to_owned());
                resp.set_api_version_major(API_MAX);
                resp.set_api_version_minor(API_MIN);
                resp.set_name(self.device.name.to_owned());

                send_packet(&mut self.stream, MessageTypes::HelloResponse, &resp).await?;

                self.state = ConnectionState::Helloed;
            }
            MessageTypes::ConnectRequest => {
                // ConnectRequest
                info!("ConnectRequest");

                let req = ConnectRequest::parse_from_bytes(&msg)?;

                let valid_login =
                    self.device.password.is_empty() || req.get_password() == self.device.password;
                if !valid_login {
                    warn!("invalid login attempt!");
                }

                let mut resp = ConnectResponse::new();
                resp.set_invalid_password(!valid_login);

                send_packet(&mut self.stream, MessageTypes::ConnectResponse, &resp).await?;

                info!("connected");
                self.state = ConnectionState::Connected;
            }
            MessageTypes::DisconnectRequest
            | MessageTypes::DisconnectResponse
            | MessageTypes::PingRequest
            | MessageTypes::PingResponse => {
                warn!("ty {}: this should already been handled!", ty);
            }
            MessageTypes::DeviceInfoRequest => {
                // DeviceInfoRequest
                info!("DeviceInfoRequest");
                expect_empty!(msg, "DeviceInfoRequest");

                let mut resp = DeviceInfoResponse::new();
                resp.set_esphome_version(String::from("rs v0"));
                resp.set_has_deep_sleep(false);

                resp.set_mac_address(self.device.mac.to_owned());
                resp.set_model(self.device.model.to_owned());
                resp.set_name(self.device.name.to_owned());
                resp.set_project_name(self.device.project_name.to_owned());
                resp.set_project_version(self.device.project_version.to_owned());

                resp.set_uses_password(!self.device.password.is_empty());

                send_packet(&mut self.stream, MessageTypes::DeviceInfoResponse, &resp).await?;
            }
            MessageTypes::ListEntitiesRequest => {
                // ListEntitiesRequest
                info!("ListEntitiesRequest");
                expect_empty!(msg, "ListEntitiesRequest");

                for comp in &self.device.component_description {
                    send_packet(&mut self.stream, comp.0, comp.1.as_ref()).await?;
                }

                // The End
                let resp = ListEntitiesDoneResponse::new();

                send_packet(
                    &mut self.stream,
                    MessageTypes::ListEntitiesDoneResponse,
                    &resp,
                )
                .await?;
            }
            MessageTypes::SubscribeStatesRequest => {
                // SubscribeStatesRequest
                info!("SubscribeStatesRequest");
                expect_empty!(msg, "SubscribeStatesRequest");

                // request state from all
                #[cfg(not(feature = "async"))]
                self.send
                    .send(ComponentUpdate::Request(None))
                    .await
                    .expect("failed to send");
            }
            MessageTypes::SubscribeLogsRequest => {
                // SubscribeLogsRequest
                info!("SubscribeLogsRequest");

                let msg = SubscribeLogsRequest::parse_from_bytes(&msg)?;
                // update log state for client
                self.log = msg.level;
            }
            MessageTypes::LightCommandRequest => {
                // LightCommandRequest
                info!("LightCommandRequest");

                let msg = LightCommandRequest::parse_from_bytes(&msg)?;
                let msg = ComponentUpdate::LightRequest(Box::new(msg));

                self.send.send(msg).await.expect("failed to send");
            }
            MessageTypes::SubscribeHomeassistantServicesRequest => {
                // SubscribeHomeassistantServicesRequest
                info!("SubscribeHomeassistantServicesRequest");

                // !?
            }
            MessageTypes::SubscribeHomeAssistantStatesRequest => {
                // SubscribeHomeAssistantStatesRequest
                info!("SubscribeHomeAssistantStatesRequest");

                // none for now
            }
            _ => {
                warn!("type {} is not implemted yet!", ty);
                // break;
            }
        }
        Ok(())
    }

    pub async fn run(&mut self) {
        while self.handle().await.is_ok() {
            // TODO
            // This is a hack to give the rest of the system some time to process any generated messages.
            // We wait for the answers to arive, before entering `handle()` again.
            // Also see comment in `handle()`.
            std::thread::sleep(Duration::from_millis(100));
        }
        self.send
            .send(ComponentUpdate::Closing)
            .await
            .expect("failed to send");
    }
}

pub fn to_varuint(mut i: u32) -> Vec<u8> {
    if i <= 0x7f {
        return vec![i as u8];
    }

    let mut buffer = vec![];

    while i > 0 {
        let tmp = (i & 0x7f) as u8;
        i >>= 7;
        if i > 0 {
            buffer.push(tmp | 0x80);
        } else {
            buffer.push(tmp)
        }
    }

    buffer
}

pub fn from_varuint(buf: &Vec<u8>) -> u32 {
    let mut i = 0 as u32;
    let mut bitpos = 0;

    for b in buf {
        i |= ((b & 0x7f) as u32) << bitpos;
        bitpos += 7;

        if (b & 0x80) == 0 {
            return i;
        }
    }
    0
}

async fn read_packet(stream: &mut smol::Async<std::net::TcpStream>) -> Result<(u32, Vec<u8>)> {
    use smol::io::AsyncReadExt;

    let mut buf_single: [u8; 1] = [!0];
    let mut buf: Vec<u8> = vec![];

    // recieve empty byte preamble
    trace!("waiting for preamble");
    let len = stream.read(&mut buf_single).await?;
    if len == 0 {
        bail!("nothing to read. Stream is closed");
    }

    if buf_single[0] != 0 {
        bail!("invalid preamble");
    }

    // receive varuint len
    buf.clear();
    loop {
        stream.read_exact(&mut buf_single).await?;
        buf.push(buf_single[0]);

        if (buf_single[0] & 0x80) == 0 {
            break;
        }
    }
    let len = from_varuint(&buf);
    trace!("len {} (0x{:x})", len, len);

    // receive varuint type
    buf.clear();
    loop {
        stream.read_exact(&mut buf_single).await?;
        buf.push(buf_single[0]);

        if (buf_single[0] & 0x80) == 0 {
            break;
        }
    }
    let ty = from_varuint(&buf);
    trace!("type {} (0x{:x})", ty, ty);

    if len == 0 {
        return Ok((ty, vec![]));
    }

    let mut msg: Vec<u8> = vec![0; len as usize];
    stream.read_exact(&mut msg).await?;

    Ok((ty, msg))
}

async fn send_packet(
    stream: &mut smol::Async<std::net::TcpStream>,
    ty: MessageTypes,
    msg: &dyn Message,
) -> Result<()> {
    trace!("sending {}, {:?}", ty as u32, msg);
    let len = msg.compute_size();

    let mut packet = vec![0 as u8];
    packet.append(&mut to_varuint(len));
    packet.append(&mut to_varuint(ty as u32));
    packet.append(&mut msg.write_to_bytes()?);

    stream.write_all(&packet).await?;

    Ok(())
}
