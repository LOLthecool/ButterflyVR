extends Node
class_name ServerHandler

const LOCAL_SERVER_KEY_LOCATION:String = "user://local_key.tmp"
# seconds after all players leave before exiting
const INACTIVITY_KILL_THRESHOLD:float = 5.0

var started:bool = false
var finished_starting:bool = false
var api_token:PackedByteArray
var max_players:int
var inactivity:float

func _ready() -> void:
	await get_tree().create_timer(5).timeout
	if !started:
		queue_free()

func _process(delta: float) -> void:
	if finished_starting and NetworkManager.get_player_count() == 0:
		inactivity += delta
		if inactivity > INACTIVITY_KILL_THRESHOLD:
			push_warning("too long with 0 players: exiting")
			get_tree().quit()
	else:
		inactivity = 0

# this should do nothing until this function is done
func start(max_players:int, api_token:PackedByteArray, is_local:bool, 
		world:UUID, bind_addr:String, key:PackedByteArray) -> void:
	started = true
	
	await GlobalAccountHandler.set_token(api_token, -1, false)
	
	print("loading world: %s" % world)
	await GlobalWorldHandler.load_world_server(world, bind_addr, key)
	
	if is_local:
		# local instance
		var local_token_file:FileAccess = FileAccess.open(LOCAL_SERVER_KEY_LOCATION, FileAccess.WRITE)
		local_token_file.store_buffer(NetworkManager.get_next_client())
		local_token_file.flush()
		local_token_file.close()
	else:
		# game server instance
		pass # todo: verify instance token, open socket and register client tokens with api
	finished_starting = true
