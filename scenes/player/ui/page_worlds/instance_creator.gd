extends Control
class_name InstanceCreator

@export var name_entry:LineEdit
@export var max_players:HSlider
@export var publicity:OptionButton
@export var anyone_can_invite:CheckBox
@export var is_gameserver:CheckBox

var world:UUID

func make_visible() -> void:
	visible = true

func make_invisible() -> void:
	visible = false

func create_and_join_instance() -> void:
	var join_permissions:InstanceHandler.InstanceJoinPermission
	
	match publicity.get_selected_id():
		0:
			join_permissions = InstanceHandler.InstanceJoinPermission.invite
		1:
			join_permissions = InstanceHandler.InstanceJoinPermission.group
		2:
			join_permissions = InstanceHandler.InstanceJoinPermission.friends
		3:
			join_permissions = InstanceHandler.InstanceJoinPermission.public
	
	
	await GlobalWorldHandler.load_world(world, 
			await GlobalInstanceHandler.create_online_instance(
					world,
					join_permissions,
					anyone_can_invite.button_pressed,
					is_gameserver.button_pressed,
					name_entry.text,
					int(max_players.value)
			))
