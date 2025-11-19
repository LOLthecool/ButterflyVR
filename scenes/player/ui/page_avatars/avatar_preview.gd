extends VBoxContainer
class_name AvatarPreview

@export var previewer:Previewer
@export var avatar_name:Label
@export var avatar_publicity:Label
@export var flag_list:FlagList
@export var details_button:Button
@export var equip_button:Button

var avatar:Dictionary[String, Variant]


func preview_avatar(avatar:Dictionary[String, Variant]) -> void:
	self.avatar = avatar
	previewer.create_preview(UUID.from_String(avatar["uuid"]))
	avatar_name = avatar["name"]
	avatar_publicity = avatar["privacy"]
	flag_list.create_list(avatar["flags"])
	# todo: handle showing details menu and pass equip to avatar manager

func on_avatar_details() -> void:
	pass

func on_avatar_equip() -> void:
	pass
