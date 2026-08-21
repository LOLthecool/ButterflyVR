extends VBoxContainer
class_name AvatarDetailsPage

@export var details_name: Label
@export var details_description: Label
@export var details_creation_time: Label
@export var details_update_time: Label
@export var details_size: Label
@export var details_tags: tags_list


func show_details(avatar: Dictionary) -> void:
	visible = true

	details_name.text = avatar["name"]
	details_description.text = avatar["description"]
	@warning_ignore("unsafe_cast")
	details_creation_time.text = Time \
			.get_datetime_string_from_unix_time(avatar["created_at"] as int, true) \
			.split(" ")[0]
	@warning_ignore("unsafe_cast")
	details_update_time.text = Time \
			.get_datetime_string_from_unix_time(avatar["updated_at"] as int, true) \
			.split(" ")[0]
	@warning_ignore("unsafe_cast")
	details_size.text = StringifyHelper.stringify_size_kb(avatar["object_size"] as int)

	details_tags.show_tags(avatar)
