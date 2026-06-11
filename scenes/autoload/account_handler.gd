extends Node
class_name AccountHandler
# handles token renewal and caches player info

# in long running sessions we may need to renew our token while the game is running
# the renewal threshold determines the minimum time remaining before renewing
const TOKEN_RENEWAL_THRESHOLD:int = 60 * 60 * 24 * 7 # 1 week
# and the check rate determines how often we check our token expiry time
const TOKEN_RENEWAL_CHECK_RATE:int = 60 * 60 # 1 hour

const TOKEN_RENEW_ENDPOINT:String = "/api/v0/token"
const TOKEN_VERIFY_ENDPOINT:String = "/api/v0/token/validate"
const TOKEN_USER_ENDPOINT:String = "/api/v0/token/user"

var session_token:PackedByteArray = PackedByteArray()
# expiry time in seconds since epoch, -1 indicates no token or a token that never expires
var token_expiry_utc:int = -1
# tokens for temporary sessions cannot renew themselves, in that case renewal logic is disabled
var token_renewable:bool = false
var user_id:UUID = UUID.new()
var renew_timer:Timer = Timer.new()
var token_checkable:bool = false

func _enter_tree() -> void:
	var saved_token:Array[int] = []
	@warning_ignore("unsafe_cast")
	saved_token.assign(GlobalPersistanceHandler.register_value("user_login", "token", "token", []) as Array)
	var expiry:int = GlobalPersistanceHandler.register_value("user_login", "token", "expiry", -1)
	var renewable:bool = GlobalPersistanceHandler.register_value("user_login", "token", "renewable", false)
	if await is_token_valid(saved_token, expiry):
		set_token(saved_token, expiry, renewable)
		check_renew()
	token_checkable = true

# renew token if game is closing so the user has the full 1 month 
# to log in again before it expires
func _exit_tree() -> void:
	if token_renewable:
		var token_header:PackedStringArray = PackedStringArray([get_token_header()])
		GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, TOKEN_RENEW_ENDPOINT, token_header).connect(on_token_request)

func _ready() -> void:
	renew_timer.autostart = true
	renew_timer.wait_time = TOKEN_RENEWAL_CHECK_RATE
	renew_timer.timeout.connect(check_renew)
	add_child.call_deferred(renew_timer)

func logout() -> void:
	set_token([], -1, false)
	GlobalPersistanceHandler.set_value("user_login", "token", "token", [])
	GlobalPersistanceHandler.set_value("user_login", "token", "expiry", -1)
	GlobalPersistanceHandler.set_value("user_login", "token", "renewable", false)

func set_token(token:Array[int], expiry_utc:int, renewable:bool) -> void:
	session_token = token
	token_expiry_utc = expiry_utc
	token_renewable = renewable
	if token_renewable:
		GlobalPersistanceHandler.set_value("user_login", "token", "token", token)
		GlobalPersistanceHandler.set_value("user_login", "token", "expiry", expiry_utc)
		GlobalPersistanceHandler.set_value("user_login", "token", "renewable", renewable)
	if await is_token_valid(session_token, token_expiry_utc):
		user_id = await get_uuid(false)

func check_token_valid() -> bool:
	while !token_checkable:
		await get_tree().physics_frame
	return await is_token_valid(session_token, token_expiry_utc)

func get_token_header() -> String:
	return "token: %s" % session_token.hex_encode()

func get_uuid(use_cached_value:bool = true) -> UUID:
	if GlobalServerHandler.is_gameserver:
		return GlobalServerHandler.instance_id
	if use_cached_value and user_id != UUID.new():
		return user_id
	var token_header:PackedStringArray = PackedStringArray([get_token_header()])
	var response:Array[Variant] = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, TOKEN_USER_ENDPOINT, token_header)
	@warning_ignore("unsafe_call_argument")
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], ["id"])
	var values:Dictionary[String, Variant] = result[4]
	if !result[0]:
		push_error("failed to aquire user uuid")
		if result[1] != -1:
			push_error("server response: %s" % result[1])
		if result[2] != "":
			push_error("error code: %s" % result[2])
		if result[3] != "":
			push_error("error message: %s" % result[3])
		return UUID.new()
	@warning_ignore("unsafe_cast")
	return UUID.from_String(values["id"] as String)

func is_token_valid(token:PackedByteArray, expiry_utc:int) -> bool:
	if GlobalServerHandler.is_gameserver:
		return true
	if token == PackedByteArray():
		return false
	if expiry_utc != -1 and Time.get_unix_time_from_system() > expiry_utc:
		return false
	var token_header:PackedStringArray = PackedStringArray(["token: %s" % token.hex_encode()])
	# response = [response_code, response_headers, response_body]
	var response:Array[Variant] = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, TOKEN_VERIFY_ENDPOINT, token_header)
	if response[0] == HTTPClient.RESPONSE_OK:
		return true
	elif response[0] == HTTPClient.RESPONSE_UNAUTHORIZED:
		return false
	else:
		push_warning("server error while validating token. code: ", response[0])
		return false

func check_renew() -> void:
	if token_expiry_utc < 0 or !token_renewable:
		return
	
	# indicate when token has expired
	# should only happen with non renewable tokens and a session lasting longer than the token expiry time
	if int(Time.get_unix_time_from_system()) > token_expiry_utc:
		push_error("token expired! if this is a renewable token (remember me checked when signing in) this is a bug")
		set_token([], -1, false)
	
	if int(Time.get_unix_time_from_system()) + TOKEN_RENEWAL_THRESHOLD > token_expiry_utc:
		var token_header:PackedStringArray = PackedStringArray([get_token_header()])
		GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, TOKEN_RENEW_ENDPOINT, token_header).connect(on_token_request)

func on_token_request(response_code:HTTPClient.ResponseCode, _headers:PackedStringArray, body:String) -> void:
	if response_code != HTTPClient.RESPONSE_OK:
		push_error("server error when renewing token. code: ", response_code)
		return
	var body_json:Dictionary = JSON.parse_string(body)
	@warning_ignore("unsafe_cast")
	var response_token:Array[int] = []
	@warning_ignore("unsafe_cast")
	response_token.assign(body_json["token"] as Array)
	if response_token.size() == 0:
		push_error("tried to renew token but server did not reply with one")
		return
	@warning_ignore("unsafe_cast")
	set_token(response_token, body_json["token_expires"] as int, true)
