extends HBoxContainer

const MODERATION_MODERATE_OBJECT_ROUTE: String = "/api/v0/mod/moderate_object"

@export var token_source:ObjectVerifyTokenUI
@export var confirm_modal:ConfirmModal
@export var user_ban_modal:UserBanModal
@export var button1:Button
@export var button2:Button
@export var button3:Button

var selected:UUID
var object_creator:UUID

func on_generated(_token: UUID, object_id:UUID, creator:UUID) -> void:
	button1.disabled = false
	button2.disabled = false
	button3.disabled = false
	selected = object_id
	object_creator = creator


func _on_button_2_pressed() -> void:
	if !confirm_modal.show("verify object with id %s?" % selected):
		return
	
	var request:Dictionary[String, String] = {"target":selected.to_string(), "action":"Verify"}
	
	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_POST,
		MODERATION_MODERATE_OBJECT_ROUTE,
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
		JSON.stringify(request),
	)
	start_reset()
	
	if response[0] != 200:
		@warning_ignore("unsafe_call_argument")
		MiscHelpers.log_request_error("error while verifying object", response[0], "", "")



func _on_button_3_pressed() -> bool:
	if !confirm_modal.show("remove object with id %s?" % selected):
		return false
	
	var request:Dictionary[String, String] = {"target":selected.to_string(), "action":"Remove"}
	
	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_POST,
		MODERATION_MODERATE_OBJECT_ROUTE,
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
		JSON.stringify(request),
	)
	start_reset()
	
	if response[0] != 200:
		@warning_ignore("unsafe_call_argument")
		MiscHelpers.log_request_error("error while removing object", response[0], "", "")
		return false
	
	return true


func _on_button_4_pressed() -> void:
	if await _on_button_3_pressed():
		user_ban_modal.open(object_creator)

func start_reset() -> void:
	token_source.end()
	button1.disabled = true
	button2.disabled = true
	button3.disabled = true
