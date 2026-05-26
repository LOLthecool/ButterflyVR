extends Node
class_name APIHelper

const USER_INFO_ENDPOINT:String = "/api/v0/user/%s"

static func get_username(uuid:UUID) -> String:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, 
			USER_INFO_ENDPOINT % uuid.to_string(), 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	@warning_ignore("unsafe_call_argument")
	var result:Array[Variant] = GlobalAPIHandler.handle_response(
			response[0], 
			response[2], 
			[200], 
			["username"])
	
	if !result[0]:
		if result[1] == 404:
			push_warning("user with id %s does not exist" % uuid)
			return "MissingUser"
		
		push_error("failed to aquire username for user uuid %s" % uuid.to_string())
		if result[1] != -1:
			push_error("server response: %s" % result[1])
		if result[2] != "":
			push_error("error code: %s" % result[2])
		if result[3] != "":
			push_error("error message: %s" % result[3])
		return "UNNAMED"
	
	return result[4]["username"]
