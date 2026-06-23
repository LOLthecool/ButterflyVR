extends Node

@export var player_access:PlayerAccess
@export var nameplate:NamePlate

func _physics_process(_delta: float) -> void:
	player_access.player.position = player_access.player.server_position
	player_access.player.rotation = player_access.player.server_rotation
	player_access.player.velocity = player_access.player.server_velocity
	if player_access.player.head_ik_target:
		nameplate.position = player_access.player.head_ik_target.position
		nameplate.position.y *= 1.2
