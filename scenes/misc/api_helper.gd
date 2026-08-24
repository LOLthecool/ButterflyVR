extends Node
class_name APIHelper

const USER_INFO_ENDPOINT: String = "/api/v0/user/%s"


static func get_username(uuid: UUID) -> String:
	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_GET,
		USER_INFO_ENDPOINT % uuid.to_string(),
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
	)
	@warning_ignore("unsafe_call_argument") var result: Array[Variant] = GlobalAPIHandler.handle_response(
		response[0],
		response[2],
		[200],
		["username"],
	)

	if !result[0]:
		if result[1] == 404:
			push_warning("user with id %s does not exist" % uuid)
			return "MissingUser"

		@warning_ignore("unsafe_cast")
		MiscHelpers.log_request_error(
			"error while getting username for user with id %s" % uuid.to_string(),
			result[1] as int,
			result[2] as String,
			result[3] as String,
		)
		return "UNNAMED"

	return result[4]["username"]
