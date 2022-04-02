use std::{
    net::TcpStream,
    sync::{Arc, Mutex},
};

use anyhow::{bail, Result};
use async_channel::{Receiver, Sender};
use futures_lite::{
    future,
    io::{ReadHalf, WriteHalf},
    AsyncRead, AsyncWrite,
};
use log::*;
use protobuf::{Message, ProtobufEnum};
use smol::io::{split, AsyncWriteExt};

use crate::{api::*, components::ComponentUpdate, consts::*, Device};

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
                // only expect HelloRequest (1)
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
pub struct EspHomeApiClient;

impl EspHomeApiClient {
    pub async fn new(
        stream: smol::Async<TcpStream>,
        device: Arc<Device>,
        receiver: Receiver<ComponentUpdate>,
        sender: Sender<ComponentUpdate>,
    ) -> Result<()> {
        // The idea is to have to halfes:
        //  1 recevies messages from the net, but does not send anything
        //  2 receives messages internally and sends to net
        // There is an internal message queue for things like `PingRequest` that do not need to go through the server
        let (stream_read, stream_send) = split(stream);
        let (int_send, int_recv) = async_channel::bounded(10);
        let logs = Arc::new(Mutex::new(LogLevel::LOG_LEVEL_NONE));

        // setup (net) sending half
        let logs_a = logs.clone();
        smol::spawn(async {
            let res = handle_queue(logs_a, receiver, int_recv, stream_send).await;
            if let Err(err) = res {
                warn!("Client queue returned: {err}");
            }
        })
        .detach();

        // setup (net) receiving part
        let logs_b = logs.clone();
        let device_b = device.clone();
        smol::spawn(async {
            let res = handle_net(device_b, logs_b, int_send, sender, stream_read).await;
            if let Err(err) = res {
                warn!("Client net returned: {err}");
            }
        })
        .detach();

        Ok(())
    }
}

async fn handle_queue(
    log: Arc<Mutex<LogLevel>>,
    ext_recv: Receiver<ComponentUpdate>,
    int_recv: Receiver<ComponentUpdate>,
    mut stream_send: WriteHalf<smol::Async<TcpStream>>,
) -> Result<()> {
    loop {
        // prefer internal queue over network
        match future::or(int_recv.recv(), ext_recv.recv()).await {
            Ok(msg) => {
                match msg {
                    ComponentUpdate::Request(..)
                    | ComponentUpdate::Update
                    | ComponentUpdate::LightRequest(..)
                    | ComponentUpdate::Closing
                    | ComponentUpdate::Connection(..) => {
                        warn!("received unexpected message! This is likely a code bug!");
                    }

                    ComponentUpdate::Response((ty, msg)) => {
                        send_packet(&mut stream_send, ty, msg.as_ref().as_ref()).await?;
                    }

                    ComponentUpdate::Log(msg) => {
                        // DO NOT LOG ANYTHING IN HERE
                        // It'll create a recursion

                        // only send when requested
                        if msg.level.value() <= log.lock().expect("lock poisened!").value() {
                            send_packet(
                                &mut stream_send,
                                MessageTypes::SubscribeLogsResponse,
                                msg.as_ref(),
                            )
                            .await?;
                        }
                    }
                }
            }
            Err(err) => {
                bail!("received error {err}, closing");
            }
        }
    }
}

async fn handle_net(
    device: Arc<Device>,
    log: Arc<Mutex<LogLevel>>,
    int_send: Sender<ComponentUpdate>,
    ext_send: Sender<ComponentUpdate>,
    mut stream_read: ReadHalf<smol::Async<TcpStream>>,
) -> Result<()> {
    let mut state = ConnectionState::Initalized;

    loop {
        // read available packets
        let (ty, msg) = read_packet(&mut stream_read).await?;
        if ty == MessageTypes::Unkown {
            info!("Recevied shutdown signal");
            return Ok(());
        }
        trace!("received type {}", ty);

        // handle special cases independend
        match ty {
            MessageTypes::DisconnectRequest => {
                // DisconnectRequest
                info!("DisconnectRequest");
                expect_empty!(msg, "DisconnectRequest");

                let resp = DisconnectResponse::new();
                int_send
                    .send(ComponentUpdate::Response((
                        MessageTypes::DisconnectResponse,
                        Arc::new(Box::new(resp)),
                    )))
                    .await?;
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
                int_send
                    .send(ComponentUpdate::Response((
                        MessageTypes::PingResponse,
                        Arc::new(Box::new(resp)),
                    )))
                    .await?;
                continue;
            }
            MessageTypes::PingResponse => {}
            _ => {}
        }

        // check if type is allowed
        if !state.is_call_legal(ty) {
            warn!("received illegal call! type: {ty}, state {state:?}");
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
                resp.set_server_info(device.project_name.to_owned());
                resp.set_api_version_major(API_MAX);
                resp.set_api_version_minor(API_MIN);
                resp.set_name(device.name.to_owned());

                int_send
                    .send(ComponentUpdate::Response((
                        MessageTypes::HelloResponse,
                        Arc::new(Box::new(resp)),
                    )))
                    .await?;

                state = ConnectionState::Helloed;
            }
            MessageTypes::ConnectRequest => {
                // ConnectRequest
                info!("ConnectRequest");

                let req = ConnectRequest::parse_from_bytes(&msg)?;

                let valid_login =
                    device.password.is_empty() || req.get_password() == device.password;

                if !valid_login {
                    // Shall we print it? Or better not?
                    // (we don't support encryption, so anybody in the network can read it anyway)
                    warn!("invalid login attempt: {}", req.get_password());

                    // don't bail yet!
                }

                let mut resp = ConnectResponse::new();
                resp.set_invalid_password(!valid_login);

                int_send
                    .send(ComponentUpdate::Response((
                        MessageTypes::ConnectResponse,
                        Arc::new(Box::new(resp)),
                    )))
                    .await?;

                if !valid_login {
                    // time to bail
                    bail!("invalid login attempt!");
                }

                info!("connected");
                state = ConnectionState::Connected;
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

                resp.set_mac_address(device.mac.to_owned());
                resp.set_model(device.model.to_owned());
                resp.set_name(device.name.to_owned());
                resp.set_project_name(device.project_name.to_owned());
                resp.set_project_version(device.project_version.to_owned());

                resp.set_uses_password(!device.password.is_empty());

                int_send
                    .send(ComponentUpdate::Response((
                        MessageTypes::DeviceInfoResponse,
                        Arc::new(Box::new(resp)),
                    )))
                    .await?;
            }
            MessageTypes::ListEntitiesRequest => {
                // ListEntitiesRequest
                info!("ListEntitiesRequest");
                expect_empty!(msg, "ListEntitiesRequest");

                // int_send.send(ComponentUpdate::ListEntitiesRequest).await?;
                for comp in &device.component_description {
                    // send_packet(&mut stream_send, comp.0, comp.1.as_ref()).await?;
                    int_send
                        .send(ComponentUpdate::Response((comp.0, comp.1.to_owned())))
                        .await?;
                }

                let resp = ListEntitiesDoneResponse::new();
                int_send
                    .send(ComponentUpdate::Response((
                        MessageTypes::ListEntitiesDoneResponse,
                        Arc::new(Box::new(resp)),
                    )))
                    .await?;
            }
            MessageTypes::SubscribeStatesRequest => {
                // SubscribeStatesRequest
                info!("SubscribeStatesRequest");
                expect_empty!(msg, "SubscribeStatesRequest");

                // request state from all
                ext_send
                    .send(ComponentUpdate::Request(None))
                    .await
                    .expect("failed to send");
            }
            MessageTypes::SubscribeLogsRequest => {
                // SubscribeLogsRequest
                info!("SubscribeLogsRequest");

                let msg = SubscribeLogsRequest::parse_from_bytes(&msg)?;
                // update log state for client
                *log.lock().expect("lock poisened!") = msg.level;
            }
            MessageTypes::LightCommandRequest => {
                // LightCommandRequest
                info!("LightCommandRequest");

                let msg = LightCommandRequest::parse_from_bytes(&msg)?;
                let msg = ComponentUpdate::LightRequest(Box::new(msg));

                ext_send.send(msg).await.expect("failed to send");
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

async fn read_packet<T: AsyncRead + Unpin>(stream: &mut T) -> Result<(MessageTypes, Vec<u8>)> {
    match read_packet_inner(stream).await {
        Ok((0, msg)) => {
            assert!(msg.is_empty());
            return Ok((MessageTypes::Unkown, vec![]));
        }
        Ok((ty, msg)) => Ok((ty.into(), msg)),
        Err(err) => {
            if let Some(err) = err.downcast_ref::<std::io::Error>() {
                match err.kind() {
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::UnexpectedEof => {
                        return Ok((MessageTypes::Unkown, vec![]));
                    }
                    _ => {
                        warn!("read_packet recieved io error: {}", err);
                        bail!("read_packet recieved io error: {}", err);
                    }
                }
            }

            bail!("unhandled error {}", err);
        }
    }
}

async fn read_packet_inner<T: AsyncRead + Unpin>(stream: &mut T) -> Result<(u32, Vec<u8>)> {
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

async fn send_packet<T>(stream: &mut T, ty: MessageTypes, msg: &dyn Message) -> Result<()>
where
    T: AsyncWrite + Unpin,
{
    trace!("sending {}, {:?}", ty as u32, msg);
    let len = msg.compute_size();

    let mut packet = vec![0 as u8];
    packet.append(&mut to_varuint(len));
    packet.append(&mut to_varuint(ty as u32));
    packet.append(&mut msg.write_to_bytes()?);

    stream.write_all(&packet).await?;

    Ok(())
}
