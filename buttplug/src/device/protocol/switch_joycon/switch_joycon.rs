use super::rumble::Rumble;
use crate::{
  core::{
    errors::{ButtplugError, ButtplugDeviceError},
    messages::{self, ButtplugDeviceCommandMessageUnion, DeviceMessageAttributesMap},
  },
  device::{
    protocol::{
      generic_command_manager::GenericCommandManager, ButtplugDeviceResultFuture, ButtplugProtocol,
      ButtplugProtocolCommandHandler, ButtplugProtocolProperties,
    },
    DeviceImpl, DeviceWriteCmd, Endpoint,
  },
};
use futures::future::BoxFuture;
use std::{time::Duration, sync::{Arc, atomic::{AtomicBool, AtomicU8, AtomicU16, Ordering::SeqCst}}};
use tokio::{time::sleep, sync::Mutex};

/// Send command, sub-command, and data (sub-command's arguments) with u8 integers
/// This returns ACK packet for the command or Error.
async fn send_command_raw(
  device: Arc<DeviceImpl>,
  packet_number: u8,
  command: u8,
  sub_command: u8,
  data: &[u8],
  rumble_r: Option<Rumble>,
  rumble_l: Option<Rumble>,
) -> Result<(), ButtplugError> {
  let mut buf = [0x0; 0x40];
  // set command
  buf[0] = command;
  // set packet number
  buf[1] = packet_number;

  // rumble
  if let Some(rumble_l) = rumble_l {
    let rumble_left: [u8; 4] = rumble_l.into();
    buf[2..6].copy_from_slice(&rumble_left);
  }
  if let Some(rumble_r) = rumble_r {
    let rumble_right: [u8; 4] = rumble_r.into();
    buf[6..10].copy_from_slice(&rumble_right);
  }

  // set sub command
  buf[10] = sub_command;
  // set data
  buf[11..11 + data.len()].copy_from_slice(data);

  // send command
  device
    .write_value(DeviceWriteCmd::new(Endpoint::Tx, buf.to_vec(), false))
    .await
    .unwrap();
  Ok(())
}

/// Send sub-command, and data (sub-command's arguments) with u8 integers
/// This returns ACK packet for the command or Error.
///
/// # Notice
/// If you are using non-blocking mode,
/// it is more likely to fail to validate the sub command reply.
async fn send_sub_command_raw(
  device: Arc<DeviceImpl>,
  packet_number: u8,
  sub_command: u8,
  data: &[u8],
) -> Result<(), ButtplugError> {
  //use input_report_mode::sub_command_mode::AckByte;

  send_command_raw(device, packet_number, 1, sub_command, data, None, None).await
  /*
  // check reply
  if self.valid_reply() {
      std::iter::repeat(())
          .take(Self::ACK_TRY)
          .flat_map(|()| {
              let mut buf = [0u8; 362];
              self.read(&mut buf).ok()?;
              let ack_byte = AckByte::from(buf[13]);

              match ack_byte {
                  AckByte::Ack { .. } => Some(buf),
                  AckByte::Nack => None
              }
          })
          .next()
          .map(SubCommandReply::Checked)
          .ok_or_else(|| JoyConError::SubCommandError(sub_command, Vec::new()))
  } else {
      Ok(SubCommandReply::Unchecked)
  }
  */
}

/// Send command, sub-command, and data (sub-command's arguments) with `Command` and `SubCommand`
/// This returns ACK packet for the command or Error.
///
/// # Notice
/// If you are using non-blocking mode,
/// it is more likely to fail to validate the sub command reply.
async fn send_command(
  device: Arc<DeviceImpl>,
  packet_number: u8,
  command: u8,
  sub_command: u8,
  data: &[u8],
) -> Result<(), ButtplugError> {
  send_command_raw(
    device,
    packet_number,
    command,
    sub_command,
    data,
    None,
    None,
  )
  .await
}

/// Send sub-command, and data (sub-command's arguments) with `Command` and `SubCommand`
/// This returns ACK packet for the command or Error.
async fn send_sub_command(
  device: Arc<DeviceImpl>,
  packet_number: u8,
  sub_command: u8,
  data: &[u8],
) -> Result<(), ButtplugError> {
  send_sub_command_raw(device, packet_number, sub_command as u8, data).await
}

#[derive(ButtplugProtocolProperties)]
pub struct SwitchJoycon {
  name: String,
  message_attributes: DeviceMessageAttributesMap,
  manager: Arc<Mutex<GenericCommandManager>>,
  stop_commands: Vec<ButtplugDeviceCommandMessageUnion>,
  packet_number: Arc<AtomicU8>,
  is_running: Arc<AtomicBool>,
  speed_val: Arc<AtomicU16>,
}

impl ButtplugProtocol for SwitchJoycon {
  fn new_protocol(
    name: &str,
    message_attributes: DeviceMessageAttributesMap,
  ) -> Box<dyn ButtplugProtocol>
  where
    Self: Sized,
  {
    let manager = GenericCommandManager::new(&message_attributes);

    Box::new(Self {
      name: name.to_owned(),
      message_attributes,
      stop_commands: manager.get_stop_commands(),
      manager: Arc::new(Mutex::new(manager)),
      packet_number: Arc::new(AtomicU8::new(0)),
      is_running: Arc::new(AtomicBool::new(false)),
      speed_val: Arc::new(AtomicU16::new(0))
    })
  }

  fn initialize(
    device_impl: Arc<DeviceImpl>,
  ) -> BoxFuture<'static, Result<Option<String>, ButtplugError>> {
    Box::pin(async move {
      // Turn on vibration
      send_sub_command(device_impl.clone(), 0, 72, &[0x01]).await.map_err(|_| ButtplugDeviceError::DeviceConnectionError("Cannot initialize joycon".to_owned()))?;
      Ok(None)
    })
  }
}

impl ButtplugProtocolCommandHandler for SwitchJoycon {
  fn handle_vibrate_cmd(
    &self,
    device: Arc<DeviceImpl>,
    message: messages::VibrateCmd,
  ) -> ButtplugDeviceResultFuture {
    let manager = self.manager.clone();
    let is_running = self.is_running.clone();
    let speed_val = self.speed_val.clone();
    Box::pin(async move {
      if message.speeds()[0].speed() <= 0.001 {
        is_running.store(false, SeqCst);
        send_command_raw(device, 1, 16, 0, &[], Some(Rumble::stop()), Some(Rumble::stop())).await?;
      } else if !is_running.load(SeqCst) {
        is_running.store(true, SeqCst);
        tokio::spawn(async move {
          loop {
            if !is_running.load(SeqCst) {
              return;
            }
            let amp = speed_val.load(SeqCst) as f32 / 1000f32;
            if let Err(e) = send_command_raw(device.clone(), 1, 16, 0, &[], Some(Rumble::new(200.0f32, amp)), Some(Rumble::new(200.0f32, amp))).await {
              return;
            }
            sleep(Duration::from_millis(250)).await;
          }
        });
      } else {
        speed_val.store((message.speeds()[0].speed() * 1000f64) as u16, SeqCst);
      }
      //let result = manager.lock().await.update_vibration(&message, false)?;
      //send_command_raw(device, 1, 16, 0, &[], Some(Rumble::new(300.0f32, (1.0 * message.speeds()[0].speed()) as f32)), Some(Rumble::new(300.0f32, (1.0 * message.speeds()[0].speed()) as f32))).await?;
      Ok(messages::Ok::default().into())
    })
  }
}
