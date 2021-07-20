use android_utils::os::JHandler;
use futures::{
  future::{FutureObj, RemoteHandle},
  task::{Spawn, SpawnError, SpawnExt},
};
use jni::{
  errors::Error,
  objects::{GlobalRef, JObject},
  JNIEnv, JavaVM,
};
use once_cell::sync::OnceCell;
use std::future::Future;

struct SpawnImpl {
  vm: JavaVM,
  handler: GlobalRef,
}

static SPAWNER: OnceCell<SpawnImpl> = OnceCell::new();

pub fn init<'a: 'b, 'b>(env: &'b JNIEnv<'a>, handler: JObject<'a>) -> Result<(), Error> {
  let vm = env.get_java_vm()?;
  let handler = env.new_global_ref(handler)?;
  let _ = SPAWNER.set(SpawnImpl { vm, handler });
  Ok(())
}

#[derive(Default)]
pub struct AndroidAsyncManager {}

impl Spawn for AndroidAsyncManager {
  fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
    let global = SPAWNER.get().unwrap();
    let env = global.vm.get_env().unwrap();
    let handler = JHandler::from_env(&env, global.handler.as_obj()).unwrap();
    let spawner = handler.spawner();
    spawner.spawn_obj(future)
  }
}

pub fn spawn<Fut>(future: Fut) -> Result<(), SpawnError>
where
  Fut: Future<Output = ()> + Send + 'static,
{
  AndroidAsyncManager::default().spawn(future)
}

pub fn spawn_with_handle<Fut>(future: Fut) -> Result<RemoteHandle<Fut::Output>, SpawnError>
where
  Fut: Future + Send + 'static,
  Fut::Output: Send,
{
  AndroidAsyncManager::default().spawn_with_handle(future)
}

pub fn block_on<F>(_f: F) -> <F as Future>::Output
where
  F: Future,
{
  unimplemented!("Can't block on Android!")
}
