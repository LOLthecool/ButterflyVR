extends Node3D

@export var player_access:PlayerAccess
var id:PackedByteArray

func _ready() -> void:
	id = player_access.networker.owner_id
	set_player_name(await APIHelper.get_username(UUID.from_bytes(id)))

func set_player_name(player_name:String) -> void:
	@warning_ignore("unsafe_property_access")
	get_child(0).text = player_name
