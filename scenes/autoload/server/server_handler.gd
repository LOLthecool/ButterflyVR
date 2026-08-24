extends Node
class_name ServerHandler

const LOCAL_SERVER_KEY_LOCATION: String = "user://local_key.tmp"
const CLOSE_INSTANCE_ENDPOINT: String = "/api/v0/internal/close_instance"
const IDENTIFIER_VERIFY_ENDPOINT: String = "/api/v0/internal/verify_identifier/%s"
const IDENTIFIER_ID_ENDPOINT: String = "/api/v0/internal/instance_id"

# seconds after all players leave before exiting
const INACTIVITY_KILL_THRESHOLD: float = 30.0

var started: bool = false
var finished_starting: bool = false
var agones_sdk: AgonesSDK = null
var api_token: PackedByteArray
var instance_id: UUID
var inactivity: float
var is_gameserver: bool = false
var client_verification_mutex: Mutex = Mutex.new()


func _physics_process(delta: float) -> void:
	if !finished_starting:
		return

	# continues until break case, unless already running (from previous frame because async)
	while client_verification_mutex.try_lock():
		var client_id: PackedByteArray = NetworkManager.get_unverified_client()

		if client_id == PackedByteArray():
			client_verification_mutex.unlock()
			break

		if !agones_sdk:
			# our token is the same as the user for a local server
			NetworkManager.verify_client(
				client_id,
				(await GlobalAccountHandler.get_uuid()).backing_storage,
			)
			client_verification_mutex.unlock()
			break

		var response: Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET,
			IDENTIFIER_VERIFY_ENDPOINT % client_id.hex_encode(),
			PackedStringArray([GlobalAccountHandler.get_token_header()]),
		)
		@warning_ignore("unsafe_call_argument") var result: Array[Variant] = GlobalAPIHandler.handle_response(
			response[0],
			response[2],
			[200],
			["user_id"],
		)

		if !result[0]:
			push_warning("rejecting client: invalid identifier")
			NetworkManager.reject_client(client_id)

			if result[1] != 404:
				@warning_ignore("unsafe_cast")
				MiscHelpers.log_request_error(
					"error while getting a client identifier",
					result[1] as int,
					result[2] as String,
					result[3] as String,
				)

		@warning_ignore("unsafe_cast")
		NetworkManager.verify_client(
			client_id,
			UUID.from_String(result[4]["user_id"] as String).backing_storage,
		)

		client_verification_mutex.unlock()

	if NetworkManager.get_player_count() == 0:
		inactivity += delta

		if inactivity > INACTIVITY_KILL_THRESHOLD or (!agones_sdk and inactivity > 5):
			inactivity = 0 # avoid spam since shutdown takes multiple frames
			push_warning("too long with 0 players: exiting")

			if agones_sdk:
				await GlobalAPIHandler.make_request(
					HTTPClient.METHOD_GET,
					CLOSE_INSTANCE_ENDPOINT,
					PackedStringArray([GlobalAccountHandler.get_token_header()]),
				)
				agones_sdk.shutdown()
			else:
				get_tree().quit()
	else:
		inactivity = 0


# this autoload should do nothing until this function has run
func start(
	api_token: PackedByteArray,
	is_local: bool,
	local_world: UUID,
	local_bind_port: int,
) -> void:
	started = true

	if is_local:
		print("binding to address: 127.0.0.1:%s" % local_bind_port)

		# set_token assumes we are a client
		# todo: some way to switch account handler 'mode' between client and server
		GlobalAccountHandler.session_token = api_token
		GlobalAccountHandler.token_expiry_utc = -1
		GlobalAccountHandler.token_renewable = false
		print("set token to %s" % api_token)

		instance_id = UUID.new()

		await GlobalWorldHandler.load_world_server(local_world, local_bind_port)
	else:
		print("starting remote server")
		is_gameserver = true

		# since we are inside cluster we need to target internal ip + port
		# todo: this should be a single function call
		GlobalAPIHandler.target_port = 80
		GlobalAPIHandler.target_host = "butterfly-api.butterfly-api"
		GlobalAPIHandler.restart_requested = true

		agones_sdk = AgonesSDK.new()
		add_child(agones_sdk)

		await ready
		await get_tree().physics_frame

		var timer: Timer = Timer.new()
		add_child(timer)
		timer.start(1)

		print("waiting for allocation...")

		var agones_response: Dictionary

		while true:
			agones_response = agones_sdk.get_gameserver_status()
			@warning_ignore("unsafe_cast")
			if "world" in (agones_response["labels"] as Dictionary).keys():
				break

			await timer.timeout
			continue

		print("got allocation")

		var port: int = agones_response["ports"]["default"]

		@warning_ignore("unsafe_cast") var world: UUID = UUID.from_String(
			agones_response["labels"]["world"] as String
		)

		@warning_ignore("unsafe_cast") var instance_token: PackedByteArray = (
			agones_response["annotations"]["token"] as String
		).hex_decode()

		print("port:", port)
		print("world:", world)
		print("instance token:", instance_token)

		await GlobalAccountHandler.set_token(instance_token, -1, false)

		var response: Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET,
			IDENTIFIER_ID_ENDPOINT,
			PackedStringArray([GlobalAccountHandler.get_token_header()]),
		)
		@warning_ignore("unsafe_call_argument") var result: Array[Variant] = GlobalAPIHandler.handle_response(
			response[0],
			response[2],
			[200],
			["id"],
		)
		if result[0]:
			@warning_ignore("unsafe_cast")
			instance_id = UUID.from_String(result[4]["id"] as String)
		else:
			@warning_ignore("unsafe_cast")
			MiscHelpers.log_request_error(
				"error while getting instance id",
				result[1] as int,
				result[2] as String,
				result[3] as String,
			)
			get_tree().quit()

		await GlobalWorldHandler.load_world_server(world, port)

	finished_starting = true
	print("ready for connections")
