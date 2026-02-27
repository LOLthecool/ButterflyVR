use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use godot::prelude::*;
use moka::{policy::EvictionPolicy, sync::Cache};
use uuid::Uuid;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

#[derive(Copy, Clone)]
struct Pack {
    cache_time_utc: i32,
    size_kb: i32,
}

#[derive(GodotClass)]
#[class(no_init, base=Node)]
struct LRUCache {
    cache: Cache<Uuid, Pack>,
    #[var]
    save_call: Callable,
    #[var]
    load_call: Callable,
    on_destroy_call: Callable,
    destructor_queue: Arc<Mutex<VecDeque<Uuid>>>,
    base: Base<Node>,
}

#[godot_api]
impl LRUCache {
    #[func]
    fn new(
        size_kb: u64,
        save_call: Callable,
        load_call: Callable,
        on_destroy_call: Callable,
    ) -> Gd<Self> {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let queue2 = queue.clone();
        let cache: Cache<Uuid, Pack> = Cache::builder()
            .max_capacity(size_kb)
            .initial_capacity(size_kb as usize / 1024)
            .weigher(|_, v: &Pack| v.size_kb as u32)
            .eviction_policy(EvictionPolicy::lru())
            .eviction_listener(move |k: Arc<Uuid>, _, _| {
                queue2.lock().unwrap().push_back(*k);
            })
            .build();
        Gd::from_init_fn(|base| Self {
            cache,
            save_call,
            load_call,
            on_destroy_call,
            destructor_queue: queue,
            base,
        })
    }

    #[func]
    fn push_front(&mut self, uuid: String, cache_time_utc: i32, size_kb: i32) {
        let Ok(key) = Uuid::try_parse(&uuid) else {
            godot_error!("uuid was not a valid UUID");
            return;
        };
        self.cache.insert(
            key,
            Pack {
                cache_time_utc,
                size_kb,
            },
        );
    }

    #[func]
    fn get(&mut self, uuid: String) -> VarDictionary {
        let Ok(key) = Uuid::try_parse(&uuid) else {
            godot_error!("uuid was not a valid UUID");
            return VarDictionary::new();
        };
        if let Some(pack) = self.cache.get(&key) {
            let mut result = VarDictionary::new();
            result.set("cache_time_utc".to_variant(), pack.cache_time_utc);
            result.set("size_kb".to_variant(), pack.size_kb);
            result
        } else {
            VarDictionary::new()
        }
    }

    #[func]
    fn pop(&mut self, uuid: String) {
        let Ok(key) = Uuid::try_parse(&uuid) else {
            godot_error!("uuid was not a valid UUID");
            return;
        };
        self.cache.invalidate(&key);
    }

    #[func]
    fn save(&self) {
        let mut data = VarDictionary::new();
        for (key, value) in self.cache.iter() {
            let mut sub: VarDictionary = VarDictionary::new();
            sub.set("cache_time_utc", value.cache_time_utc);
            sub.set("size_kb", value.size_kb);
            data.set(key.to_string(), sub);
        }
        self.save_call.call(&[data.to_variant()]);
    }

    #[func]
    fn load(&self) {
        let data = self.load_call.call(&[]);

        let Ok(data) = VarDictionary::try_from_variant(&data) else {
            godot_error!("load_call callable did not return dictionary");
            return;
        };

        for (key, sub) in data.iter_shared() {
            let Ok(key) = String::try_from_variant(&key) else {
                godot_error!("key was not a string");
                return;
            };
            let Ok(key) = Uuid::try_parse(&key) else {
                godot_error!("key was not a valid UUID");
                return;
            };

            let Ok(sub) = VarDictionary::try_from_variant(&sub) else {
                godot_error!("value was not a dictionary");
                return;
            };

            let Some(cache_time_utc) = sub.get("cache_time_utc") else {
                godot_error!("value did not contain cache_time_utc");
                return;
            };
            let Some(size_kb) = sub.get("size_kb") else {
                godot_error!("value did not contain size_kb");
                return;
            };

            let Ok(cache_time_utc) = i32::try_from_variant(&cache_time_utc) else {
                godot_error!("cache_time_utc was not an int");
                return;
            };
            let Ok(size_kb) = i32::try_from_variant(&size_kb) else {
                godot_error!("size_kb was not an int");
                return;
            };

            self.cache.insert(
                key,
                Pack {
                    cache_time_utc: cache_time_utc,
                    size_kb: size_kb,
                },
            );
        }
    }
}

#[godot_api]
impl INode for LRUCache {
    fn physics_process(&mut self, _: f64) {
        while let Some(dropped) = self.destructor_queue.lock().unwrap().pop_front() {
            self.on_destroy_call
                .call(&[dropped.to_string().to_variant()]);
        }
    }
}
