extends Node
class_name ServerHandler

const INSTANCE_QUEUE_ENDPOINT = "/api/v0/server/instance"

const LOCAL_SERVER_KEY_LOCATION:String = "user://local_key.tmp"

# seconds after all players leave before exiting
const INACTIVITY_KILL_THRESHOLD:float = 5.0

var started:bool = false
var finished_starting:bool = false
var agones_health_checking:bool = false
var api_token:PackedByteArray
var max_players:int
var inactivity:float

func _ready() -> void:
	await get_tree().create_timer(5).timeout
	if !started:
		queue_free()

func _physics_process(delta: float) -> void:
	if agones_health_checking:
		AgonesSDK.health()
	if finished_starting and NetworkManager.get_player_count() == 0:
		inactivity += delta
		if inactivity > INACTIVITY_KILL_THRESHOLD:
			push_warning("too long with 0 players: exiting")
			if agones_health_checking:
				AgonesSDK.shutdown()
			else:
				get_tree().quit()
	else:
		inactivity = 0

# this class should do nothing until this function is done
func start(max_players:int, api_token:PackedByteArray, is_local:bool, 
		world:UUID, bind_addr:String, key:PackedByteArray) -> void:
	started = true
	
	if is_local:
		# local instance
		
		await GlobalAccountHandler.set_token(api_token, -1, false)
		
		await GlobalWorldHandler.load_world_server(world, bind_addr, key)
		
		var local_token_file:FileAccess = FileAccess.open(LOCAL_SERVER_KEY_LOCATION, FileAccess.WRITE)
		local_token_file.store_buffer(NetworkManager.get_next_client())
		local_token_file.close()
	else:
		# game server instance
		AgonesSDK.start()
		agones_health_checking = true
		
		while true:
			AgonesSDK.reserve(3)
			var response:Array[Variant] = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, INSTANCE_QUEUE_ENDPOINT, PackedStringArray([GlobalAccountHandler.get_token_header()]))
			var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["uuid", "world_uuid", "token"])
			
			var success:bool = result[0]
			var values:Dictionary[String, Variant] = {}
			values.assign(result[4])
			
			if !success:
				await get_tree().create_timer(6).timeout
				continue
			
			AgonesSDK.allocate()
			
			await GlobalAccountHandler.set_token(values["token"], -1, false)
			
			print("loading world: %s" % values["world_uuid"])
			await GlobalWorldHandler.load_world_server(values["world_uuid"], bind_addr, key)
			break
	
	finished_starting = true
