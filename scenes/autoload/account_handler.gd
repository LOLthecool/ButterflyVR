extends Node
class_name AccountHandler
# handles token renewal and caches player info

# in long running sessions we may need to renew our token while the game is running
# the renewal threshold determines the minimum time remaining before renewing
const TOKEN_RENEWAL_THRESHOLD:int = 60 * 60 * 24 * 7 * 3 # 3 weeks
# and the check rate determines how often we check our token expiry time
const TOKEN_RENEWAL_CHECK_RATE:int = 1800 # 30 minutes

const TOKEN_RENEW_ENDPOINT:String = "API/V0/token/"
const TOKEN_VERIFY_ENDPOINT:String = "API/V0/token/validate"

var session_token:Array[int] = []
# expiry time in seconds since epoch, -1 indicates no token or a token that never expires
var token_expiry_utc:int = -1
# tokens for temporary sessions cannot renew themselves, in that case renewal logic is disabled
var token_renewable:bool = false
var token_valid:bool = false
var persist_token:bool = true
var renew_timer:Timer = Timer.new()

class APIPlayer:
	var id:UUID

func _enter_tree() -> void:
	var saved_token:Array[int] = []
	saved_token.assign(GlobalPersistanceHandler.register_value("user_login", "token", "token", []))
	var expiry:int = GlobalPersistanceHandler.register_value("user_login", "token", "expiry", -1)
	var renewable:bool = GlobalPersistanceHandler.register_value("user_login", "token", "renewable", false)
	if await is_token_valid(saved_token):
		token_valid = true
		set_token(saved_token, expiry, renewable)
		check_renew()

func _ready() -> void:
	renew_timer.autostart = true
	renew_timer.wait_time = TOKEN_RENEWAL_CHECK_RATE
	renew_timer.timeout.connect(check_renew)
	add_child.call_deferred(renew_timer)

func logout() -> void:
	token_valid = false
	set_token([], -1, false)

func set_token(token:Array[int], expiry_utc:int, renewable:bool) -> void:
	session_token = token
	token_expiry_utc = expiry_utc
	token_renewable = renewable
	if persist_token:
		GlobalPersistanceHandler.set_value("user_login", "token", "token", token)
		GlobalPersistanceHandler.set_value("user_login", "token", "expiry", expiry_utc)
		GlobalPersistanceHandler.set_value("user_login", "token", "renewable", renewable)

func is_token_valid(token:Array[int]) -> bool:
	if token == []:
		return false
	var token_header:PackedStringArray = PackedStringArray(["token: " + str(token)])
	# response = [response_code, response_headers, response_body]
	var response:Array[Variant] = await GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, TOKEN_VERIFY_ENDPOINT, token_header)
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
		token_valid = false
		set_token([], -1, false)
	
	# renewal logic
	if token_expiry_utc < 0 or !token_renewable:
		return
	if int(Time.get_unix_time_from_system()) + TOKEN_RENEWAL_THRESHOLD > token_expiry_utc:
		var token_header:PackedStringArray = PackedStringArray(["token: " + str(session_token)])
		GlobalAPIHandler.make_request(HTTPClient.METHOD_GET, TOKEN_RENEW_ENDPOINT, token_header).connect(on_token_request)

func on_token_request(response_code:HTTPClient.ResponseCode, _headers:PackedStringArray, body:String) -> void:
	if response_code != HTTPClient.RESPONSE_OK:
		push_warning("server error when renewing token. code: ", response_code)
	var body_json:Dictionary = JSON.parse_string(body)
	var response_token:Array[int] = body_json["token"].map(func(x:float) -> int: return int(x))
	if response_token.size() == 0:
		push_error("tried to renew token but server did not reply with one")
		return
	token_valid = true
	set_token(response_token, int(body_json["token_expiry_utc"]), true)
