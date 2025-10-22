extends Node
class_name WorldHandler

var current_world:WorldController

func load_homeworld() -> void:
	pass

func load_world(world_id:UUID) -> void:
	pass

func disconnect_from_world(go_home:bool = true) -> void:
	pass
