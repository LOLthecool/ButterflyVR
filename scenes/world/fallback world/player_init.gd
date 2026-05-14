extends Node
# handles initializing the player since otherwise it waits forever for the server to start

@export var player:Player

func _ready() -> void:
	player.init_local()
	await get_tree().physics_frame
	await get_tree().physics_frame
	player.get_child(1).get_child(2).queue_free()
