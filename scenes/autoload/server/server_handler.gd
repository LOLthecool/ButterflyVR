extends Node
class_name ServerHandler

const LOCAL_SERVER_KEY_LOCATION:String = "user://local_key.tmp"

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
			push_warning("too long with 0 players: exiting")
			if agones_sdk:
				agones_sdk.shutdown()
			else:
				get_tree().quit()
	else:
		inactivity = 0

# this class should do nothing until this function is done
func start(api_token:PackedByteArray, is_local:bool, local_world:UUID, 
		local_bind_addr:String, key:PackedByteArray) -> void:
	started = true
	
	if is_local:
		# local instance
		print("binding to address: ", local_bind_addr)
		
		print("set token to %s" % api_token)
		await GlobalAccountHandler.set_token(api_token, -1, false)
		
		await GlobalWorldHandler.load_world_server(local_world, local_bind_addr, key)
		
		var local_token_file:FileAccess = FileAccess.open(LOCAL_SERVER_KEY_LOCATION, FileAccess.WRITE)
		local_token_file.store_buffer(NetworkManager.get_next_client())
		local_token_file.close()
	else:
		# game server instance
		print("starting remote server")
		agones_sdk = AgonesSDK.new()
		add_child(agones_sdk)
		
		var timer:Timer = Timer.new()
		add_child(timer)
		timer.start(3)
		
		var response:Dictionary
		while true:
			print("waiting for allocation...")
			response = agones_sdk.get_gameserver_status()
			
			if "world" in (response["labels"] as Dictionary).keys():
				break
			else:
				await timer.timeout
				continue
		
		print("got allocation")
		var addr:String = response["address"]
		var port:int = response["ports"]["default"]
		var world:UUID = UUID.from_String(response["labels"]["world"])
		var instance_token:PackedByteArray = PackedByteArray(JSON.parse_string(response["labels"]["token"]))
		
		print("addr:", addr)
		print("port:", port)
		print("world:", world)
		print("instancetoken:", instance_token)
		
		await GlobalAccountHandler.set_token(api_token, -1, false)
		
		await GlobalWorldHandler.load_world_server(world, addr + ":" + str(port), key)
	
	finished_starting = true
