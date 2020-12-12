use gdnative::prelude::*;
use message_io::events::EventQueue;
use message_io::events::EventSender;
use message_io::network::Endpoint;
use message_io::network::{NetEvent, Network};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::*;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

godot_init!(init);
fn init(handle: InitHandle) {
    handle.add_class::<L>();
}

#[derive(NativeClass)]
#[inherit(Control)]
pub struct L {
    rx: Receiver<InternalMessage>,
    sender: EventSender<Event>,
    map: std::collections::HashMap<Endpoint, String>,
}

impl L {
    fn new(_owner: &Control) -> Self {
        let (rx, sender) = run().unwrap();
        L {
            rx,
            sender,
            map: Default::default(),
        }
    }
}

#[methods]
impl L {
    #[export]
    fn _ready(&self, _owner: &Control) {
        godot_print!("Hello, world!");
    }

    #[export]
    fn _process(&mut self, _owner: &Control, _delta: f64) {
        if let Ok(i) = self.rx.try_recv() {
            match i {
                InternalMessage::User(endpoint, user) => {
                    self.map.insert(endpoint, user);
                }
                InternalMessage::Content(endpoint, message) => {
                    let m = format!("{}: {}\n", self.map[&endpoint], message);

                    unsafe {
                        _owner
                            .get_node("RichTextLabel")
                            .unwrap()
                            .assume_safe()
                            .cast::<gdnative::api::RichTextLabel>()
                            .unwrap()
                            .add_text(m);
                    }
                }
            }
        }
    }

    #[export]
    fn _input(&mut self, _owner: &Control, event: Ref<InputEvent>) {
        let ev = unsafe {
            match event.assume_safe().cast::<InputEventKey>() {
                Some(ev) => ev,
                None => return,
            }
        };
        if !ev.is_pressed() {
            return;
        }
        let k = ev.get_scancode_with_modifiers();
        godot_print!("{}", k);
        if k == 16777221 {
            let text = unsafe {
                _owner
                    .get_node("TextEdit")
                    .unwrap()
                    .assume_safe()
                    .cast::<gdnative::api::TextEdit>()
                    .unwrap()
                    .text()
            }
            .to_string();
            let text = text.trim();
            unsafe {
                _owner
                    .get_node("TextEdit")
                    .unwrap()
                    .assume_safe()
                    .cast::<gdnative::api::TextEdit>()
                    .unwrap()
                    .set_text("")
            };

            let m = format!("me: {}\n", &text);
            unsafe {
                _owner
                    .get_node("RichTextLabel")
                    .unwrap()
                    .assume_safe()
                    .cast::<gdnative::api::RichTextLabel>()
                    .unwrap()
                    .add_text(m);
            }
            self.sender
                .send(Event::SendUiMsgToNetwork(text.to_string()));
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum NetMessage {
    HelloLan(String, u16), // user_name, server_port
    HelloUser(String),     // user_name
    UserMessage(String),   // content
}

pub enum InternalMessage {
    User(Endpoint, String),
    Content(Endpoint, String),
}

pub enum Event {
    SendUiMsgToNetwork(String),
    Network(NetEvent<NetMessage>),
    Close(Option<()>),
}

pub fn run() -> Result<(Receiver<InternalMessage>, EventSender<Event>)> {
    let mut event_queue = EventQueue::new();

    let sender = event_queue.sender().clone();
    let mut network = Network::new(move |net_event| sender.send(Event::Network(net_event)));

    let server_addr: std::net::SocketAddrV4 = "0.0.0.0:0".parse().unwrap();
    let (_, server_addr) = network.listen_tcp(server_addr)?;

    let discovery_addr: std::net::SocketAddrV4 = "238.255.0.1:5877".parse().unwrap();
    network.listen_udp_multicast(discovery_addr)?;
    let discovery_endpoint = network.connect_udp(discovery_addr)?;

    let message = NetMessage::HelloLan("Test".to_string(), server_addr.port());
    network.send(discovery_endpoint, message);

    let (tx, rx) = channel();
    let mut eps = std::collections::HashSet::new();
    let sender = event_queue.sender().clone();

    std::thread::spawn(move || loop {
        match event_queue.receive() {
            Event::SendUiMsgToNetwork(msg) => {
                godot_print!("{}", &msg);
                godot_print!("{:?}", &eps);
                network.send_all(&eps, NetMessage::UserMessage(msg));
            }
            Event::Network(net_event) => match net_event {
                NetEvent::Message(endpoint, message) => {
                    match message {
                        NetMessage::HelloLan(user, server_port) => {
                            if user == "Test" {
                                continue;
                            }
                            let server_addr = (endpoint.addr().ip(), server_port);
                            let try_connect = || -> Result<()> {
                                let user_endpoint = network.connect_tcp(server_addr)?;
                                let message = NetMessage::HelloUser("Test".to_string());
                                network.send(user_endpoint, message);
                                eps.insert(user_endpoint);
                                tx.send(InternalMessage::User(user_endpoint, user)).unwrap();
                                Ok(())
                            };
                            try_connect().unwrap();
                        }
                        // by tcp:
                        NetMessage::HelloUser(_user) => {
                            eps.insert(endpoint);
                        }
                        NetMessage::UserMessage(content) => {
                            tx.send(InternalMessage::Content(endpoint, content))
                                .unwrap();
                        }
                    }
                }
                NetEvent::AddedEndpoint(_) => (),
                NetEvent::RemovedEndpoint(ep) => {
                    eps.remove(&ep);
                }
                NetEvent::DeserializationError(_) => (),
            },
            Event::Close(_) => {}
        }
    });
    Ok((rx, sender))
}
