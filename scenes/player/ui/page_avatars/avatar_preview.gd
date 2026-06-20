extends VBoxContainer
class_name AvatarPreview

const OBJECT_INFO_ENDPOINT:String = "/api/v0/%s/%s"

@export var previewer:Previewer
@export var avatar_name:Label
@export var avatar_publicity:Label
@export var avatar_author:Label
@export var flag_list:FlagList
@export var details_button:Button
@export var equip_button:Button
@export var details_page:AvatarDetailsPage

var avatar:Dictionary[String, Variant]

func preview_avatar(avatar:Dictionary[String, Variant]) -> void:
	var response:Array[Variant] = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, OBJECT_INFO_ENDPOINT % ["Avatar", avatar["id"]], 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	@warning_ignore("unsafe_call_argument")
	var result:Array[Variant] = GlobalAPIHandler.handle_response(response[0], response[2], [200], 
			["id", "name", "description", "flags", "updated_at", "created_at", "object_size", "creator", "publicity", "tags"])
	
	if !result[0]:
		push_error("error when getting avatar details")
		if result[1] != -1:
			push_error("server response: %s" % result[1])
		if result[2] != "":
			push_error("error code: %s" % result[2])
		if result[3] != "":
			push_error("error message: %s" % result[3])
		return
	
	avatar = result[4]
	
	self.avatar = avatar
	@warning_ignore("unsafe_cast")
	previewer.create_preview(UUID.from_String(avatar["id"] as String))
	avatar_name.text = avatar["name"]
	@warning_ignore("unsafe_cast")
	avatar_publicity.text = "publicity: %s" % StringifyHelper.stringify_object_publicity(avatar["publicity"] as int)
	@warning_ignore("unsafe_cast")
	avatar_author.text = "created by: %s" % await APIHelper.get_username(UUID.from_String(avatar["creator"] as String))
	@warning_ignore("unsafe_cast")
	flag_list.create_list(avatar["flags"] as Array)

func on_avatar_details() -> void:
	if !details_page.visible:
		details_page.show_details(avatar)
	else:
		details_page.visible = false

func on_avatar_equip() -> void:
	var avatar_handler:AvatarChangeHandler = GlobalWorldHandler.current_world.avatar_change_handler
	@warning_ignore("unsafe_cast")
	avatar_handler.send_message((await GlobalAccountHandler.get_uuid()).backing_storage, UUID.from_String(avatar["id"] as String))
