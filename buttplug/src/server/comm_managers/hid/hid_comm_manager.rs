use super::hid_device_impl::HIDDeviceImplCreator;
use crate::{
  core::ButtplugResultFuture,
  server::comm_managers::{
    DeviceCommunicationEvent, DeviceCommunicationManager, DeviceCommunicationManagerBuilder,
  },
};
use futures::future;
use tokio::sync::mpsc::Sender;
use tracing_futures::Instrument;
use hidapi::HidApi;
use std::sync::Arc;

#[derive(Default)]
pub struct HIDCommunicationManagerBuilder {
  sender: Option<tokio::sync::mpsc::Sender<DeviceCommunicationEvent>>
}

impl DeviceCommunicationManagerBuilder for HIDCommunicationManagerBuilder {
  fn set_event_sender(&mut self, sender: Sender<DeviceCommunicationEvent>) {
    self.sender = Some(sender)
  }

  fn finish(mut self) -> Box<dyn DeviceCommunicationManager> {
    Box::new(HIDCommunicationManager::new(self.sender.take().unwrap()))
  }
}

pub struct HIDCommunicationManager {
  sender: Sender<DeviceCommunicationEvent>,
  hidapi: Arc<HidApi>
}

impl HIDCommunicationManager {
  fn new(sender: Sender<DeviceCommunicationEvent>) -> Self {
    Self { 
      sender,
      hidapi: Arc::new(HidApi::new().unwrap())
    }
  }
}

impl DeviceCommunicationManager for HIDCommunicationManager {
  fn name(&self) -> &'static str {
    "HIDCommunicationManager"
  }

  fn start_scanning(&self) -> ButtplugResultFuture {
    // TODO Does this block? Should it run in one of our threads?
    let device_sender = self.sender.clone();
    let api = self.hidapi.clone();
    Box::pin(
      async move {
        let mut seen_addresses = vec!();
        for device in api.device_list() {
          if let None = device.serial_number() {
            continue;
          }
          let serial_number = device.serial_number().unwrap().to_owned();
          if seen_addresses.contains(&serial_number) {
            continue;
          }
          seen_addresses.push(serial_number.clone());
          let device_creator = HIDDeviceImplCreator::new(api.clone(), &device);
          if device_sender
            .send(DeviceCommunicationEvent::DeviceFound {
              name: device.product_string().unwrap().to_owned(),
              address: serial_number,
              creator: Box::new(device_creator),
            })
            .await
            .is_err()
          {
            error!("Device manager receiver dropped, cannot send device found message.");
            return Ok(());
          }
        }
        Ok(())
      }
      .instrument(tracing::info_span!(
        "HID Device Comm Manager Scanning."
      )),
    )
  }

  fn stop_scanning(&self) -> ButtplugResultFuture {
    Box::pin(future::ready(Ok(())))
  }
}
