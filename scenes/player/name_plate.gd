extends Node3D

@export var player_access:PlayerAccess
var id:PackedByteArray

func _ready() -> void:
	id = player_access.networker.owner_id
	# todo: get username
	set_player_name("Player " + str(id))

func set_player_name(player_name:String) -> void:
	@warning_ignore("unsafe_property_access")
	get_child(0).text = player_name
