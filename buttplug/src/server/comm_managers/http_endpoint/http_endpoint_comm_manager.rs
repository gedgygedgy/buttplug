use super::http_endpoint_device_impl::HTTPEndpointDeviceImplCreator;
use crate::{
  core::ButtplugResultFuture,
  server::comm_managers::{
    DeviceCommunicationEvent,
    DeviceCommunicationManager,
    DeviceCommunicationManagerCreator,
  },
  util::async_manager,
};
use futures::{future};
use std::{
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
};
use tokio::sync::{mpsc, Notify};

pub struct HTTPEndpointCommManager {
  sender: mpsc::Sender<DeviceCommunicationEvent>,
  scanning_notifier: Arc<Notify>,
  has_emitted_device: Arc<AtomicBool>
}

impl DeviceCommunicationManagerCreator for HTTPEndpointCommManager {
  fn new(sender: mpsc::Sender<DeviceCommunicationEvent>) -> Self {
    Self {
      sender,
      scanning_notifier: Arc::new(Notify::new()),
      has_emitted_device: Arc::new(AtomicBool::new(false))
    }
  }
}

impl DeviceCommunicationManager for HTTPEndpointCommManager {
  fn name(&self) -> &'static str {
    "HTTPEndpointCommunicationManager"
  }

  fn start_scanning(&self) -> ButtplugResultFuture {
    if !self.has_emitted_device.load(Ordering::SeqCst) {
      let sender = self.sender.clone();
      self.has_emitted_device.store(true, Ordering::SeqCst);
      async_manager::spawn(async move {
        let device_creator = Box::new(HTTPEndpointDeviceImplCreator::new(1));
        if sender
          .send(DeviceCommunicationEvent::DeviceFound(device_creator))
          .await
          .is_err()
        {
          error!("Error sending device found message from HTTP Endpoint Manager.");
        }
        let device_creator_2 = Box::new(HTTPEndpointDeviceImplCreator::new(2));
        if sender
          .send(DeviceCommunicationEvent::DeviceFound(device_creator_2))
          .await
          .is_err()
        {
          error!("Error sending device found message from HTTP Endpoint Manager.");
        }
      }).unwrap();
    } 
    self.scanning_notifier.notify_waiters();
    Box::pin(future::ready(Ok(())))
  }

  fn stop_scanning(&self) -> ButtplugResultFuture {
    self.scanning_notifier.notify_waiters();
    let sender = self.sender.clone();
    Box::pin(async move {
      if sender
        .send(DeviceCommunicationEvent::ScanningFinished)
        .await
        .is_err()
      {
        error!("Error sending scanning finished from Xinput.");
      }
      Ok(())
    })
  }
}
