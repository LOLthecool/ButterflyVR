extends Node
class_name ServerHandler

const LOCAL_SERVER_KEY_LOCATION:String = "user://local_key.tmp"
const SET_CLIENT_TOKEN_ENDPOINT:String = "/api/v0/internal/token"
const CLOSE_INSTANCE_ENDPOINT:String = "/api/v0/internal/close_instance"

# seconds after all players leave before exiting
const INACTIVITY_KILL_THRESHOLD:float = 15.0

var started:bool = false
var finished_starting:bool = false
var agones_sdk:AgonesSDK = null
var api_token:PackedByteArray
var max_players:int
var inactivity:float

func _ready() -> void:
	await get_tree().create_timer(5).timeout
	if !started:
		queue_free()

func _physics_process(delta: float) -> void:
	if finished_starting and NetworkManager.get_player_count() == 0:
		inactivity += delta
		if inactivity > INACTIVITY_KILL_THRESHOLD:
			inactivity = 0 # avoid spam since shutdown takes multiple frames
			push_warning("too long with 0 players: exiting")
			if agones_sdk:
				await GlobalAPIHandler.make_request(
						HTTPClient.METHOD_GET, 
						CLOSE_INSTANCE_ENDPOINT, 
						PackedStringArray([GlobalAccountHandler.get_token_header()]))
				# TEMP: dont want it to shutdown and delete logs
				while true:
					await get_tree().physics_frame
				agones_sdk.shutdown()
			else:
				get_tree().quit()
	else:
		inactivity = 0

# this class should do nothing until this function is done
func start(api_token:PackedByteArray, is_local:bool, local_world:UUID, 
		local_bind_port:int) -> void:
	started = true
	
	if is_local:
		# local instance
		print("binding to address: 127.0.0.1:%s" % local_bind_port)
		
		print("set token to %s" % api_token)
		# set_token assumes we are a client
		# todo: some way to switch account handler 'mode' between client and server
		GlobalAccountHandler.session_token = api_token
		GlobalAccountHandler.token_expiry_utc = -1
		GlobalAccountHandler.token_renewable = false
		
		await GlobalWorldHandler.load_world_server(local_world, local_bind_port)
		
		var local_token_file:FileAccess = FileAccess.open(LOCAL_SERVER_KEY_LOCATION, FileAccess.WRITE)
		local_token_file.store_buffer(NetworkManager.get_next_client())
		local_token_file.close()
	else:
		# game server instance
		print("starting remote server")
		
		# since we are inside cluster we need to target internal ip + port
		GlobalAPIHandler.target_port = 80
		GlobalAPIHandler.target_host = "butterfly-api.butterfly-api"#.svc.cluster.local"
		GlobalAPIHandler.restart_requested = true
		
		agones_sdk = AgonesSDK.new()
		add_child(agones_sdk)
		
		var timer:Timer = Timer.new()
		add_child(timer)
		timer.start(3)
		
		var agones_response:Dictionary
		while true:
			print("waiting for allocation...")
			agones_response = agones_sdk.get_gameserver_status()
			
			@warning_ignore("unsafe_cast")
			if "world" in (agones_response["labels"] as Dictionary).keys():
				break
			else:
				await timer.timeout
				continue
		
		print("got allocation")
		var port:int = agones_response["ports"]["default"]
		@warning_ignore("unsafe_cast")
		var world:UUID = UUID.from_String(agones_response["labels"]["world"] as String)
		@warning_ignore("unsafe_cast")
		var instance_token:PackedByteArray = (
				agones_response["annotations"]["token"] as String
				).hex_decode()
		
		print("port:", port)
		print("world:", world)
		print("instancetoken:", instance_token)
		
		await GlobalAccountHandler.set_token(instance_token, -1, false)
		
		await GlobalWorldHandler.load_world_server(world, port)
		
		var response:Array[Variant] = await GlobalAPIHandler.make_request(
				HTTPClient.METHOD_POST, 
				SET_CLIENT_TOKEN_ENDPOINT, 
				PackedStringArray([GlobalAccountHandler.get_token_header()]), 
				JSON.stringify({"client_token":NetworkManager.get_next_client() as Array[int]}))
		@warning_ignore("unsafe_call_argument")
		var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], [])
		if !result[0]:
			push_error("error while setting client token")
			if result[1] != -1:
				push_error("server response: %s" % result[1])
			if result[2] != "":
				push_error("error code: %s" % result[2])
			if result[3] != "":
				push_error("error message: %s" % result[3])
			# TEMP: dont want it to shutdown and delete logs
			while true:
				await get_tree().physics_frame
			agones_sdk.shutdown()
			return
		print("ready for connections")
		while true:
			await get_tree().physics_frame
	
	finished_starting = true
