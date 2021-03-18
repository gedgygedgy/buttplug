use crate::{
  core::{
    errors::{ButtplugDeviceError, ButtplugError},
    messages::RawReading,
    ButtplugResultFuture,
  },
  device::{
    configuration_manager::{DeviceSpecifier, ProtocolDefinition, HTTPEndpointSpecifier},
    ButtplugDeviceEvent,
    ButtplugDeviceImplCreator,
    DeviceImpl,
    DeviceImplInternal,
    DeviceReadCmd,
    DeviceSubscribeCmd,
    DeviceUnsubscribeCmd,
    DeviceWriteCmd,
    Endpoint,
  },
  server::comm_managers::ButtplugDeviceSpecificError,
};
use async_trait::async_trait;
use futures::future::{self, BoxFuture};
use std::{
  fmt::{self, Debug},
};
use tokio::sync::broadcast;
use surf;

pub struct HTTPEndpointDeviceImplCreator {
  index: u8
}

impl HTTPEndpointDeviceImplCreator {
  pub fn new(index: u8) -> Self {
    debug!("Emitting a new http endpoint device impl creator!");
    Self { index }
  }
}

impl Debug for HTTPEndpointDeviceImplCreator {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("HTTPEndpointDeviceImplCreator")
      .finish()
  }
}

#[async_trait]
impl ButtplugDeviceImplCreator for HTTPEndpointDeviceImplCreator {
  fn get_specifier(&self) -> DeviceSpecifier {
    debug!("Getting the specifier!");
    DeviceSpecifier::HTTP(HTTPEndpointSpecifier::default())
  }

  async fn try_create_device_impl(
    &mut self,
    _protocol: ProtocolDefinition,
  ) -> Result<DeviceImpl, ButtplugError> {
    debug!("Emitting a new xbox device impl.");
    let device_impl_internal = HTTPEndpointDeviceImpl::new(self.index);
    let device_impl = DeviceImpl::new(
      &format!("Ear{}", self.index),
      &format!("HTTP Device {}", self.index),
      &[Endpoint::Tx],
      Box::new(device_impl_internal),
    );
    Ok(device_impl)
  }
}

#[derive(Clone, Debug)]
pub struct HTTPEndpointDeviceImpl {
  event_sender: broadcast::Sender<ButtplugDeviceEvent>,
  index: u8
}

impl HTTPEndpointDeviceImpl {
  pub fn new(index: u8) -> Self {
    let (device_event_sender, _) = broadcast::channel(256);
    Self {
      event_sender: device_event_sender,
      index
    }
  }
}

impl DeviceImplInternal for HTTPEndpointDeviceImpl {
  fn event_stream(&self) -> broadcast::Receiver<ButtplugDeviceEvent> {
    self.event_sender.subscribe()
  }

  fn connected(&self) -> bool {
    true
  }

  fn disconnect(&self) -> ButtplugResultFuture {
    Box::pin(future::ready(Ok(())))
  }

  fn read_value(
    &self,
    _msg: DeviceReadCmd,
  ) -> BoxFuture<'static, Result<RawReading, ButtplugError>> {
    panic!("We should never get here!");
  }

  fn write_value(&self, msg: DeviceWriteCmd) -> ButtplugResultFuture {
    let index = self.index;
    Box::pin(async move {
      if surf::get(format!("http://192.168.123.191:5000/?speed={}&index={}", msg.data[0], index)).await.is_err() {
        error!("Got http error.");
      }
      Ok(())
    })
  }

  fn subscribe(&self, _msg: DeviceSubscribeCmd) -> ButtplugResultFuture {
    panic!("We should never get here!");
  }

  fn unsubscribe(&self, _msg: DeviceUnsubscribeCmd) -> ButtplugResultFuture {
    panic!("We should never get here!");
  }
}
