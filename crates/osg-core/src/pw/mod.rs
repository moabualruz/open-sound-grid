// Adapted from Sonusmix (MPL-2.0) — https://codeberg.org/sonusmix/sonusmix

mod identifier;
mod mainloop;
mod object;
mod pod;
mod store;

use std::{collections::HashMap, sync::mpsc, thread};

use thiserror::Error;
use tracing::error;
use ulid::Ulid;

pub use identifier::NodeIdentifier;
pub use object::PortKind;

use mainloop::init_mainloop;
pub use mainloop::map_ports;

/// Errors originating from the PipeWire backend.
#[derive(Error, Debug)]
pub enum PwError {
    #[error("failed to connect to PipeWire: {0}")]
    ConnectionFailed(String),

    #[error("node {0} not found")]
    NodeNotFound(u32),

    #[error("port {0} not found")]
    PortNotFound(u32),

    #[error("device {0} not found")]
    DeviceNotFound(u32),

    #[error("no active route found on device {device_id} with device index {device_index}")]
    RouteNotFound { device_id: u32, device_index: i32 },

    #[error("failed to create sink: {0}")]
    SinkCreationFailed(String),

    #[error("failed to create link: {0}")]
    LinkCreationFailed(String),

    #[error("invalid port: {0}")]
    InvalidPort(String),

    #[error("no port pairs to connect between nodes {start_id} and {end_id}")]
    NoPortPairs { start_id: u32, end_id: u32 },

    #[error("group node with id '{0}' does not exist")]
    GroupNodeNotFound(ulid::Ulid),

    #[error("node {0} is missing device index")]
    MissingDeviceIndex(u32),

    #[error("PipeWire thread exited unexpectedly")]
    ThreadExited,
}

// TODO: Import from crate::state once the state module is created
// use crate::state::GroupNodeKind;

const OSG_APP_NAME: &str = "open-sound-grid";
const OSG_APP_ID: &str = "org.opensoundgrid.OpenSoundGrid";

/// Kind of virtual audio bus to create. Domain: Channel kind.
///
/// TODO: Move to crate::state once the state module is created.
/// This is defined here temporarily so the PW layer compiles standalone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupNodeKind {
    Source,
    #[default]
    Duplex,
    Sink,
}

pub struct PipewireHandle {
    pipewire_thread_handle: Option<thread::JoinHandle<()>>,
    adapter_thread_handle: Option<thread::JoinHandle<Result<(), PipewireChannelError>>>,
    pipewire_sender: mpsc::Sender<ToPipewireMessage>,
}

impl PipewireHandle {
    pub fn init(
        to_pw_channel: (
            mpsc::Sender<ToPipewireMessage>,
            mpsc::Receiver<ToPipewireMessage>,
        ),
        update_fn: impl Fn(Box<AudioGraph>) + Send + 'static,
    ) -> Result<Self, PwError> {
        // TODO: Decide if we actually need a dedicated channel and message type to communicate
        // from Pipewire to the main thread, or if the graph updates are enough
        let (pipewire_thread_handle, pw_sender, _from_pw_receiver) = init_mainloop(update_fn)?;
        let adapter_thread_handle = init_adapter(to_pw_channel.1, pw_sender);
        Ok(Self {
            pipewire_thread_handle: Some(pipewire_thread_handle),
            adapter_thread_handle: Some(adapter_thread_handle),
            pipewire_sender: to_pw_channel.0,
        })
    }
}

impl Drop for PipewireHandle {
    fn drop(&mut self) {
        let _ = self.pipewire_sender.send(ToPipewireMessage::Exit);
        if let Some(adapter_thread_handle) = self.adapter_thread_handle.take()
            && let Err(err) = adapter_thread_handle.join()
        {
            error!("Adapter thread panicked: {err:?}");
        }
        if let Some(pipewire_thread_handle) = self.pipewire_thread_handle.take()
            && let Err(err) = pipewire_thread_handle.join()
        {
            error!("Pipewire thread panicked: {err:?}");
        }
    }
}

pub type GroupNode = object::GroupNode<(), ()>;
pub type Client = object::Client<()>;
pub type Device = object::Device<(), ()>;
pub type Node = object::Node<(), ()>;
pub type Port = object::Port<()>;
pub type Link = object::Link<()>;

/// Read-only projection of PipeWire's current graph state. DDD read model.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioGraph {
    pub group_nodes: HashMap<Ulid, GroupNode>,
    pub clients: HashMap<u32, Client>,
    pub devices: HashMap<u32, Device>,
    pub nodes: HashMap<u32, Node>,
    pub ports: HashMap<u32, Port>,
    pub links: HashMap<u32, Link>,
}

#[derive(Debug, PartialEq)]
pub enum ToPipewireMessage {
    Update,
    NodeVolume(u32, Vec<f32>),
    NodeMute(u32, bool),
    #[rustfmt::skip]
    CreatePortLink { start_id: u32, end_id: u32 },
    #[rustfmt::skip]
    CreateNodeLinks { start_id: u32, end_id: u32 },
    #[rustfmt::skip]
    RemovePortLink { start_id: u32, end_id: u32 },
    #[rustfmt::skip]
    RemoveNodeLinks { start_id: u32, end_id: u32 },
    /// Domain: AddChannel. Creates a virtual audio bus (Channel) in PipeWire.
    CreateGroupNode(String, Ulid, GroupNodeKind),
    /// Domain: RemoveChannel. Removes a virtual audio bus (Channel) from PipeWire.
    RemoveGroupNode(Ulid),
    Exit,
}

#[derive(Debug)]
enum FromPipewireMessage {}

#[derive(Error, Debug)]
#[error("failed to send message to Pipewire: {0:?}")]
struct PipewireChannelError(ToPipewireMessage);

/// This thread takes events from a stdlib mpsc channel and puts them into a pipewire::channel,
/// because pipewire::channel uses a synchronous mutex and thus could cause deadlocks if called
/// from async code. This might not be needed, but it'd probably be pretty annoying to debug if it
/// turned out that the small block to send messages is actually a problem.
fn init_adapter(
    receiver: mpsc::Receiver<ToPipewireMessage>,
    pw_sender: pipewire::channel::Sender<ToPipewireMessage>,
) -> thread::JoinHandle<Result<(), PipewireChannelError>> {
    thread::spawn(move || {
        loop {
            match receiver.recv().unwrap_or(ToPipewireMessage::Exit) {
                ToPipewireMessage::Exit => {
                    break pw_sender
                        .send(ToPipewireMessage::Exit)
                        .map_err(PipewireChannelError);
                }
                message => pw_sender.send(message).map_err(PipewireChannelError)?,
            }
        }
    })
}
