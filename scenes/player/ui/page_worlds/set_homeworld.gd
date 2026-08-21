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
		push_error("failed to update homeworld")
		if result[1] != -1:
			push_error("server response: %s" % result[1])
		if result[2] != "":
			push_error("error code: %s" % result[2])
		if result[3] != "":
			push_error("error message: %s" % result[3])
