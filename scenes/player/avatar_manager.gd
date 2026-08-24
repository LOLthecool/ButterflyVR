extends Node

const USER_INFO_ENDPOINT: String = "/api/v0/user/%s"
const USER_AVATAR_ENDPOINT: String = "/api/v0/this_user/avatar"

signal avatar_loaded

@export var networker: PlayerNetworker
@export var player: Player

var owner_id: PackedByteArray
var avatar_root: Node3D
var avatar_uuid: UUID


func _ready() -> void:
	@warning_ignore("unsafe_property_access")
	owner_id = networker.owner_id
	GlobalWorldHandler.current_world.avatar_change_handler.avatar_changed.connect(change_avatar)
	if (
		owner_id == (await GlobalAccountHandler.get_uuid()).backing_storage
		and !NetworkManager.is_server()
	):
		var response: Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET,
			USER_INFO_ENDPOINT % await GlobalAccountHandler.get_uuid(),
			PackedStringArray([GlobalAccountHandler.get_token_header()]),
		)
		@warning_ignore("unsafe_call_argument") var result: Array[Variant] = GlobalAPIHandler.handle_response(
			response[0],
			response[2],
			[200],
			["avatar"],
		)

		if !result[0]:
			push_error("failed to aquire avatar id")
			if result[1] != -1:
				push_error("server response: %s" % result[1])
			if result[2] != "":
				push_error("error code: %s" % result[2])
			if result[3] != "":
				push_error("error message: %s" % result[3])
			GlobalWorldHandler.current_world.avatar_change_handler.send_message(
				owner_id,
				UUID.from_bytes(PackedByteArray([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])),
			)
			return

		@warning_ignore("unsafe_call_argument") var avatar_uuid: UUID = UUID.from_String(
			result[4]["avatar"]
		)

		GlobalWorldHandler.current_world.avatar_change_handler.send_message(owner_id, avatar_uuid)


func change_avatar(target_player: PackedByteArray, avatar: UUID) -> void:
	if target_player != owner_id:
		return

	if avatar_root != null:
		avatar_root.queue_free()

	var new_avatar: PackedScene

	new_avatar = preload("res://scenes/player/loading_avatar.tscn")
	avatar_root = new_avatar.instantiate()

	get_parent().add_child(avatar_root)

	new_avatar = await GlobalDownloadHandler.get_object(avatar, TypeHelper.ObjectType.avatar)

	if !new_avatar:
		# todo: specific 'loading failed' avatar
		new_avatar = preload("res://scenes/player/loading_avatar.tscn")

	# set current avatar in api
	if (
		owner_id == (await GlobalAccountHandler.get_uuid()).backing_storage
		and !NetworkManager.is_server()
	):
		var response: Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_POST,
			USER_AVATAR_ENDPOINT,
			PackedStringArray([GlobalAccountHandler.get_token_header()]),
			JSON.stringify({ "uuid": avatar.to_string() }),
		)
		@warning_ignore("unsafe_call_argument") var result: Array[Variant] = GlobalAPIHandler.handle_response(
			response[0],
			response[2],
			[200],
			[],
		)

		if !result[0]:
			push_error("failed to update current avatar")
			if result[1] != -1:
				push_error("server response: %s" % result[1])
			if result[2] != "":
				push_error("error code: %s" % result[2])
			if result[3] != "":
				push_error("error message: %s" % result[3])

	avatar_uuid = avatar

	if avatar_root:
		avatar_root.queue_free()
		await get_tree().physics_frame # dont have both avatars loaded at the same time

	avatar_root = new_avatar.instantiate()
	SetupHelpers.setup_avatar(avatar_root, player)

	get_parent().add_child(avatar_root)

	player.avatar_changed.emit()
