use std::collections::HashMap;

use godot::prelude::{ExtensionLibrary, gdextension};
use serde::{Deserialize, Serialize};

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct IndexPath {
    path: Vec<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Vector3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct AABB {
    position: Vector3,
    size: Vector3,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Basis {
    x: Vector3,
    y: Vector3,
    z: Vector3,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Color {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Plane {
    d: f32,
    normal: Vector3,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Projection {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Quaternion {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Rect2 {
    position: Vector2,
    size: Vector2,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Rect2i {
    position: Vector2i,
    size: Vector2i,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Transform2D {
    origin: Vector2,
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Transform3D {
    basis: Basis,
    origin: Vector3,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Vector2 {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Vector2i {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Vector3i {
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Vector4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Vector4i {
    x: i32,
    y: i32,
    z: i32,
    w: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
enum Variant {
    #[default]
    Nil,
    AABB(AABB),
    Array(Vec<Variant>),
    Basis(Basis),
    Bool(bool),
    Color(Color),
    Dictionary(Vec<(Variant, Variant)>),
    Float(f64),
    Int(i64),
    NodePath(IndexPath),
    Object(IndexPath),
    PackedArray(Vec<Variant>),
    Plane(Plane),
    Projection(Projection),
    Quaternion(Quaternion),
    Rect2(Rect2),
    Rect2i(Rect2i),
    String(String),
    StringName(String),
    Transform2D(Transform2D),
    Transform3D(Transform3D),
    Vector2(Vector2),
    Vector2i(Vector2i),
    Vector3(Vector3),
    Vector3i(Vector3i),
    Vector4(Vector4),
    Vector4i(Vector4i),
}
