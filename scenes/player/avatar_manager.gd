extends Node

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
	if owner_id == (await GlobalAccountHandler.get_uuid()).backing_storage:
		# todo: get current avatar from api
		GlobalWorldHandler.current_world.avatar_change_handler.send_message(owner_id, UUID.new())

func change_avatar(target_player:PackedByteArray, avatar:UUID) -> void:
	if target_player != owner_id:
		return
	
	if equiped_avatar != null:
		equiped_avatar.queue_free()
	
	var new_avatar:PackedScene
	
	new_avatar = preload("res://scenes/player/avatar/loading_avatar.tscn")
	equiped_avatar = new_avatar.instantiate()
	
	get_parent().add_child(equiped_avatar)
	
	var thread:Thread = Thread.new()
	thread.start(load_avatar_on_thread.bind(avatar, new_avatar))
	
	await avatar_loaded
	
	thread.wait_to_finish()
	
	current_avatar = avatar
	
	if equiped_avatar:
		equiped_avatar.queue_free()
		await get_tree().physics_frame # dont have both avatars loaded at the same time
	
	if !new_avatar:
		# todo: loading failed avatar
		new_avatar = preload("res://scenes/player/avatar/loading_avatar.tscn")
	
	equiped_avatar = new_avatar.instantiate()
	SetupHelpers.setup_avatar(equiped_avatar, player)
	
	get_parent().add_child(equiped_avatar)
	
	player.avatar_changed.emit()

func load_avatar_on_thread(avatar:UUID, new_avatar:PackedScene) -> void:
	new_avatar = await GlobalDownloadHandler.get_object(avatar, LRUCache.ObjectType.avatar)
	
	if !new_avatar:
		avatar_loaded.emit.call_deferred()
		return
	
	# abort if source avatar is unsafe
	if !SetupHelpers.check_safe(new_avatar.get_state()):
		push_error("tried to load unsafe world, aborting")
		push_error("no error handling here, exiting")
	
	avatar_loaded.emit.call_deferred()
