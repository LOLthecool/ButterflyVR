use std::collections::HashMap;

use agones::{Sdk, Status};
use godot::classes::Engine;
use godot::prelude::*;
use tokio::runtime::{Builder, Runtime};

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {
    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Scene {
            Engine::singleton().register_singleton(
                &AgonesSDK::class_id().to_string_name(),
                &AgonesSDK::new_alloc(),
            );
        }
    }

    fn on_level_deinit(level: InitLevel) {
        if level == InitLevel::Scene {
            let mut engine = Engine::singleton();
            let singleton_name = &AgonesSDK::class_id().to_string_name();
            if let Some(my_singleton) = engine.get_singleton(singleton_name) {
                engine.unregister_singleton(singleton_name);
                my_singleton.free();
            } else {
                godot_error!("Failed to get singleton");
            }
        }
    }
}

#[derive(GodotClass)]
#[class(init, base=Node)]
struct AgonesSDK {
    #[init(val = Builder::new_current_thread().build().unwrap())]
    runtime: Runtime,
    sdk: Option<Sdk>,
    #[init(val = 5.0)]
    health_check_interval: f64,
    health_check_timer: f64,
    base: Base<Node>,
}

#[godot_api]
impl AgonesSDK {
    #[func]
    fn shutdown(&mut self) {
        if let Some(sdk) = self.sdk.as_mut() {
            self.runtime
                .block_on(async { sdk.shutdown().await.expect("failed to shutdown") })
        }
    }
    // does not actually get the full gameserver status, but if we need more info we can just implement it later
    #[func]
    fn get_gameserver_status(&mut self) -> VarDictionary {
        if let Some(sdk) = self.sdk.as_mut() {
            self.runtime.block_on(async {
                let gameserver = sdk
                    .get_gameserver()
                    .await
                    .expect("failed to get gameserver");
                gameserver
                    .status
                    .map(AgonesSDK::status_to_dict)
                    .unwrap_or(VarDictionary::new())
            })
        } else {
            VarDictionary::new()
        }
    }
    fn status_to_dict(status: Status) -> VarDictionary {
        let mut internal_dict: HashMap<String, Variant> = HashMap::new();
        internal_dict.insert("state".to_string(), status.state.to_variant());
        internal_dict.insert("address".to_string(), status.address.to_variant());
        let addresses: Vec<Variant> = status
            .addresses
            .iter()
            .map(|x| vec![x.address.to_variant(), x.r#type.to_variant()].to_variant())
            .collect();
        internal_dict.insert("addresses".to_string(), addresses.to_variant());
        let ports: Vec<Variant> = status
            .ports
            .iter()
            .map(|x| vec![x.name.to_variant(), x.port.to_variant()].to_variant())
            .collect();
        internal_dict.insert("ports".to_string(), ports.to_variant());
        VarDictionary::from(internal_dict.iter())
    }
    #[func]
    fn set_label(&mut self, key: String, value: String) {
        if let Some(sdk) = self.sdk.as_mut() {
            self.runtime.block_on(async {
                sdk.set_label(key, value)
                    .await
                    .expect("failed to set label");
            })
        }
    }
}

#[godot_api]
impl INode for AgonesSDK {
    fn ready(&mut self) {
        self.sdk = Some(self.runtime.block_on(async {
            let mut sdk = agones::Sdk::new(None, None)
                .await
                .expect("failed to connect to SDK server");
            sdk.ready().await.expect("failed to call ready");
            sdk
        }))
    }

    fn physics_process(&mut self, delta: f64) {
        self.health_check_timer += delta;
        if self.health_check_timer > self.health_check_interval && self.sdk.is_some() {
            self.health_check_timer = 0.0;
            self.runtime.block_on(async {
                self.sdk
                    .as_mut()
                    .unwrap()
                    .health_check()
                    .send(())
                    .await
                    .expect("failed health check")
            });
        }
    }
}
