extends Node
class_name InstanceHandler

const INSTANCE_CREATION_ENDPOINT:String = "/api/v0/instances"
const INSTANCE_JOIN_ENDPOINT:String = "/api/v0/instances/%s/join"

const OFFLINE_INSTANCE_CMD_ARGUMENTS:Array[String] = ["--server", "--local", "--headless", "--log-file", "user://logs/server.log"]

enum InstanceJoinPermission{
	InviteOnly,
	Friends,
	FriendsOfFriends,
	Public
}

var current_instance:UUID

func create_online_instance(
		world_uuid:UUID, join_permission:InstanceJoinPermission, 
		anyone_can_invite:bool, is_gameserver:bool, 
		instance_name:String, max_players:int) -> UUID:
	var body_dict:Dictionary[String, Variant] = {"world":world_uuid, 
			"publicity":join_permission, 
			"anyone_can_invite":anyone_can_invite, 
			"is_gameserver":is_gameserver,
			"name":instance_name,
			"max_players":max_players}
	
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_POST, 
			INSTANCE_CREATION_ENDPOINT, 
			PackedStringArray([GlobalAccountHandler.get_token_header()]), 
			JSON.stringify(body_dict))
	
	@warning_ignore("unsafe_call_argument")
	var result:Array[Variant] = GlobalAPIHandler.handle_response(
			response[0], response[2], [200], ["id"])
	if !result[0]:
		push_error("error while creating an online instance")
		if result[1] != -1:
			push_error("server response: %s" % result[1])
		if result[2] != "":
			push_error("error code: %s" % result[2])
		if result[3] != "":
			push_error("error message: %s" % result[3])
		return null
	
	@warning_ignore("unsafe_cast")
	return UUID.from_String(result[4]["id"] as String)

# do not call directly, call load_world with instance_id == null instead
func create_and_join_offline_instance(world_uuid:UUID) -> void:
	var port:int = randi_range(20000, 30000)
	
	var arguments:PackedStringArray = PackedStringArray(OFFLINE_INSTANCE_CMD_ARGUMENTS)
	
	var world_argument:String = "--world=%s" % world_uuid
	arguments.push_back(world_argument)
	
	var self_pid_argument:String = "--owner_pid=%s" % OS.get_process_id()
	arguments.push_back(self_pid_argument)
	
	var token_argument:String = "--api_token=%s" % GlobalAccountHandler.session_token.hex_encode()
	arguments.push_back(token_argument)
	
	var port_argument:String = "--bind_port=%s" % port
	arguments.push_back(port_argument)
	
	var local_server_pid:int = OS.create_instance(arguments)
	
	if local_server_pid == -1:
		push_error("failed to create local instance")
	
	NetworkManager.start_client(
			"127.0.0.1", 
			port, 
			(await GlobalAccountHandler.get_uuid()).backing_storage, 
			(await GlobalAccountHandler.get_uuid()).backing_storage.slice(0, 8))

# do not call directly, call load_world instead
func join_instance(instance:UUID) -> bool:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, 
			INSTANCE_JOIN_ENDPOINT % instance.to_string(), 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	
	@warning_ignore("unsafe_call_argument")
	var result:Array[Variant] = GlobalAPIHandler.handle_response(
			response[0], response[2], [200], ["ip", "port", "identifier"])
	
	if !result[0]:
		push_error("error while joining an online instance")
		if result[1] != -1:
			push_error("server response: %s" % result[1])
		if result[2] != "":
			push_error("error code: %s" % result[2])
		if result[3] != "":
			push_error("error message: %s" % result[3])
		return false
	
	var ip:String = result[4]["ip"]
	var port:int = result[4]["port"]
	@warning_ignore("unsafe_cast")
	var identifier:PackedByteArray = PackedByteArray(result[4]["identifier"] as Array)
	
	assert(ip.split("/")[0].is_valid_ip_address())
	assert(port > 0 and port < 65_535)
	assert(identifier.size() == 8)

	NetworkManager.start_client(
			ip.split("/")[0], 
			port, 
			(await GlobalAccountHandler.get_uuid()).backing_storage, 
			identifier, )
	return true
