@abstract
extends RefCounted
class_name CCKMarker

@abstract
func setup(values:Dictionary[String, Variant], target:Node, state:SetupHelpers.SetupState) -> void

@abstract
func get_name() -> String

@abstract
func supports_multiple_copies() -> bool

@abstract
func is_allowed_on(object_type: LRUCache.ObjectType) -> bool
