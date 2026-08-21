extends HFlowContainer
class_name tags_list


func show_tags(object: Dictionary) -> void:
	for child: Node in get_children():
		child.queue_free()

	for tag: String in object["tags"]:
		var tag_button: Button = Button.new()
		tag_button.text = tag
		add_child(tag_button)
