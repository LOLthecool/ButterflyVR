extends Node
class_name APIHandler

const TARGET_HOST:String = "127.0.0.1"
const TARGET_PORT:int = 23888
const RECONNECT_DELAY_TIME:float = 3

# contains the request information stored before processing a request
class Request:
	var method:HTTPClient.Method
	var target:String
	var body:String
	var additional_headers:PackedStringArray = PackedStringArray()
	var on_complete:Signal
	@warning_ignore("shadowed_variable")
	func _init(method:HTTPClient.Method, target:String, body:String, headers:PackedStringArray) -> void:
		self.method = method
		self.target = target
		self.body = body
		additional_headers = headers
		if body != "":
			additional_headers.push_back("Content-Length: " + str(body.length()))
			additional_headers.push_back("Content-Type: application/json")
		var singal_name:String = str(randi())
		add_user_signal(singal_name, [
		{ "name": "response_code", "type": TYPE_INT},
		{ "name": "headers", "type": TYPE_PACKED_STRING_ARRAY},
		{ "name": "body", "type": TYPE_STRING}
		])
		on_complete = Signal(self, singal_name)

var is_ready:bool = false
var client:HTTPClient
var waiting_requests:Array[Request]
@onready var tree:SceneTree = get_tree()

# todo: make readonly once its available
var headers:PackedStringArray = PackedStringArray(["User-Agent: Pirulo/1.0 (Godot)", "Accept: */*"])

# makes a request for the handler to process, requests are handled sequentially.
# returns a signal that can be awaited to get the response (if it is received).
# user-agent, accept, content-type, and content-length headers are managed automatically.
func make_request(method:HTTPClient.Method, target:String, request_headers:PackedStringArray = PackedStringArray(), body:String = "") -> Signal:
	var request:Request = Request.new(method, target, body, request_headers)
	waiting_requests.push_back(request)
	return request.on_complete

# generic handler for api responses, 
# can check that specific response codes are sent or that specific values are in the response body
# returns a bool indicating sucess, the response code, an error code or message if one was sent by the server, and an array of the requested values
func handle_response(code:HTTPClient.ResponseCode, body:String, expected_codes:Array[int], expected_body_keys:Array[String]) -> Array[Variant]:
	var success:bool = false
	var response_code:int = code
	var error_code:int = -1
	var error_message:String = ""
	var response_values:Array[Variant] = []
	
	if !(code in expected_codes):
		if body != "":
			var decoder:JSON = JSON.new()
			if decoder.data is Dictionary and "error_code" in (decoder.data as Dictionary):
				error_code = (decoder.data as Dictionary)["error_code"]
			if decoder.data is Dictionary and "error_message" in (decoder.data as Dictionary):
				error_message = (decoder.data as Dictionary)["error_message"]
		return [success, response_code, error_code, error_string, response_values]
	
	var decoder:JSON = JSON.new()
	if decoder.data is not Dictionary:
		return [success, response_code, error_code, error_string, response_values]
	var data:Dictionary = decoder.data as Dictionary
	
	if "error_code" in data:
		error_code = data["error_code"]
	if "error_message" in data:
		error_message = data["error_message"]
	
	if !(expected_body_keys.all(func(x:String) -> bool: return x in data)):
		return [success, response_code, error_code, error_string, response_values]
	
	success = true
	for x:String in expected_body_keys:
		response_values.push_back(data[x])
	return [success, response_code, error_code, error_string, response_values]

# request handler, runs forever.
# will call itself deferred to recreate the connection if it errors out
func _ready() -> void:
	client = HTTPClient.new()
	var err:Error = client.connect_to_host(TARGET_HOST, TARGET_PORT)
	if err != OK:
		push_error("error while connecting to api: ", str(err))
		await tree.create_timer(3).timeout
		push_warning("retrying connection...")
		_ready.call_deferred()
		return
	while client.get_status() == HTTPClient.STATUS_CONNECTING or client.get_status() == HTTPClient.STATUS_RESOLVING:
		client.poll()
		await tree.process_frame
	# main processing loop
	while true:
		if client.get_status() != HTTPClient.STATUS_CONNECTED:
			push_error("error in api connection: client state should be connected but was ", client.get_status())
			await tree.create_timer(3).timeout
			push_warning("retrying connection...")
			_ready.call_deferred()
			return
		while waiting_requests.is_empty():
			await tree.physics_frame
		var request:Request = waiting_requests.pop_back()
		client.request(request.method, request.target, headers + request.additional_headers, request.body)
		while client.get_status() == HTTPClient.STATUS_REQUESTING:
			client.poll()
			await tree.process_frame
		if client.get_status() != HTTPClient.STATUS_BODY and client.get_status() != HTTPClient.STATUS_CONNECTED:
			push_error("error in api connection: expected body or ready connection, got: ", client.get_status())
			await tree.create_timer(3).timeout
			push_warning("retrying connection...")
			_ready.call_deferred()
			return
		if !client.has_response():
			request.on_complete.emit(-1, PackedStringArray(), "")
		else:
			# body retrival, works for chunked or unchunked responses
			var response_headers:PackedStringArray = client.get_response_headers()
			var raw_body:PackedByteArray = PackedByteArray()
			while client.get_status() == HTTPClient.STATUS_BODY:
				var chunk:PackedByteArray = client.read_response_body_chunk()
				client.poll()
				if chunk.size() == 0:
					await get_tree().process_frame
				else:
					raw_body = raw_body + chunk
			if raw_body.is_empty():
				request.on_complete.emit(client.get_response_code(), response_headers, "")
			else:
				var body:String = raw_body.get_string_from_ascii()
				request.on_complete.emit(client.get_response_code(), response_headers, body)
