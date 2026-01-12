extends VBoxContainer
class_name AvatarPreview

@export var previewer:Previewer
@export var avatar_name:Label
@export var avatar_publicity:Label
@export var avatar_author:Label
@export var flag_list:FlagList
@export var details_button:Button
@export var equip_button:Button

var avatar:Dictionary[String, Variant]


func preview_avatar(avatar:Dictionary[String, Variant]) -> void:
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
