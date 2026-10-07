extends VBoxContainer

const MODERATOR_SEARCH_ROUTE:String = "/api/v0/mod/search"
const MODERATION_MODERATE_USER_ROUTE: String = "/api/v0/mod/moderate_user"

@export var text:LineEdit
@export var results_container:FlowContainer
@export var user_ban_modal:UserBanModal

var selected:UUID

func search(_x:String) -> void:
	var request:Dictionary[String, String] = {
			"search_term":text.text, 
			"search_type":"Users",}
	
	if UUID.is_uuid(text.text):
		request["target_id"] = UUID.from_String(text.text).to_string()
	
	for child:Node in results_container.get_children():
		child.queue_free()
	
	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_POST,
		MODERATOR_SEARCH_ROUTE,
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
		JSON.stringify(request),
	)

	@warning_ignore("unsafe_call_argument") var result: Array[Variant] = GlobalAPIHandler.handle_response(
		response[0],
		response[2],
		[200],
		["results"],
	)
	
	if !result[0]:
		@warning_ignore("unsafe_call_argument")
		MiscHelpers.log_request_error("error during search", result[1], result[2], result[3])
	
	for object:Dictionary in result[4]:
		var listing:Button = Button.new()
		listing.add_theme_font_size_override("font_size", 32)
		listing.text = "%s -- Creator ID: %s -- UUID: %s" % [object["name"], object["creator"], object["id"]]
		@warning_ignore("unsafe_call_argument")
		listing.pressed.connect(on_select.bind(UUID.from_String(object["id"]), UUID.from_String(object["creator"])))
		results_container.add_child(listing)

func on_select(id:UUID, creator:UUID) -> void:
	remove_button.disabled = false
	remove_and_ban_button.disabled = false
	selected = id
	selected_creator = creator

func on_remove() -> bool:
	remove_button.disabled = true
	remove_and_ban_button.disabled = true
	
	if !confirm_modal.show("remove object with id %s?" % selected):
		return false
	
	var request:Dictionary[String, String] = {"target":selected.to_string(), "action":"Remove"}
	
	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_POST,
		MODERATION_MODERATE_OBJECT_ROUTE,
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
		JSON.stringify(request),
	)
	
	if response[0] != 200:
		@warning_ignore("unsafe_call_argument")
		MiscHelpers.log_request_error("error while removing object", response[0], "", "")
		return false
	
	return true


func on_remove_and_moderate() -> void:
	if await on_remove():
		user_ban_modal.open(selected_creator)
