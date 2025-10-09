extends Node
class_name AccountHandler
# handles token renewal and caches player info

# in long running sessions we may need to renew our token while the game is running
# the renewal threshold determines the minimum time remaining before renewing
const TOKEN_RENEWAL_THRESHOLD:int = 60 * 60 * 24 * 7 * 3 # 3 weeks
# and the check rate determines how often we check our token expiry time
const TOKEN_RENEWAL_CHECK_RATE:int = 1800 # 30 minutes

var session_token:Array[int] = []
# expiry time in seconds since epoch, -1 indicates no token or a token that never expires
var token_expiry_utc:int = -1
# tokens for temporary sessions cannot renew themselves, in that case renewal logic is disabled
var token_renewable:bool = false
var renew_timer:Timer = Timer.new()

class APIPlayer:
	var id:UUID

func _init() -> void:
	var saved_token:Array[int] = GlobalPersistanceHandler.register_value("user_login", "token", "token", [])
	var expiry:int = GlobalPersistanceHandler.register_value("user_login", "token", "expiry", -1)
	var renewable:bool = GlobalPersistanceHandler.register_value("user_login", "token", "renewable", false)
	if saved_token == []:
		return
	if await is_token_valid(saved_token):
		set_token(saved_token, expiry, renewable)
		check_renew()

func _ready() -> void:
	renew_timer.autostart = true
	renew_timer.wait_time = TOKEN_RENEWAL_CHECK_RATE
	renew_timer.timeout.connect(check_renew)
	add_child.call_deferred(renew_timer)

func logout() -> void:
	session_token = []
	token_expiry_utc = -1
	token_renewable = false

func set_token(token:Array[int], expiry_utc:int, renewable:bool) -> void:
	session_token = token
	token_expiry_utc = expiry_utc
	token_renewable = renewable

func is_token_valid(token:Array[int]) -> bool:
	var token_header:PackedStringArray = PackedStringArray(["token: " + str(token)])
	# response = [response_code, response_headers, response_body]
	var response:Array[Variant] = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, "API/V0/token/validate", token_header)
	if response[0] == HTTPClient.RESPONSE_OK:
		return true
	elif response[0] == HTTPClient.RESPONSE_UNAUTHORIZED:
		return false
	else:
		push_error("server error while validating token. code: ", response[0])
		return false

func check_renew() -> void:
	# indicate when token has expired
	# should only happen with non renewable tokens and a session lasting longer than the token expiry time
	if int(Time.get_unix_time_from_system()) > token_expiry_utc:
		push_error("token expired! if this is a renewable token (remember me checked when signing in) this is a bug")
		token_expiry_utc = -1
		session_token = []
	
	# renewal logic
	if token_expiry_utc < 0 or !token_renewable:
		return
	if int(Time.get_unix_time_from_system()) + TOKEN_RENEWAL_THRESHOLD > token_expiry_utc:
		var token_header:PackedStringArray = PackedStringArray(["token: " + str(session_token)])
		GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, "API/V0/user/", token_header).connect(on_token_request)

func on_token_request(response_code:HTTPClient.ResponseCode, _headers:PackedStringArray, body:String) -> void:
	if response_code != HTTPClient.RESPONSE_OK:
		push_warning("server error when renewing token. code: ", response_code)
	var body_json:Dictionary = JSON.parse_string(body)
	var response_token:Array[float] = body_json["token"]
	if response_token.size() == 0:
		push_error("tried to renew token but server did not reply with one")
		token_renewable = false
		return
	session_token = response_token.map(func(x:float) -> int: return int(x))
	token_expiry_utc = body_json["token_expiry_utc"]
	# renewed tokens are always renewable so dont need to update that here
