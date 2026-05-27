extends Node

const USER_INFO_ENDPOINT:String = "/api/v0/user/%s"

signal avatar_loaded

@export var networker:PlayerNetworker
@export var player:Player

var owner_id:PackedByteArray
var equiped_avatar:Node3D
var current_avatar:UUID

func _ready() -> void:
	@warning_ignore("unsafe_property_access")
	owner_id = networker.owner_id
	GlobalWorldHandler.current_world.avatar_change_handler.avatar_changed.connect(change_avatar)
	if owner_id == (await GlobalAccountHandler.get_uuid()).backing_storage and !NetworkManager.is_server():
		var response:Array[Variant] = await GlobalAPIHandler.make_request(
				HTTPClient.METHOD_GET, 
				USER_INFO_ENDPOINT % await GlobalAccountHandler.get_uuid(), 
				PackedStringArray([GlobalAccountHandler.get_token_header()]))
		@warning_ignore("unsafe_call_argument")
		var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["avatar"])
		
		@warning_ignore("unsafe_call_argument")
		var avatar_uuid:UUID = UUID.from_String(result[4]["avatar"])
		
		GlobalWorldHandler.current_world.avatar_change_handler.send_message(owner_id, avatar_uuid)

func change_avatar(target_player:PackedByteArray, avatar:UUID) -> void:
	if target_player != owner_id:
		return
	
	if equiped_avatar != null:
		equiped_avatar.queue_free()
	
	var new_avatar:PackedScene
	
	new_avatar = preload("res://scenes/player/avatar/loading_avatar.tscn")
	equiped_avatar = new_avatar.instantiate()
	
	get_parent().add_child(equiped_avatar)
	
	new_avatar = await GlobalDownloadHandler.get_object(avatar, LRUCache.ObjectType.avatar)
	
	
	if !new_avatar:
		# todo: loading failed avatar
		new_avatar = preload("res://scenes/player/avatar/loading_avatar.tscn")
	
	# abort if source avatar is unsafe
	if !SetupHelpers.check_safe(new_avatar.get_state()):
		push_error("tried to load unsafe world, aborting")
		push_error("no error handling here, exiting")
	
	current_avatar = avatar
	
	if equiped_avatar:
		equiped_avatar.queue_free()
		await get_tree().physics_frame # dont have both avatars loaded at the same time
	
	equiped_avatar = new_avatar.instantiate()
	SetupHelpers.setup_avatar(equiped_avatar, player)
	
	get_parent().add_child(equiped_avatar)
	
	player.avatar_changed.emit()
