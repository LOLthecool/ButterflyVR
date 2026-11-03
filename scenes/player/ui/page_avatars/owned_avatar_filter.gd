extends Button

@export var avatar_list: AvatarList


func _pressed() -> void:
	await avatar_list.get_and_show_avatars(
		"",
		{ "creator": (await GlobalAccountHandler.get_uuid()).to_string() },
	)
