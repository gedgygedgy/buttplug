use crate::{
    core::{
        errors::{ButtplugDeviceError, ButtplugError},
        messages::RawReading,
    },
    device::{
        configuration_manager::{BluetoothLESpecifier, DeviceSpecifier, ProtocolDefinition},
        device::{
            ButtplugDeviceCommand, ButtplugDeviceEvent, ButtplugDeviceImplCreator,
            ButtplugDeviceReturn, DeviceImpl, DeviceReadCmd, DeviceSubscribeCmd,
            DeviceUnsubscribeCmd, DeviceWriteCmd,
        },
        Endpoint,
    },
};
use async_std::{
    sync::{channel, Receiver, Sender},
    task,
};
use rumble::api::Peripheral;
use async_trait::async_trait;
use super::rumble_internal::{RumbleInternalEventLoop, DeviceReturnFuture, DeviceReturnStateShared};

pub struct RumbleBLEDeviceImplCreator<T: Peripheral + 'static> {
    device: Option<T>,
}

impl<T: Peripheral> RumbleBLEDeviceImplCreator<T> {
    pub fn new(device: T) -> Self {
        Self {
            device: Some(device),
        }
    }
}

#[async_trait]
impl<T: Peripheral> ButtplugDeviceImplCreator for RumbleBLEDeviceImplCreator<T> {
    fn get_specifier(&self) -> DeviceSpecifier {
        if self.device.is_none() {
            panic!("Cannot call get_specifier after device is taken!");
        }
        let name = self
            .device
            .as_ref()
            .unwrap()
            .properties()
            .local_name
            .unwrap();
        DeviceSpecifier::BluetoothLE(BluetoothLESpecifier::new_from_device(&name))
    }

    async fn try_create_device_impl(
        &mut self,
        protocol: ProtocolDefinition,
    ) -> Result<Box<dyn DeviceImpl>, ButtplugError> {
        // TODO ugggggggh there's gotta be a way to ensure this at compile time.
        if self.device.is_none() {
            panic!("Cannot call try_create_device_impl twice!");
        }
        let device = self.device.take().unwrap();
        if let Some(ref proto) = protocol.btle {
            let (device_sender, device_receiver) = channel(256);
            let (output_sender, output_receiver) = channel(256);
            let p = proto.clone();
            let name = device.properties().local_name.unwrap();
            let address = device.properties().address.to_string();
            // TODO This is not actually async. We're currently using blocking
            // rumble calls, so this will block whatever thread it's spawned to. We
            // should probably switch to using async rumble calls w/ callbacks.
            //
            // The new watchdog async-std executor will at least leave this task on
            // its own thread in time, but I'm not sure when that's landing.
            task::spawn(async move {
                let mut event_loop = RumbleInternalEventLoop::new(device, p, device_receiver, output_sender);
                event_loop.run().await;
            });
            let fut = DeviceReturnFuture::default();
            let waker = fut.get_state_clone();
            device_sender
                .send((ButtplugDeviceCommand::Connect, waker))
                .await;
            match fut.await {
                ButtplugDeviceReturn::Connected(info) => Ok(Box::new(RumbleBLEDeviceImpl {
                    name,
                    address,
                    endpoints: info.endpoints,
                    thread_sender: device_sender,
                    event_receiver: output_receiver,
                })),
                _ => Err(ButtplugError::ButtplugDeviceError(
                    ButtplugDeviceError::new("Cannot connect"),
                )),
            }
        } else {
            panic!("Got a protocol with no Bluetooth Definition!");
        }
    }
}

#[derive(Clone)]
pub struct RumbleBLEDeviceImpl {
    name: String,
    address: String,
    endpoints: Vec<Endpoint>,
    thread_sender: Sender<(ButtplugDeviceCommand, DeviceReturnStateShared)>,
    event_receiver: Receiver<ButtplugDeviceEvent>,
}

unsafe impl Send for RumbleBLEDeviceImpl {}
unsafe impl Sync for RumbleBLEDeviceImpl {}

impl RumbleBLEDeviceImpl {
    pub fn new(
        name: &String,
        address: &String,
        endpoints: Vec<Endpoint>,
        thread_sender: Sender<(ButtplugDeviceCommand, DeviceReturnStateShared)>,
        event_receiver: Receiver<ButtplugDeviceEvent>,
    ) -> Self {
        Self {
            name: name.to_string(),
            address: address.to_string(),
            endpoints,
            thread_sender,
            event_receiver,
        }
    }

    async fn send_to_device_task(
        &self,
        cmd: ButtplugDeviceCommand,
        err_msg: &str,
    ) -> Result<(), ButtplugError> {
        let fut = DeviceReturnFuture::default();
        let waker = fut.get_state_clone();
        self.thread_sender.send((cmd, waker)).await;
        match fut.await {
            ButtplugDeviceReturn::Ok(_) => Ok(()),
            _ => Err(ButtplugError::ButtplugDeviceError(
                ButtplugDeviceError::new(err_msg),
            )),
        }
    }
}

#[async_trait]
impl DeviceImpl for RumbleBLEDeviceImpl {
    fn get_event_receiver(&self) -> Receiver<ButtplugDeviceEvent> {
        self.event_receiver.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn address(&self) -> &str {
        &self.address
    }

    fn connected(&self) -> bool {
        // TODO Should figure out how we wanna deal with this across the
        // representation and inner loop.
        true
    }

    fn endpoints(&self) -> Vec<Endpoint> {
        self.endpoints.clone()
    }

    async fn disconnect(&self) {
        self.send_to_device_task(
            ButtplugDeviceCommand::Disconnect,
            "Cannot disconnect device"
        ).await;
    }

    fn box_clone(&self) -> Box<dyn DeviceImpl> {
        Box::new((*self).clone())
    }

    async fn write_value(&self, msg: DeviceWriteCmd) -> Result<(), ButtplugError> {
        self.send_to_device_task(
            ButtplugDeviceCommand::Message(msg.into()),
            "Cannot write to endpoint",
        )
            .await
    }

    async fn read_value(&self, msg: DeviceReadCmd) -> Result<RawReading, ButtplugError> {
        // TODO Actually implement value reading
        Ok(RawReading::new(0, msg.endpoint, vec![]))
    }

    async fn subscribe(&self, msg: DeviceSubscribeCmd) -> Result<(), ButtplugError> {
        self.send_to_device_task(
            ButtplugDeviceCommand::Message(msg.into()),
            "Cannot subscribe",
        )
            .await
    }

    async fn unsubscribe(&self, msg: DeviceUnsubscribeCmd) -> Result<(), ButtplugError> {
        self.send_to_device_task(
            ButtplugDeviceCommand::Message(msg.into()),
            "Cannot unsubscribe",
        )
            .await
    }
}