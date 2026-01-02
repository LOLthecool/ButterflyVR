extends Node
class_name ServerHandler

const LOCAL_SERVER_KEY_LOCATION:String = "user://local_key.tmp"

var started:bool = false
var api_token:PackedByteArray
var max_players:int
var owner_pid:int = -1

# this should do nothing until this function is called
func start(max_players:int, api_token:PackedByteArray, owner_pid:int, is_local:bool) -> void:
	print("starting server handler")
	
	self.owner_pid = owner_pid
	started = true
	
	GlobalAccountHandler.set_token(api_token, -1, false)
	
	if is_local:
		# local instance
		var local_token_file:FileAccess = FileAccess.open(LOCAL_SERVER_KEY_LOCATION, FileAccess.WRITE)
		local_token_file.store_buffer(NetworkManager.get_next_client())
		local_token_file.flush()
		local_token_file.close()
	else:
		# game server instance
		return # todo: verify instance token, open socket and register client tokens with api
