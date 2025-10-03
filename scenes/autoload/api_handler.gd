extends Node
class_name APIHandler

const TARGET_HOST:String = "127.0.0.1"

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
		var singal_name:String = str(randi())
		add_user_signal(singal_name, [
		{ "name": "response_code", "type": TYPE_INT},
		{ "name": "headers", "type": TYPE_PACKED_STRING_ARRAY},
		{ "name": "body", "type": TYPE_STRING}
		])
		on_complete = Signal(self, singal_name)

var is_ready:bool = false
var client = HTTPClient.new()
var waiting_requests:Array[Request]
@onready var tree = get_tree()

var headers:PackedStringArray = PackedStringArray(["User-Agent: Pirulo/1.0 (Godot)", "Accept: */*", "Content-Type: application/json"])

func make_request(method:HTTPClient.Method, target:String, body:String, headers:PackedStringArray = PackedStringArray()) -> Signal:
	var request:Request = Request.new(method, target, body, headers)
	waiting_requests.push_back(request)
	return request.on_complete

func _ready() -> void:
	push_error("temp code remove this")
	return
	while true:
		# cant figure out how to reuse the client so we restart the client after every request here
		client = HTTPClient.new()
		var err = client.connect_to_host("127.0.0.1", 23888)
		assert(err == OK)
		while client.get_status() == HTTPClient.STATUS_CONNECTING or client.get_status() == HTTPClient.STATUS_RESOLVING:
			client.poll()
			await tree.process_frame
		# regular request code from here
		assert(client.get_status() == HTTPClient.STATUS_CONNECTED)
		while waiting_requests.is_empty():
			await tree.physics_frame
		var request:Request = waiting_requests.pop_back()
		client.request(request.method, request.target, headers + request.additional_headers, request.body)
		while client.get_status() == HTTPClient.STATUS_REQUESTING:
			client.poll()
			await tree.process_frame
		assert(client.get_status() == HTTPClient.STATUS_BODY or client.get_status() == HTTPClient.STATUS_CONNECTED)
		if !client.has_response():
			request.on_complete.emit(-1, PackedStringArray(), "")
		else:
			var response_headers = client.get_response_headers()
			var raw_body = PackedByteArray()
			while client.get_status() == HTTPClient.STATUS_BODY:
				var chunk = client.read_response_body_chunk()
				client.poll()
				if chunk.size() == 0:
					await get_tree().process_frame
				else:
					raw_body = raw_body + chunk
			if raw_body.is_empty():
				request.on_complete.emit(client.get_response_code(), response_headers, "")
			else:
				var body = raw_body.get_string_from_ascii()
				request.on_complete.emit(client.get_response_code(), response_headers, body)
		# close the connection so it can be restarted
		client.close()
