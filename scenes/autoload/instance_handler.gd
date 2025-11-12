extends Node
class_name InstanceHandler

const INSTANCE_CREATION_ENDPOINT:String = "api/v0/instance"
const INSTANCE_JOIN_ENDPOINT:String = "api/v0/instance/%s/join"
const OFFLINE_INSTANCE_CMD_ARGUMENTS:Array[String] = ["--headless", "--server"]
const LOCAL_SERVER_KEY_LOCATION:String = "user://local_key.tmp"

enum InstanceJoinPermission{
	public,
	group,
	friends,
	invite
}

const STATUS_REFRESH_RATE:int = 30

var current_instance:UUID
var local_server_pid:int = -1

func create_online_instance(
		world_uuid:UUID, join_permission:InstanceJoinPermission, 
		anyone_can_invite:bool, is_gameserver:bool) -> UUID:
	var body_dict:Dictionary[String, Variant] = {"world":world_uuid, 
			"join_permission":join_permission, 
			"anyone_can_invite":anyone_can_invite, 
			"is_gameserver":is_gameserver}
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_POST, 
			INSTANCE_CREATION_ENDPOINT, 
			PackedStringArray([GlobalAccountHandler.get_token_header()]), 
			JSON.stringify(body_dict))
	var result:Array[Variant] = GlobalAPIHandler.handle_response(
			response[0], response[2], [200], ["instance_uuid"])
	return result[4][0]

func create_and_join_offline_instance(world_uuid:UUID) -> void:
	var arguments:PackedStringArray = PackedStringArray(OFFLINE_INSTANCE_CMD_ARGUMENTS)
	var world_argument:String = "--world=%s" % world_uuid
	arguments.push_back(world_argument)
	local_server_pid = OS.create_instance(arguments)
	while !FileAccess.file_exists(LOCAL_SERVER_KEY_LOCATION):
		await get_tree().physics_frame
	var local_server_token:PackedByteArray = FileAccess.get_file_as_bytes(
			LOCAL_SERVER_KEY_LOCATION)
	NetworkManager.start_client(local_server_token)

func join_instance(instance:UUID) -> void:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, 
			INSTANCE_JOIN_ENDPOINT, 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	var result:Array[Variant] = GlobalAPIHandler.handle_response(
			response[0], response[2], [200], ["join_token"])
	var token_string:String = result[4][0]
	var token:PackedByteArray = PackedByteArray()
	for idx:int in range(0, token_string.length(), 2):
		token.push_back(token_string.substr(idx, 2).hex_to_int())
	NetworkManager.start_client(token)
