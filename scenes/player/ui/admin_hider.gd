extends Node

const MODERATION_ROUTE: String = "/api/v0/mod"


func _ready() -> void:
	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_GET,
		MODERATION_ROUTE,
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
	)
	if response[0] != 200:
		get_parent().queue_free()
