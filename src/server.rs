use async_channel::{Receiver, Sender};
use async_io::Timer;
use futures_lite::{self, StreamExt};
use log::*;
use smol::Async;
use std::{
    net::TcpListener,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    client::EspHomeApiClient,
    components::{ComponentManager, ComponentUpdate},
    Device, PORT,
};

pub struct Listener;

impl Listener {
    pub async fn run(send: Sender<ComponentUpdate>) -> smol::io::Result<()> {
        let listener = Async::<TcpListener>::bind(([0, 0, 0, 0], PORT));
        if listener.is_err() {
            error!("failed to bind to socket!");
            unreachable!();
        } else {
            info!("listener is ok");
        }

        let listener = listener.expect("failed to set up listener");

        while let Ok((socket, _addr)) = listener.accept().await {
            send.send(ComponentUpdate::Connection(Arc::new(socket)))
                .await
                .expect("failed to sent to server");
        }
        Ok(())
    }
}

const UPDATE_TICK: Duration = Duration::from_secs(10); // TODO tune this

pub struct TickTimer;

impl TickTimer {
    pub async fn run(send: Sender<ComponentUpdate>) {
        let mut timer = Timer::interval(UPDATE_TICK);

        while let Some(_) = timer.next().await {
            send.send(ComponentUpdate::Update)
                .await
                .expect("failed to sent to server");
        }
    }
}

pub struct EspHomeApiServer {
    device: Arc<Device>,
    components: Mutex<Box<ComponentManager>>,

    client_recv: Receiver<ComponentUpdate>,
    client_send: Sender<ComponentUpdate>,
    clients: Vec<Sender<ComponentUpdate>>,
}

impl EspHomeApiServer {
    pub async fn new(device: Arc<Device>, components: Box<ComponentManager>) -> Self {
        info!("setting up ...");

        // server communication channels
        let (client_send, client_recv) = async_channel::bounded(10);

        smol::spawn(Listener::run(client_send.clone())).detach();
        info!("listener running");

        // setup timer
        smol::spawn(TickTimer::run(client_send.clone())).detach();
        info!("timer running");

        EspHomeApiServer {
            device,
            components: Mutex::new(components),
            // listener,
            client_recv,
            client_send,
            clients: vec![],
        }
    }

    pub async fn run_asyn(&mut self) {
        loop {
            match self.client_recv.recv().await {
                Ok(upd) => match upd {
                    ComponentUpdate::Closing => (),
                    ComponentUpdate::Connection(socket) => {
                        let (server_send, client_recv) = async_channel::unbounded();
                        let client_send = self.client_send.clone();
                        let device = self.device.to_owned();
                        let socket = Arc::<Async<std::net::TcpStream>>::try_unwrap(socket)
                            .expect("failed to get socket");
                        smol::spawn(async {
                            Box::new(EspHomeApiClient::new(
                                socket,
                                device,
                                client_recv,
                                client_send,
                            ))
                            .run()
                            .await;
                        })
                        .detach();

                        self.clients.push(server_send);
                    }
                    upd @ _ => {
                        let resp = self.components.lock().expect("mutex poisoned").hanlde(&upd);
                        // for now, send to all
                        for resp in resp.into_iter() {
                            let mut index = 0;
                            while index < self.clients.len() {
                                // how to do this better?
                                let remove = if let Err(err) =
                                    self.clients[index].send(resp.to_owned()).await
                                {
                                    warn!("server 2 client: {}", err);
                                    true
                                } else {
                                    false
                                };

                                if remove {
                                    self.clients.remove(index);
                                } else {
                                    index += 1;
                                }
                            }
                        }
                    }
                },
                Err(err) => warn!("{}", &err),
            }
        }
    }
}
