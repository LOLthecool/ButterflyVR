extends Button

@export var world_list:WorldList

func _pressed() -> void:
	world_list.get_and_show_worlds("", {"creator": (await GlobalAccountHandler.get_uuid()).to_string()})
