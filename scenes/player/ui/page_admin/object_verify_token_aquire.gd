extends HBoxContainer
class_name ObjectVerifyTokenUI

const MODERATION_MODERATE_OBJECT_ROUTE: String = "/api/v0/mod/object_token"

signal token_generated(token: UUID, object_id: UUID, creator: UUID)
signal reset

@export var token_box: LineEdit
@export var generate_button: Button


func _on_button_pressed() -> void:
	generate_button.disabled = true
	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_GET,
		MODERATION_MODERATE_OBJECT_ROUTE,
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
	)

	@warning_ignore("unsafe_call_argument") var result: Array[Variant] = GlobalAPIHandler.handle_response(
		response[0],
		response[2],
		[200],
		["token", "object_id", "creator"],
	)

	if !result[0]:
		generate_button.disabled = false
		if result[1] == 404 and result[2] == "DosentExist":
			return # no objects available to verify
		@warning_ignore("unsafe_call_argument")
		MiscHelpers.log_request_error(
			"error while getting one time token",
			result[1],
			result[2],
			result[3],
		)
		return

	token_box.text = result[4]["token"]
	@warning_ignore("unsafe_call_argument")
	token_generated.emit(
		UUID.from_String(result[4]["token"]),
		UUID.from_String(result[4]["object_id"]),
		UUID.from_String(result[4]["creator"]),
	)


func end() -> void:
	generate_button.disabled = false
	token_box.text = ""
	reset.emit()
