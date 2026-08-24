extends HFlowContainer
class_name AvatarList

const SEARCH_ENDPOINT: String = "/api/v0/search/%s"

@export var avatar_previewer: AvatarPreview


func _ready() -> void:
	get_and_show_avatars("", { })


func get_and_show_avatars(search_string: String, filters: Dictionary[String, String]) -> void:
	visible = true

	filters["is"] = "avatar"

	var filter_string: String = ""
	for key: String in filters.keys():
		filter_string += "%s:%s," % [key, filters[key]]

	filter_string.trim_suffix(",")

	# remove & to stop users accidentally breaking the filters
	search_string = search_string.remove_char("&".unicode_at(0))

	var search: String = "%s&%s" % [search_string, filter_string]
	var response: Array[Variant] = await GlobalAPIHandler.make_request(
		HTTPClient.METHOD_GET,
		SEARCH_ENDPOINT % search,
		PackedStringArray([GlobalAccountHandler.get_token_header()]),
	)

	for child: Node in get_children():
		child.queue_free()
	await get_tree().physics_frame

	@warning_ignore("unsafe_call_argument") var result: Array[Variant] = GlobalAPIHandler.handle_response(
		response[0],
		response[2],
		[200],
		["avatars"],
	)
	if !result[0]:
		var error_msg: Label = Label.new()
		error_msg.text = "error while retriving avatars, please try again"
		@warning_ignore("unsafe_cast")
		MiscHelpers.log_request_error(
			"error while retriving avatars",
			result[1] as int,
			result[2] as String,
			result[3] as String,
		)
		return

	var first_entry: bool = true
	for avatar_untyped: Dictionary in result[4]["avatars"]:
		var avatar: Dictionary[String, Variant] = { }
		avatar.assign(avatar_untyped)
		var avatar_listing: ObjectListing = ObjectListing.new()
		avatar_listing.create(avatar, TypeHelper.ObjectType.avatar)
		avatar_listing.object_selected.connect(avatar_previewer.preview_avatar)
		add_child(avatar_listing)
		if first_entry:
			first_entry = false
			avatar_previewer.preview_avatar(avatar)
