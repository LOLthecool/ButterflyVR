use std::collections::HashMap;

use agones::{GameServer, Sdk};
use godot::prelude::*;
use tokio::{
    runtime::{Builder, Runtime},
    sync::OnceCell,
};

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

#[derive(GodotClass)]
#[class(base=Node)]
struct AgonesSDK {
    runtime: Runtime,
    // if we create the sdk on init the editor and client freeze while waiting for the timeout
    // since we should have the sdk available if its functions are called this should work fine
    sdk: OnceCell<Sdk>,
    health_check_interval: f64,
    health_check_timer: f64,
    base: Base<Node>,
}

#[godot_api]
impl AgonesSDK {
    #[func]
    fn shutdown(&mut self) {
        self.runtime.block_on(async {
            self.sdk.get_or_init(AgonesSDK::init_sdk).await;
            self.sdk
                .get_mut()
                .unwrap()
                .shutdown()
                .await
                .expect("failed to shutdown")
        })
    }
    // does not actually get the full gameserver status, but if we need more info we can just implement it later
    #[func]
    fn get_gameserver_status(&mut self) -> VarDictionary {
        self.runtime.block_on(async {
            self.sdk.get_or_init(AgonesSDK::init_sdk).await;
            self.sdk
                .get_mut()
                .unwrap()
                .get_gameserver()
                .await
                .map(AgonesSDK::status_to_dict)
                .unwrap_or_else(|_| {
                    godot_warn!("Failed to unwrap gameserver status");
                    VarDictionary::new()
                })
        })
    }
    fn status_to_dict(gameserver: GameServer) -> VarDictionary {
        let mut internal_dict: HashMap<String, Variant> = HashMap::new();

        let status = gameserver.status.expect("gameserver didnt have status");
        internal_dict.insert("state".to_string(), status.state.to_variant());

        internal_dict.insert("address".to_string(), status.address.to_variant());

        let ports: HashMap<GString, Variant> = status
            .ports
            .iter()
            .flat_map(|x| {
                let mut map = HashMap::new();
                map.insert(x.name.to_godot(), x.port.to_variant());
                map
            })
            .collect();
        let ports = VarDictionary::from(ports.iter());
        internal_dict.insert("ports".to_string(), ports.to_variant());

        let labels = gameserver
            .object_meta
            .expect("gameserver didnt have metadata")
            .labels;
        internal_dict.insert(
            "labels".to_string(),
            VarDictionary::from(labels.iter()).to_variant(),
        );

        VarDictionary::from(internal_dict.iter())
    }
    async fn init_sdk() -> Sdk {
        let mut sdk = agones::Sdk::new(None, None)
            .await
            .expect("failed to connect to SDK server");
        sdk.ready().await.expect("failed to call ready");
        sdk
    }
    #[func]
    fn set_annotation(&mut self, key: String, value: String) {
        self.runtime.block_on(async {
            self.sdk.get_or_init(AgonesSDK::init_sdk).await;
            self.sdk
                .get_mut()
                .unwrap()
                .set_annotation(key, value)
                .await
                .expect("failed to set label");
        })
    }
}

#[godot_api]
impl INode for AgonesSDK {
    fn init(base: Base<Node>) -> Self {
        Self {
            runtime: Builder::new_current_thread().enable_all().build().unwrap(),
            sdk: OnceCell::new(),
            health_check_interval: 5.0,
            health_check_timer: 0.0,
            base,
        }
    }

    fn physics_process(&mut self, delta: f64) {
        self.health_check_timer += delta;
        if self.health_check_timer > self.health_check_interval {
            self.health_check_timer = 0.0;
            self.runtime.block_on(async {
                self.sdk.get_or_init(AgonesSDK::init_sdk).await;
                self.sdk
                    .get_mut()
                    .unwrap()
                    .health_check()
                    .send(())
                    .await
                    .expect("failed health check")
            });
        }
    }
}
