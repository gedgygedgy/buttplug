use crate::{
  core::{
    errors::{ButtplugDeviceError, ButtplugError},
    messages::RawReading,
    ButtplugResultFuture,
  },
  device::{
    configuration_manager::{DeviceSpecifier, ProtocolDefinition, HIDSpecifier},
    ButtplugDeviceEvent, ButtplugDeviceImplCreator, DeviceImpl, DeviceImplInternal, DeviceReadCmd,
    DeviceSubscribeCmd, DeviceUnsubscribeCmd, DeviceWriteCmd, Endpoint,
  },
  server::comm_managers::{ButtplugDeviceSpecificError, util::hidapi_async::HidAsyncDevice},
  util::async_manager,
};
use async_trait::async_trait;
use futures::{AsyncWriteExt, FutureExt, future::BoxFuture};
use hidapi::{DeviceInfo, HidApi};
use std::{
  fmt::{self, Debug},
  io::ErrorKind,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
  thread,
  time::Duration,
};
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;

pub struct HIDDeviceImplCreator {
  hid_instance: Arc<HidApi>,
  specifier: DeviceSpecifier,
  device_info: DeviceInfo,
}

impl HIDDeviceImplCreator {
  pub fn new(hid_instance: Arc<HidApi>, device_info: &DeviceInfo) -> Self {
    Self {
      hid_instance,
      specifier: DeviceSpecifier::HID(HIDSpecifier::new(device_info.vendor_id(), device_info.product_id())),
      device_info: device_info.clone()
    }
  }
}

impl Debug for HIDDeviceImplCreator {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("HIDDeviceImplCreator")
      .field("device_info", &self.device_info)
      .finish()
  }
}

#[async_trait]
impl ButtplugDeviceImplCreator for HIDDeviceImplCreator {
  fn get_specifier(&self) -> DeviceSpecifier {
    self.specifier.clone()
  }

  async fn try_create_device_impl(
    &mut self,
    protocol: ProtocolDefinition,
  ) -> Result<DeviceImpl, ButtplugError> {
    let device = self.device_info.open_device(&self.hid_instance).unwrap();
    let device_impl_internal = HIDDeviceImpl::new(HidAsyncDevice::new(device).unwrap());
    let device_impl = DeviceImpl::new(
      &self.device_info.product_string().unwrap(),
      &self.device_info.serial_number().unwrap(),
      &[Endpoint::Rx, Endpoint::Tx],
      Box::new(device_impl_internal),
    );
    info!("New HID device created: {}", self.device_info.product_string().unwrap());
    Ok(device_impl)
  }
}

pub struct HIDDeviceImpl {
  connected: Arc<AtomicBool>,
  device_event_sender: broadcast::Sender<ButtplugDeviceEvent>,
  device: Arc<Mutex<HidAsyncDevice>>
}

impl HIDDeviceImpl {
  pub fn new(
    device: HidAsyncDevice,
  ) -> Self {
    let (device_event_sender, _) = broadcast::channel(256);
    Self {
      device: Arc::new(Mutex::new(device)),
      connected: Arc::new(AtomicBool::new(true)),
      device_event_sender
    }
  }
}

impl DeviceImplInternal for HIDDeviceImpl {
  fn event_stream(&self) -> broadcast::Receiver<ButtplugDeviceEvent> {
    self.device_event_sender.subscribe()
  }

  fn connected(&self) -> bool {
    self.connected.load(Ordering::SeqCst)
  }

  fn disconnect(&self) -> ButtplugResultFuture {
    let connected = self.connected.clone();
    Box::pin(async move {
      connected.store(false, Ordering::SeqCst);
      Ok(())
    })
  }

  fn read_value(
    &self,
    _msg: DeviceReadCmd,
  ) -> BoxFuture<'static, Result<RawReading, ButtplugError>> {
    unimplemented!();
  }

  fn write_value(&self, msg: DeviceWriteCmd) -> ButtplugResultFuture {
    let device = self.device.clone();
    Box::pin(async move {
      device.lock().await.write(&msg.data).await.map_err(|e| ButtplugError::from(ButtplugDeviceError::DeviceCommunicationError(format!("Cannot write to HID Device: {:?}.", e))))?;
      Ok(())
    })
  }

  fn subscribe(&self, _msg: DeviceSubscribeCmd) -> ButtplugResultFuture {
    unimplemented!();
  }

  fn unsubscribe(&self, _msg: DeviceUnsubscribeCmd) -> ButtplugResultFuture {
    unimplemented!();
  }
}
