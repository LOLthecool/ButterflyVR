extends Button

const USER_HOMEWORLD_ENDPOINT: String = "/api/v0/this_user/homeworld"

@export var instance_page: InstancePage


func _pressed() -> void:
	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_POST,
		USER_HOMEWORLD_ENDPOINT,
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
		JSON.stringify({ "uuid": instance_page.world_uuid.to_string() }),
	)
	@warning_ignore("unsafe_call_argument") var result: Array[Variant] = GlobalAPIHandler.handle_response(
		response[0],
		response[2],
		[200],
		[],
	)

	if !result[0]:
		@warning_ignore("unsafe_cast")
		MiscHelpers.log_request_error(
			"failed to update homeworld",
			result[1] as int,
			result[2] as String,
			result[3] as String,
		)
