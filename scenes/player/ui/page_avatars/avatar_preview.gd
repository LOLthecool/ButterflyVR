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

var avatar:Dictionary[String, Variant]

func preview_avatar(avatar:Dictionary[String, Variant]) -> void:
	var response = await GlobalAPIHandler.make_request(
			HTTPClient.METHOD_GET, OBJECT_INFO_ENDPOINT % ["Avatar", avatar["id"]], 
			PackedStringArray([GlobalAccountHandler.get_token_header()]))
	var result = GlobalAPIHandler.handle_response(response[0], response[2], [200], 
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
	previewer.create_preview(UUID.from_String(avatar["id"]))
	avatar_name.text = avatar["name"]
	avatar_publicity.text = "publicity: %s" % avatar["publicity"]
	avatar_author.text = "created by: %s" % avatar["creator"]
	flag_list.create_list(avatar["flags"])

func on_avatar_details() -> void:
	# todo: details page with extra info
	pass

func on_avatar_equip() -> void:
	var avatar_handler:AvatarChangeHandler = GlobalWorldHandler.current_world.avatar_change_handler
	avatar_handler.send_message(NetworkManager.get_id(), avatar["id"])
