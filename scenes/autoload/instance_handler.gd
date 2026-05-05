extends Node
class_name InstanceHandler

const INSTANCE_CREATION_ENDPOINT:String = "/api/v0/instances"
const INSTANCE_JOIN_ENDPOINT:String = "/api/v0/instances/%s/join"
const OFFLINE_INSTANCE_CMD_ARGUMENTS:Array[String] = ["--server", "--local", "--headless", "--log-file server.log"]
const MAX_CONNECT_RETRYS:int = 10

enum InstanceJoinPermission{
	public,
	group,
	friends,
	invite
}

const STATUS_REFRESH_RATE:int = 30

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
	
	if FileAccess.file_exists(ServerHandler.LOCAL_SERVER_KEY_LOCATION):
		DirAccess.remove_absolute(ServerHandler.LOCAL_SERVER_KEY_LOCATION)
	
	var local_server_pid:int = OS.create_instance(arguments)
	
	if local_server_pid == -1:
		push_error("failed to create local instance")
	
	while !FileAccess.file_exists(ServerHandler.LOCAL_SERVER_KEY_LOCATION):
		await get_tree().physics_frame
	
	var local_server_token:PackedByteArray = FileAccess.get_file_as_bytes(
			ServerHandler.LOCAL_SERVER_KEY_LOCATION)
	
	assert(local_server_token.size() == 40)
	
	NetworkManager.start_client(
			"127.0.0.1", 
			port, 
			(await GlobalAccountHandler.get_uuid()).backing_storage, 
			local_server_token.slice(0, 8), 
			local_server_token.slice(8, 40))

# do not call directly, call load_world instead
func join_instance(instance:UUID) -> void:
	for i:int in range(0, MAX_CONNECT_RETRYS):
		var response:Array[Variant] = await GlobalAPIHandler.make_request(
				HTTPClient.METHOD_GET, 
				INSTANCE_JOIN_ENDPOINT % instance.to_string(), 
				PackedStringArray([GlobalAccountHandler.get_token_header()]))
		
		@warning_ignore("unsafe_call_argument")
		var result:Array[Variant] = GlobalAPIHandler.handle_response(
				response[0], response[2], [200], ["ip", "port", "token"])
		
		if !result[0]:
			if result[1] == 202 and i + 1 < MAX_CONNECT_RETRYS:
				push_warning("no connect token available, retrying")
				await get_tree().create_timer(3).timeout
				continue
			
			push_error("error while joining an online instance")
			if result[1] != -1:
				push_error("server response: %s" % result[1])
			if result[2] != "":
				push_error("error code: %s" % result[2])
			if result[3] != "":
				push_error("error message: %s" % result[3])
			return
		
		var ip:String = result[4]["ip"]
		var port:int = result[4]["port"]
		@warning_ignore("unsafe_cast")
		var token:PackedByteArray = PackedByteArray(result[4]["token"] as Array)
		
		assert(token.size() == 40)
	
		NetworkManager.start_client(
			ip, 
			port, 
			(await GlobalAccountHandler.get_uuid()).backing_storage, 
			token.slice(0, 8), 
			token.slice(8, 40))
		break
