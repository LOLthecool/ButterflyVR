extends VBoxContainer

const MODERATOR_SEARCH_ROUTE: String = "/api/v0/mod/search"

@export var text: LineEdit
@export var results_container: VBoxContainer
@export var user_ban_modal: UserBanModal
@export var moderate_button: Button

var selected: UUID


func search() -> void:
	var request: Dictionary[String, String] = { "search_term": text.text, "search_type": "Users" }

	if UUID.is_uuid(text.text):
		request["target_id"] = UUID.from_String(text.text).to_string()

	for child: Node in results_container.get_children():
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

	for user: Dictionary in result[4]["results"]:
		var listing: Button = Button.new()
		listing.add_theme_font_size_override("font_size", 24)
		listing.alignment = HORIZONTAL_ALIGNMENT_LEFT
		listing.text = "%s -- UUID: %s" % [user["name"], user["id"]]
		@warning_ignore("unsafe_call_argument")
		listing.pressed.connect(on_select.bind(UUID.from_String(user["id"])))
		results_container.add_child(listing)


func on_select(id: UUID) -> void:
	moderate_button.disabled = false
	selected = id


func on_moderate() -> void:
	moderate_button.disabled = true
	user_ban_modal.open(selected)


func _on_button_pressed() -> void:
	search()


func _on_line_edit_text_submitted(_new_text: String) -> void:
	search()
